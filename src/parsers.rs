//! Format parsers for snapconfig.

use crate::error::{Result, SnapconfigError};
use crate::value::{FlatValue, ValueIdx, ValueNode};
use ini::Ini;
use std::path::Path;

fn sort_pairs(pairs: &mut Vec<(String, ValueIdx)>) {
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
}

fn parse_scalar_value(flat: &mut FlatValue, value: &str) -> ValueIdx {
    if value.is_empty() {
        flat.add_node(ValueNode::String(String::new()))
    } else if value.eq_ignore_ascii_case("true") {
        flat.add_node(ValueNode::Bool(true))
    } else if value.eq_ignore_ascii_case("false") {
        flat.add_node(ValueNode::Bool(false))
    } else if value.eq_ignore_ascii_case("null")
        || value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("nil")
    {
        flat.add_node(ValueNode::Null)
    } else if let Ok(i) = value.parse::<i64>() {
        flat.add_node(ValueNode::Int(i))
    } else if let Ok(f) = value.parse::<f64>() {
        flat.add_node(ValueNode::Float(f))
    } else {
        flat.add_node(ValueNode::String(value.to_string()))
    }
}
pub fn parse_json(content: &str) -> Result<FlatValue> {
    let mut bytes = content.as_bytes().to_vec();
    let parsed = simd_json::to_owned_value(&mut bytes)?;
    Ok(from_simd_json(parsed))
}

pub fn from_simd_json(value: simd_json::OwnedValue) -> FlatValue {
    let mut flat = FlatValue::new();
    let root_idx = add_simd_json_value(&mut flat, value);
    flat.set_root(root_idx);
    flat
}

fn add_simd_json_value(flat: &mut FlatValue, value: simd_json::OwnedValue) -> ValueIdx {
    use simd_json::prelude::*;

    if value.is_null() {
        flat.add_node(ValueNode::Null)
    } else if let Some(b) = value.as_bool() {
        flat.add_node(ValueNode::Bool(b))
    } else if let Some(i) = value.as_i64() {
        flat.add_node(ValueNode::Int(i))
    } else if let Some(f) = value.as_f64() {
        flat.add_node(ValueNode::Float(f))
    } else if let Some(s) = value.as_str() {
        flat.add_node(ValueNode::String(s.to_string()))
    } else if value.is_array() {
        if let Some(arr) = value.into_array() {
            let indices: Vec<ValueIdx> = arr
                .into_iter()
                .map(|v| add_simd_json_value(flat, v))
                .collect();
            flat.add_node(ValueNode::Array(indices))
        } else {
            flat.add_node(ValueNode::Null)
        }
    } else if value.is_object() {
        if let Some(obj) = value.into_object() {
            let mut pairs: Vec<(String, ValueIdx)> = obj
                .into_iter()
                .map(|(k, v)| (k.to_string(), add_simd_json_value(flat, v)))
                .collect();
            sort_pairs(&mut pairs);
            flat.add_node(ValueNode::Object(pairs))
        } else {
            flat.add_node(ValueNode::Null)
        }
    } else {
        flat.add_node(ValueNode::Null)
    }
}

pub fn from_yaml(value: serde_yaml::Value) -> FlatValue {
    let mut flat = FlatValue::new();
    let root_idx = add_yaml_value(&mut flat, value);
    flat.set_root(root_idx);
    flat
}

pub fn parse_yaml(content: &str) -> Result<FlatValue> {
    let parsed: serde_yaml::Value = serde_yaml::from_str(content)?;
    Ok(from_yaml(parsed))
}

fn add_yaml_value(flat: &mut FlatValue, value: serde_yaml::Value) -> ValueIdx {
    use serde_yaml::Value;

    match value {
        Value::Null => flat.add_node(ValueNode::Null),
        Value::Bool(b) => flat.add_node(ValueNode::Bool(b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                flat.add_node(ValueNode::Int(i))
            } else if let Some(f) = n.as_f64() {
                flat.add_node(ValueNode::Float(f))
            } else {
                flat.add_node(ValueNode::Null)
            }
        }
        Value::String(s) => flat.add_node(ValueNode::String(s)),
        Value::Sequence(arr) => {
            let indices: Vec<ValueIdx> = arr.into_iter().map(|v| add_yaml_value(flat, v)).collect();
            flat.add_node(ValueNode::Array(indices))
        }
        Value::Mapping(obj) => {
            let mut pairs: Vec<(String, ValueIdx)> = obj
                .into_iter()
                .filter_map(|(k, v)| {
                    let key = match k {
                        Value::String(s) => s,
                        _ => k.as_str()?.to_string(),
                    };
                    Some((key, add_yaml_value(flat, v)))
                })
                .collect();
            sort_pairs(&mut pairs);
            flat.add_node(ValueNode::Object(pairs))
        }
        Value::Tagged(tagged) => add_yaml_value(flat, tagged.value),
    }
}

pub fn from_toml(value: toml::Value) -> FlatValue {
    let mut flat = FlatValue::new();
    let root_idx = add_toml_value(&mut flat, value);
    flat.set_root(root_idx);
    flat
}

pub fn parse_toml(content: &str) -> Result<FlatValue> {
    let parsed: toml::Value = toml::from_str(content)?;
    Ok(from_toml(parsed))
}

fn add_toml_value(flat: &mut FlatValue, value: toml::Value) -> ValueIdx {
    use toml::Value;

    match value {
        Value::String(s) => flat.add_node(ValueNode::String(s)),
        Value::Integer(i) => flat.add_node(ValueNode::Int(i)),
        Value::Float(f) => flat.add_node(ValueNode::Float(f)),
        Value::Boolean(b) => flat.add_node(ValueNode::Bool(b)),
        Value::Datetime(dt) => flat.add_node(ValueNode::String(dt.to_string())),
        Value::Array(arr) => {
            let indices: Vec<ValueIdx> = arr.into_iter().map(|v| add_toml_value(flat, v)).collect();
            flat.add_node(ValueNode::Array(indices))
        }
        Value::Table(table) => {
            let mut pairs: Vec<(String, ValueIdx)> = table
                .into_iter()
                .map(|(k, v)| (k, add_toml_value(flat, v)))
                .collect();
            sort_pairs(&mut pairs);
            flat.add_node(ValueNode::Object(pairs))
        }
    }
}

pub fn parse_ini(content: &str) -> Result<FlatValue> {
    let ini = Ini::load_from_str(content).map_err(|e| SnapconfigError::IniParse(e.to_string()))?;

    let mut flat = FlatValue::new();
    let mut sections: Vec<(String, ValueIdx)> = Vec::new();

    for (section, props) in ini.iter() {
        let section_name = section.unwrap_or("default").to_string();
        let mut pairs: Vec<(String, ValueIdx)> = Vec::new();

        for (key, value) in props.iter() {
            let value_idx = parse_scalar_value(&mut flat, value);
            pairs.push((key.to_string(), value_idx));
        }

        sort_pairs(&mut pairs);
        let section_idx = flat.add_node(ValueNode::Object(pairs));
        sections.push((section_name, section_idx));
    }

    sort_pairs(&mut sections);
    let root_idx = flat.add_node(ValueNode::Object(sections));
    flat.set_root(root_idx);
    Ok(flat)
}

pub fn parse_env(content: &str) -> FlatValue {
    let mut flat = FlatValue::new();
    let mut pairs: Vec<(String, ValueIdx)> = Vec::new();

    for line in content.lines() {
        let mut line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Handle 'export ' prefix (shell-compatible .env files)
        if let Some(stripped) = line.strip_prefix("export ") {
            line = stripped;
        }

        // Parse KEY=VALUE
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let mut value = line[eq_pos + 1..].trim().to_string();

            // Remove surrounding quotes if present
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                if value.len() >= 2 {
                    value = value[1..value.len() - 1].to_string();
                }
            }

            let value_idx = parse_scalar_value(&mut flat, &value);
            pairs.push((key, value_idx));
        }
    }

    sort_pairs(&mut pairs);
    let root_idx = flat.add_node(ValueNode::Object(pairs));
    flat.set_root(root_idx);
    flat
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Yaml,
    Toml,
    Ini,
    Env,
}

impl Format {
    pub fn from_path(path: &Path) -> Option<Self> {
        let path_str = path.to_string_lossy().to_lowercase();

        if path_str.ends_with(".json") {
            Some(Format::Json)
        } else if path_str.ends_with(".yaml") || path_str.ends_with(".yml") {
            Some(Format::Yaml)
        } else if path_str.ends_with(".toml") {
            Some(Format::Toml)
        } else if path_str.ends_with(".ini")
            || path_str.ends_with(".cfg")
            || path_str.ends_with(".conf")
        {
            Some(Format::Ini)
        } else if path_str.ends_with(".env") || path_str.contains(".env.") {
            Some(Format::Env)
        } else {
            None
        }
    }
}

pub fn parse_content(content: &str, path: &Path) -> Result<FlatValue> {
    match Format::from_path(path).unwrap_or(Format::Env) {
        Format::Json => parse_json(content),
        Format::Yaml => parse_yaml(content),
        Format::Toml => parse_toml(content),
        Format::Ini => parse_ini(content),
        Format::Env => Ok(parse_env(content)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_simple() {
        let flat = parse_json(r#"{"key": "value", "num": 42}"#).unwrap();
        assert_eq!(flat.len(), 3); // string, int, object
    }

    #[test]
    fn test_parse_json_nested() {
        let flat = parse_json(r#"{"a": {"b": {"c": 1}}}"#).unwrap();
        assert_eq!(flat.len(), 4); // int, 3 objects
    }

    #[test]
    fn test_parse_yaml() {
        let flat = parse_yaml("key: value\nnum: 42").unwrap();
        assert_eq!(flat.len(), 3);
    }

    #[test]
    fn test_parse_toml() {
        let flat = parse_toml("[section]\nkey = \"value\"").unwrap();
        assert_eq!(flat.len(), 3); // string, section object, root object
    }

    #[test]
    fn test_parse_ini() {
        let flat = parse_ini("[section]\nkey = value").unwrap();
        // 1 string value + 1 section object + 1 root object = 3 or 4
        // (may include empty "default" section)
        assert!(flat.len() >= 3);
    }

    #[test]
    fn test_parse_env() {
        let flat = parse_env("KEY=value\nNUM=42\nBOOL=true");
        assert_eq!(flat.len(), 4); // 3 values + root object
    }

    #[test]
    fn test_parse_env_export() {
        let flat = parse_env("export KEY=value");
        let root_idx = flat.root().expect("expected root");
        if let ValueNode::Object(pairs) = &flat.nodes[root_idx as usize] {
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].0, "KEY");
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_parse_env_quotes() {
        let flat = parse_env("KEY=\"quoted value\"");
        let root_idx = flat.root().expect("expected root");
        if let ValueNode::Object(pairs) = &flat.nodes[root_idx as usize] {
            let value_idx = pairs[0].1;
            if let ValueNode::String(s) = &flat.nodes[value_idx as usize] {
                assert_eq!(s, "quoted value");
            } else {
                panic!("Expected String");
            }
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(
            Format::from_path(Path::new("config.json")),
            Some(Format::Json)
        );
        assert_eq!(
            Format::from_path(Path::new("config.yaml")),
            Some(Format::Yaml)
        );
        assert_eq!(
            Format::from_path(Path::new("config.yml")),
            Some(Format::Yaml)
        );
        assert_eq!(
            Format::from_path(Path::new("Cargo.toml")),
            Some(Format::Toml)
        );
        assert_eq!(
            Format::from_path(Path::new("settings.ini")),
            Some(Format::Ini)
        );
        assert_eq!(Format::from_path(Path::new(".env")), Some(Format::Env));
        assert_eq!(
            Format::from_path(Path::new(".env.local")),
            Some(Format::Env)
        );
    }
}
