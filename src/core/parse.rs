//! Data-parsing host functions: `parse_json` and `parse_yaml`.

use rhai::{Dynamic, EvalAltResult, Map, Position};

/// Maximum input size accepted by parse functions.
///
/// This bounds large-flat-input DoS. Note: it does NOT fully prevent YAML
/// alias-bomb expansion, which can amplify a small input into enormous output.
const MAX_PARSE_BYTES: usize = 10 * 1024 * 1024; // 10 MiB

/// Parse a JSON string into a Rhai `Dynamic` value.
///
/// Throws a Rhai runtime error with `kind = "ParseError"` and
/// `format = "json"` on failure. Inputs exceeding 10 MiB are rejected
/// before deserialization.
pub fn parse_json(s: &str) -> Result<Dynamic, Box<EvalAltResult>> {
    if s.len() > MAX_PARSE_BYTES {
        return Err(parse_error(
            "json",
            &format!("input exceeds {} bytes", MAX_PARSE_BYTES),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(s).map_err(|e| {
        parse_error("json", &e.to_string())
    })?;
    rhai::serde::to_dynamic(value).map_err(|e| parse_error("json", &e.to_string()))
}

/// Parse a YAML string into a Rhai `Dynamic` value.
///
/// Throws a Rhai runtime error with `kind = "ParseError"` and
/// `format = "yaml"` on failure. Inputs exceeding 10 MiB are rejected
/// before deserialization.
pub fn parse_yaml(s: &str) -> Result<Dynamic, Box<EvalAltResult>> {
    if s.len() > MAX_PARSE_BYTES {
        return Err(parse_error(
            "yaml",
            &format!("input exceeds {} bytes", MAX_PARSE_BYTES),
        ));
    }
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

    #[test]
    fn parse_json_rejects_oversized_input() {
        // Length check runs before parse, so content does not need to be valid JSON.
        let oversized = "a".repeat(MAX_PARSE_BYTES + 1);
        let result = parse_json(&oversized);
        let err = result.unwrap_err();
        match err.as_ref() {
            rhai::EvalAltResult::ErrorRuntime(dyn_val, _) => {
                let map = dyn_val.clone().cast::<Map>();
                assert_eq!(map.get("kind").unwrap().clone().cast::<String>(), "ParseError");
                assert_eq!(map.get("format").unwrap().clone().cast::<String>(), "json");
                let msg = map.get("message").unwrap().clone().cast::<String>();
                assert!(msg.contains("10485760"), "message should mention the byte limit: {}", msg);
            }
            other => panic!("expected ErrorRuntime, got: {:?}", other),
        }
    }

    #[test]
    fn parse_yaml_rejects_oversized_input() {
        // Length check runs before parse, so content does not need to be valid YAML.
        let oversized = "a".repeat(MAX_PARSE_BYTES + 1);
        let result = parse_yaml(&oversized);
        let err = result.unwrap_err();
        match err.as_ref() {
            rhai::EvalAltResult::ErrorRuntime(dyn_val, _) => {
                let map = dyn_val.clone().cast::<Map>();
                assert_eq!(map.get("kind").unwrap().clone().cast::<String>(), "ParseError");
                assert_eq!(map.get("format").unwrap().clone().cast::<String>(), "yaml");
                let msg = map.get("message").unwrap().clone().cast::<String>();
                assert!(msg.contains("10485760"), "message should mention the byte limit: {}", msg);
            }
            other => panic!("expected ErrorRuntime, got: {:?}", other),
        }
    }
}
