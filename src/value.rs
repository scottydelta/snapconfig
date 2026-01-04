//! Core value types for snapconfig.

use rkyv::bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Serialize};

pub type ValueIdx = u32;

/// Value node using indices instead of nested references (enables zero-copy).
#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq)]
#[archive_attr(derive(Debug, CheckBytes))]
pub enum ValueNode {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<ValueIdx>),
    Object(Vec<(String, ValueIdx)>),
}

/// Flat storage for configuration values.
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(Debug, CheckBytes))]
pub struct FlatValue {
    pub nodes: Vec<ValueNode>,
    pub root: Option<ValueIdx>,
}

impl FlatValue {
    #[inline]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            root: None,
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            root: None,
        }
    }

    #[inline]
    pub fn add_node(&mut self, node: ValueNode) -> ValueIdx {
        let idx = self.nodes.len() as ValueIdx;
        self.nodes.push(node);
        idx
    }

    #[inline]
    pub fn set_root(&mut self, idx: ValueIdx) {
        self.root = Some(idx);
    }

    #[inline]
    pub fn root(&self) -> Option<ValueIdx> {
        self.root
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for FlatValue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_value_new() {
        let fv = FlatValue::new();
        assert!(fv.is_empty());
        assert_eq!(fv.root, None);
    }

    #[test]
    fn test_add_node() {
        let mut fv = FlatValue::new();
        let idx = fv.add_node(ValueNode::Int(42));
        assert_eq!(idx, 0);
        assert_eq!(fv.len(), 1);
    }

    #[test]
    fn test_build_simple_object() {
        let mut fv = FlatValue::new();
        let str_idx = fv.add_node(ValueNode::String("hello".to_string()));
        let int_idx = fv.add_node(ValueNode::Int(42));
        let root = fv.add_node(ValueNode::Object(vec![
            ("name".to_string(), str_idx),
            ("value".to_string(), int_idx),
        ]));
        fv.set_root(root);

        assert_eq!(fv.len(), 3);
        assert_eq!(fv.root, Some(2));
    }
}
