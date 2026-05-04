//! Data-parsing host functions: `parse_json` and `parse_yaml`.

use rhai::{Dynamic, EvalAltResult, Map, Position};

/// Parse a JSON string into a Rhai `Dynamic` value.
///
/// Throws a Rhai runtime error with `kind = "ParseError"` and
/// `format = "json"` on failure.
pub fn parse_json(s: &str) -> Result<Dynamic, Box<EvalAltResult>> {
    let value: serde_json::Value = serde_json::from_str(s).map_err(|e| {
        parse_error("json", &e.to_string())
    })?;
    rhai::serde::to_dynamic(value).map_err(|e| parse_error("json", &e.to_string()))
}

/// Parse a YAML string into a Rhai `Dynamic` value.
///
/// Throws a Rhai runtime error with `kind = "ParseError"` and
/// `format = "yaml"` on failure.
pub fn parse_yaml(s: &str) -> Result<Dynamic, Box<EvalAltResult>> {
    let value: serde_yaml::Value = serde_yaml::from_str(s).map_err(|e| {
        parse_error("yaml", &e.to_string())
    })?;
    rhai::serde::to_dynamic(value).map_err(|e| parse_error("yaml", &e.to_string()))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_error(format: &str, message: &str) -> Box<EvalAltResult> {
    let mut map = Map::new();
    map.insert("kind".into(), Dynamic::from("ParseError".to_owned()));
    map.insert("format".into(), Dynamic::from(format.to_owned()));
    map.insert("message".into(), Dynamic::from(message.to_owned()));
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from_map(map),
        Position::NONE,
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::Map;

    #[test]
    fn parse_json_object() {
        let result = parse_json(r#"{"a": 1}"#).expect("valid JSON should parse");
        let map = result.cast::<Map>();
        let a = map.get("a").cloned().expect("key 'a' should exist");
        // serde_json::Value numbers are converted to i64 by rhai::serde::to_dynamic
        assert_eq!(a.cast::<i64>(), 1_i64);
    }

    #[test]
    fn parse_json_rejects_invalid() {
        let result = parse_json("{not json");
        assert!(result.is_err(), "invalid JSON should return Err");
    }

    #[test]
    fn parse_yaml_list() {
        let result = parse_yaml("- a\n- b\n").expect("valid YAML should parse");
        let arr = result.cast::<rhai::Array>();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn parse_yaml_rejects_invalid() {
        // Tabs at the start of a YAML mapping value are invalid in YAML.
        // Use a truly malformed document: unclosed flow mapping.
        let result = parse_yaml("{key: [unclosed");
        assert!(result.is_err(), "invalid YAML should return Err");
    }
}
