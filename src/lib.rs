//! snapconfig - Fast zero-copy configuration file loading for Python.
//!
//! Uses rkyv for zero-copy deserialization, providing up to 1000x faster
//! config file loading compared to standard JSON/YAML/TOML parsers.
//!
//! Supported formats: JSON, YAML, TOML, INI, dotenv

pub mod config;
pub mod error;
pub mod parsers;
pub mod value;

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use memmap2::Mmap;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use tempfile::Builder;

pub use config::SnapConfig;
pub use error::{Result, SnapconfigError};
pub use parsers::Format;
pub use value::{FlatValue, ValueNode};

#[pyfunction]
#[pyo3(signature = (source_path, cache_path=None))]
fn compile(source_path: &str, cache_path: Option<&str>) -> PyResult<String> {
    let source = Path::new(source_path);
    if !source.exists() {
        return Err(SnapconfigError::FileNotFound(source_path.to_string()).into());
    }

    let output_path: PathBuf = cache_path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("{}.snapconfig", source_path)));

    let content = fs::read_to_string(source)?;
    let flat_value = parsers::parse_content(&content, source)?;

    let bytes = rkyv::to_bytes::<_, 65536>(&flat_value)
        .map_err(|e| SnapconfigError::Serialize(e.to_string()))?;

    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = Builder::new()
        .prefix("snapconfig-")
        .suffix(".tmp")
        .tempfile_in(parent)?;
    tmp.as_file_mut().write_all(&bytes)?;
    tmp.as_file_mut().sync_all()?;
    tmp.persist(&output_path)
        .map_err(|e| SnapconfigError::Io(e.error))?;

    Ok(output_path.to_string_lossy().into_owned())
}

/// Load config file with automatic caching.
#[pyfunction]
#[pyo3(signature = (path, cache_path=None, force_recompile=false))]
fn load(path: &str, cache_path: Option<&str>, force_recompile: bool) -> PyResult<SnapConfig> {
    let source = Path::new(path);
    let cache = cache_path
        .map(String::from)
        .unwrap_or_else(|| format!("{}.snapconfig", path));
    let cache_file = Path::new(&cache);

    let needs_compile = force_recompile
        || !cache_file.exists()
        || (source.exists() && is_source_newer(source, cache_file)?);

    if needs_compile {
        if !source.exists() {
            return Err(
                SnapconfigError::FileNotFound(format!("{} (and no cache exists)", path)).into(),
            );
        }
        compile(path, Some(&cache))?;
    }

    load_compiled(&cache, if source.exists() { Some(path) } else { None })
}

fn is_source_newer(source: &Path, cache: &Path) -> PyResult<bool> {
    let source_modified = source.metadata()?.modified()?;
    let cache_modified = cache.metadata()?.modified()?;
    Ok(source_modified > cache_modified)
}

/// Load directly from compiled .snapconfig cache file (skips freshness check).
#[pyfunction]
#[pyo3(signature = (cache_path, source_path=None))]
fn load_compiled(cache_path: &str, source_path: Option<&str>) -> PyResult<SnapConfig> {
    let file = fs::File::open(cache_path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    if mmap.is_empty() {
        return Err(SnapconfigError::InvalidCache("Cache file is empty".to_string()).into());
    }

    rkyv::check_archived_root::<FlatValue>(&mmap)
        .map_err(|e| SnapconfigError::InvalidCache(format!("Validation failed: {}", e)))?;

    let archived = unsafe { rkyv::archived_root::<FlatValue>(&mmap) };
    let root_idx = archived
        .root
        .as_ref()
        .copied()
        .ok_or_else(|| SnapconfigError::InvalidCache("Cache missing root node".to_string()))?;
    if (root_idx as usize) >= archived.nodes.len() {
        return Err(SnapconfigError::InvalidCache(
            "Cache root node index is out of bounds".to_string(),
        )
        .into());
    }

    Ok(SnapConfig::new(
        mmap,
        root_idx,
        cache_path.to_string(),
        source_path.map(String::from),
    ))
}

/// Parse content from string without caching.
#[pyfunction]
#[pyo3(signature = (content, format="json"))]
fn loads(py: Python<'_>, content: &str, format: &str) -> PyResult<PyObject> {
    let flat_value = match format.to_lowercase().as_str() {
        "json" => parsers::parse_json(content)?,
        "yaml" | "yml" => parsers::parse_yaml(content)?,
        "toml" => parsers::parse_toml(content)?,
        "ini" | "cfg" => parsers::parse_ini(content)?,
        "env" => parsers::parse_env(content),
        _ => return Err(PyValueError::new_err(format!("Unknown format: {}", format))),
    };

    config::flat_value_to_python(py, &flat_value)
}

#[pyfunction]
#[pyo3(signature = (path=".env", cache_path=None, force_recompile=false))]
fn load_env(path: &str, cache_path: Option<&str>, force_recompile: bool) -> PyResult<SnapConfig> {
    load(path, cache_path, force_recompile)
}

/// Load .env file and populate os.environ.
#[pyfunction]
#[pyo3(signature = (path=".env", override_existing=false))]
fn load_dotenv(py: Python<'_>, path: &str, override_existing: bool) -> PyResult<usize> {
    let config = load_env(path, None, false)?;
    let os = py.import_bound("os")?;
    let environ = os.getattr("environ")?;

    let archived = config.archived();
    let root_idx = archived
        .root
        .as_ref()
        .copied()
        .ok_or_else(|| SnapconfigError::InvalidCache("Cache missing root node".to_string()))?;
    if (root_idx as usize) >= archived.nodes.len() {
        return Err(SnapconfigError::InvalidCache(
            "Cache root node index is out of bounds".to_string(),
        )
        .into());
    }
    let root_node = &archived.nodes[root_idx as usize];

    let mut count = 0;

    if let value::ArchivedValueNode::Object(pairs) = root_node {
        for pair in pairs.iter() {
            let key = pair.0.as_str();
            let value_idx = pair.1;
            let value_node = &archived.nodes[value_idx as usize];

            // Check if key already exists
            let exists: bool = environ.call_method1("__contains__", (key,))?.extract()?;
            if exists && !override_existing {
                continue;
            }

            // Convert value to string for os.environ
            let value_str = match value_node {
                value::ArchivedValueNode::String(s) => s.as_str().to_string(),
                value::ArchivedValueNode::Int(i) => i.to_string(),
                value::ArchivedValueNode::Float(f) => f.to_string(),
                value::ArchivedValueNode::Bool(b) => if *b { "true" } else { "false" }.to_string(),
                value::ArchivedValueNode::Null => String::new(),
                _ => continue,
            };

            environ.set_item(key, value_str)?;
            count += 1;
        }
    }

    Ok(count)
}

#[pyfunction]
fn parse_env(py: Python<'_>, content: &str) -> PyResult<PyObject> {
    let flat = parsers::parse_env(content);
    config::flat_value_to_python(py, &flat)
}

#[pyfunction]
fn cache_info(source_path: &str) -> PyResult<HashMap<String, PyObject>> {
    Python::with_gil(|py| {
        let mut info = HashMap::new();
        let source = Path::new(source_path);
        let cache_path = format!("{}.snapconfig", source_path);
        let cache = Path::new(&cache_path);

        info.insert("source_exists".to_string(), source.exists().to_object(py));
        info.insert("cache_exists".to_string(), cache.exists().to_object(py));
        info.insert("cache_path".to_string(), cache_path.clone().to_object(py));

        if source.exists() {
            if let Ok(meta) = source.metadata() {
                info.insert("source_size".to_string(), (meta.len() as i64).to_object(py));
            }
        }

        if cache.exists() {
            if let Ok(meta) = cache.metadata() {
                info.insert("cache_size".to_string(), (meta.len() as i64).to_object(py));
            }
            if source.exists() {
                if let (Ok(source_mod), Ok(cache_mod)) = (
                    source.metadata().and_then(|m| m.modified()),
                    cache.metadata().and_then(|m| m.modified()),
                ) {
                    info.insert(
                        "cache_fresh".to_string(),
                        (cache_mod >= source_mod).to_object(py),
                    );
                }
            }
        }

        Ok(info)
    })
}

#[pyfunction]
fn clear_cache(source_path: &str) -> PyResult<bool> {
    let cache_path = format!("{}.snapconfig", source_path);
    let cache = Path::new(&cache_path);

    if cache.exists() {
        fs::remove_file(cache)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[pymodule]
fn snapconfig(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SnapConfig>()?;
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    m.add_function(wrap_pyfunction!(load, m)?)?;
    m.add_function(wrap_pyfunction!(load_compiled, m)?)?;
    m.add_function(wrap_pyfunction!(loads, m)?)?;
    m.add_function(wrap_pyfunction!(load_env, m)?)?;
    m.add_function(wrap_pyfunction!(load_dotenv, m)?)?;
    m.add_function(wrap_pyfunction!(parse_env, m)?)?;
    m.add_function(wrap_pyfunction!(cache_info, m)?)?;
    m.add_function(wrap_pyfunction!(clear_cache, m)?)?;
    Ok(())
}
