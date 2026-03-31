mod compose;
mod env;
mod package;
mod tsconfig;

use anyhow::Result;
use serde_json::json;

pub struct TemplateEngine;

impl TemplateEngine {
    pub fn new() -> Result<Self> {
        Ok(TemplateEngine)
    }
}

/// Convert a TOML value to a serde_json value for tsconfig generation.
pub(crate) fn toml_value_to_json(value: &toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(s) => json!(s),
        toml::Value::Integer(i) => json!(i),
        toml::Value::Float(f) => json!(f),
        toml::Value::Boolean(b) => json!(b),
        toml::Value::Array(a) => {
            serde_json::Value::Array(a.iter().map(toml_value_to_json).collect())
        }
        toml::Value::Table(t) => {
            let map: serde_json::Map<String, serde_json::Value> = t
                .iter()
                .map(|(k, v)| (k.clone(), toml_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        toml::Value::Datetime(d) => json!(d.to_string()),
    }
}

#[cfg(test)]
mod tests;
