//! SnapConfig - Zero-copy configuration access.

use memmap2::Mmap;
use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyInt, PyList, PyString};

use crate::value::{ArchivedFlatValue, ArchivedValueNode, FlatValue};

/// Zero-copy view into cached configuration data.
#[pyclass]
pub struct SnapConfig {
    mmap: Mmap,
    root_idx: u32,
    #[pyo3(get)]
    cache_path: String,
    #[pyo3(get)]
    source_path: Option<String>,
}

impl SnapConfig {
    pub fn new(mmap: Mmap, root_idx: u32, cache_path: String, source_path: Option<String>) -> Self {
        Self {
            mmap,
            root_idx,
            cache_path,
            source_path,
        }
    }

    #[inline]
    pub(crate) fn archived(&self) -> &ArchivedFlatValue {
        unsafe { rkyv::archived_root::<FlatValue>(&self.mmap) }
    }

    fn node_type_name(node: &ArchivedValueNode) -> &'static str {
        match node {
            ArchivedValueNode::Null => "null",
            ArchivedValueNode::Bool(_) => "bool",
            ArchivedValueNode::Int(_) => "int",
            ArchivedValueNode::Float(_) => "float",
            ArchivedValueNode::String(_) => "string",
            ArchivedValueNode::Array(_) => "array",
            ArchivedValueNode::Object(_) => "object",
        }
    }
}

#[pymethods]
impl SnapConfig {
    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<PyObject> {
        let archived = self.archived();
        let root_node = &archived.nodes[self.root_idx as usize];
        get_item_from_node(py, &archived.nodes, root_node, key)
    }

    /// Get nested value using dot notation (e.g., "database.host").
    fn get(&self, py: Python<'_>, path: &str) -> PyResult<PyObject> {
        let archived = self.archived();
        let parts: Vec<&str> = path.split('.').collect();
        let mut current_idx = self.root_idx;

        for part in parts {
            let node = &archived.nodes[current_idx as usize];
            match node {
                ArchivedValueNode::Object(pairs) => {
                    if let Some(idx) = find_key_in_object(pairs, part) {
                        current_idx = idx;
                    } else {
                        return Err(PyKeyError::new_err(format!("Key not found: {}", part)));
                    }
                }
                ArchivedValueNode::Array(indices) => {
                    if let Ok(idx) = part.parse::<usize>() {
                        if idx < indices.len() {
                            current_idx = indices[idx];
                        } else {
                            return Err(PyKeyError::new_err(format!(
                                "Index out of bounds: {}",
                                idx
                            )));
                        }
                    } else {
                        return Err(PyTypeError::new_err("Cannot index array with non-integer"));
                    }
                }
                _ => {
                    return Err(PyTypeError::new_err(format!(
                        "Cannot traverse into {:?}",
                        Self::node_type_name(node)
                    )));
                }
            }
        }

        node_to_python(py, &archived.nodes, current_idx)
    }

    fn keys(&self, py: Python<'_>) -> PyResult<PyObject> {
        let archived = self.archived();
        let root_node = &archived.nodes[self.root_idx as usize];

        match root_node {
            ArchivedValueNode::Object(pairs) => {
                let list = PyList::empty_bound(py);
                for pair in pairs.iter() {
                    list.append(pair.0.as_str())?;
                }
                Ok(list.into())
            }
            _ => Err(PyTypeError::new_err("keys() only works on objects")),
        }
    }

    fn __len__(&self) -> PyResult<usize> {
        let archived = self.archived();
        let root_node = &archived.nodes[self.root_idx as usize];

        match root_node {
            ArchivedValueNode::Object(pairs) => Ok(pairs.len()),
            ArchivedValueNode::Array(indices) => Ok(indices.len()),
            _ => Err(PyTypeError::new_err("Object has no length")),
        }
    }

    fn __contains__(&self, key: &str) -> PyResult<bool> {
        let archived = self.archived();
        let root_node = &archived.nodes[self.root_idx as usize];

        match root_node {
            ArchivedValueNode::Object(pairs) => Ok(find_key_in_object(pairs, key).is_some()),
            _ => Err(PyTypeError::new_err("'in' only works on objects")),
        }
    }

    /// Convert to Python dict/list (loses zero-copy benefits).
    fn to_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let archived = self.archived();
        node_to_python(py, &archived.nodes, self.root_idx)
    }

    fn root_type(&self) -> &'static str {
        let archived = self.archived();
        let root_node = &archived.nodes[self.root_idx as usize];
        Self::node_type_name(root_node)
    }

    fn __repr__(&self) -> String {
        let archived = self.archived();
        let root_node = &archived.nodes[self.root_idx as usize];
        let type_name = Self::node_type_name(root_node);

        let size = match root_node {
            ArchivedValueNode::Object(pairs) => format!("{} keys", pairs.len()),
            ArchivedValueNode::Array(indices) => format!("{} items", indices.len()),
            _ => "scalar".to_string(),
        };

        format!(
            "SnapConfig({}, {}, cache='{}')",
            type_name, size, self.cache_path
        )
    }
}

pub fn find_key_in_object(
    pairs: &rkyv::vec::ArchivedVec<(rkyv::string::ArchivedString, u32)>,
    key: &str,
) -> Option<u32> {
    pairs
        .binary_search_by(|pair| pair.0.as_str().cmp(key))
        .ok()
        .map(|idx| pairs[idx].1)
}

fn get_item_from_node(
    py: Python<'_>,
    nodes: &rkyv::vec::ArchivedVec<ArchivedValueNode>,
    node: &ArchivedValueNode,
    key: &Bound<'_, PyAny>,
) -> PyResult<PyObject> {
    if let Ok(key_str) = key.downcast::<PyString>() {
        let key_str = key_str.to_str()?;

        match node {
            ArchivedValueNode::Object(pairs) => {
                if let Some(idx) = find_key_in_object(pairs, key_str) {
                    node_to_python(py, nodes, idx)
                } else {
                    Err(PyKeyError::new_err(format!("Key not found: {}", key_str)))
                }
            }
            _ => Err(PyTypeError::new_err("Cannot index non-object with string")),
        }
    } else if let Ok(key_int) = key.downcast::<PyInt>() {
        let idx: usize = key_int.extract()?;

        match node {
            ArchivedValueNode::Array(indices) => {
                if idx < indices.len() {
                    node_to_python(py, nodes, indices[idx])
                } else {
                    Err(PyKeyError::new_err(format!("Index out of bounds: {}", idx)))
                }
            }
            _ => Err(PyTypeError::new_err("Cannot index non-array with integer")),
        }
    } else {
        Err(PyTypeError::new_err("Key must be string or integer"))
    }
}

pub fn node_to_python(
    py: Python<'_>,
    nodes: &rkyv::vec::ArchivedVec<ArchivedValueNode>,
    idx: u32,
) -> PyResult<PyObject> {
    let node = &nodes[idx as usize];

    match node {
        ArchivedValueNode::Null => Ok(py.None()),
        ArchivedValueNode::Bool(b) => Ok(b.to_object(py)),
        ArchivedValueNode::Int(i) => Ok(i.to_object(py)),
        ArchivedValueNode::Float(f) => Ok(f.to_object(py)),
        ArchivedValueNode::String(s) => Ok(s.as_str().to_object(py)),
        ArchivedValueNode::Array(indices) => {
            let list = PyList::empty_bound(py);
            for child_idx in indices.iter() {
                list.append(node_to_python(py, nodes, *child_idx)?)?;
            }
            Ok(list.into())
        }
        ArchivedValueNode::Object(pairs) => {
            let dict = PyDict::new_bound(py);
            for pair in pairs.iter() {
                let key = pair.0.as_str();
                let value_idx = pair.1;
                dict.set_item(key, node_to_python(py, nodes, value_idx)?)?;
            }
            Ok(dict.into())
        }
    }
}

/// Converts FlatValue to Python object (for loads() which doesn't use mmap).
pub fn flat_value_to_python(py: Python<'_>, flat: &crate::value::FlatValue) -> PyResult<PyObject> {
    use crate::value::ValueNode;

    fn convert(py: Python<'_>, nodes: &[ValueNode], idx: u32) -> PyResult<PyObject> {
        let node = &nodes[idx as usize];

        match node {
            ValueNode::Null => Ok(py.None()),
            ValueNode::Bool(b) => Ok(b.to_object(py)),
            ValueNode::Int(i) => Ok(i.to_object(py)),
            ValueNode::Float(f) => Ok(f.to_object(py)),
            ValueNode::String(s) => Ok(s.to_object(py)),
            ValueNode::Array(indices) => {
                let list = PyList::empty_bound(py);
                for &child_idx in indices {
                    list.append(convert(py, nodes, child_idx)?)?;
                }
                Ok(list.into())
            }
            ValueNode::Object(pairs) => {
                let dict = PyDict::new_bound(py);
                for (key, value_idx) in pairs {
                    dict.set_item(key, convert(py, nodes, *value_idx)?)?;
                }
                Ok(dict.into())
            }
        }
    }

    let root_idx = flat
        .root()
        .ok_or_else(|| PyValueError::new_err("FlatValue missing root node"))?;
    convert(py, &flat.nodes, root_idx)
}
