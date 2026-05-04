use std::sync::Arc;

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use crate::{executor, logging, parse};

// ---------------------------------------------------------------------------
// Public constructors
// ---------------------------------------------------------------------------

/// Build a Rhai engine with the supplied `args` available to scripts via
/// `script_args()`.  All resource limits and host functions are configured.
pub fn build_engine_with_args(args: Vec<String>) -> Engine {
    let mut engine = build_engine_inner();

    let args_arc = Arc::new(args);
    let args_for_fn = Arc::clone(&args_arc);
    engine.register_fn("script_args", move || -> Array {
        args_for_fn
            .iter()
            .map(|s| Dynamic::from(s.clone()))
            .collect()
    });

    engine
}

/// Convenience wrapper that builds an engine with no script args.
/// Useful for tests and for calling contexts that pass no extra arguments.
pub fn build_engine() -> Engine {
    build_engine_with_args(Vec::new())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Configure resource limits and all host functions except `script_args`.
fn build_engine_inner() -> Engine {
    let mut engine = Engine::new();

    // Resource limits (per _contract/02-host-fns.md)
    engine.set_max_operations(1_000_000);
    engine.set_max_call_levels(32);
    engine.set_max_string_size(102_400);
    engine.set_max_array_size(10_000);
    engine.set_max_modules(0);
    engine.disable_symbol("eval");

    register_host_fns(&mut engine);

    engine
}

fn register_host_fns(engine: &mut Engine) {
    // exec — validates argv via pact, spawns process, enforces timeout + cap.
    engine.register_fn(
        "exec",
        |binary: String, args: Array| -> Result<Map, Box<EvalAltResult>> {
            let argv: Vec<String> = args.into_iter().map(|d| d.cast::<String>()).collect();
            executor::run_exec(&binary, &argv)
        },
    );

    // exec_allow_fail — like exec but non-zero exit returns map instead of throwing.
    engine.register_fn(
        "exec_allow_fail",
        |binary: String, args: Array| -> Result<Map, Box<EvalAltResult>> {
            let argv: Vec<String> = args.into_iter().map(|d| d.cast::<String>()).collect();
            executor::run_exec_allow_fail(&binary, &argv)
        },
    );

    // parse_json — deserialise JSON string to Dynamic.
    engine.register_fn(
        "parse_json",
        |s: &str| -> Result<Dynamic, Box<EvalAltResult>> { parse::parse_json(s) },
    );

    // parse_yaml — deserialise YAML string to Dynamic.
    engine.register_fn(
        "parse_yaml",
        |s: &str| -> Result<Dynamic, Box<EvalAltResult>> { parse::parse_yaml(s) },
    );

    // print — use on_print so Rhai's built-in variadic handling calls our handler.
    engine.on_print(logging::print_line);

    // log_info / log_warn / log_error — single-string stderr logging.
    engine.register_fn("log_info", |msg: &str| logging::log_info(msg));
    engine.register_fn("log_warn", |msg: &str| logging::log_warn(msg));
    engine.register_fn("log_error", |msg: &str| logging::log_error(msg));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::EvalAltResult;

    #[test]
    fn engine_constructs() {
        let _engine = build_engine();
    }

    #[test]
    fn stub_exec_callable() {
        let engine = build_engine();
        let result = engine.run(r#"exec("whoami", []);"#);
        assert!(result.is_ok(), "stub exec should be callable: {:?}", result);
    }

    #[test]
    fn stub_log_callable() {
        let engine = build_engine();
        let result = engine.run(r#"log_info("hi");"#);
        assert!(
            result.is_ok(),
            "log_info should be callable: {:?}",
            result
        );
    }

    // Row #7 — import should be rejected (modules disabled via set_max_modules(0))
    #[test]
    fn rejects_module_import() {
        let engine = build_engine();
        let result = engine.run(r#"import "fs" as fs;"#);
        assert!(result.is_err(), "module import should be rejected");
    }

    // Row #8 — eval symbol is disabled
    #[test]
    fn rejects_eval() {
        let engine = build_engine();
        let result = engine.run(r#"eval("1 + 1")"#);
        assert!(result.is_err(), "eval should be rejected");
    }

    // Row #9 — loop exceeding max_operations should throw
    #[test]
    fn rejects_excessive_operations() {
        let engine = build_engine();
        let result = engine.run("let i = 0; while i < 10_000_000 { i += 1; }");
        let err = result.unwrap_err();
        assert!(
            matches!(err.as_ref(), EvalAltResult::ErrorTooManyOperations(_)),
            "expected ErrorTooManyOperations, got: {:?}",
            err
        );
    }

    // Task-7: script_args returns the args passed to build_engine_with_args
    #[test]
    fn script_args_returns_passed_args() {
        let engine =
            build_engine_with_args(vec!["foo".to_owned(), "bar".to_owned()]);
        let arr: rhai::Array = engine
            .eval("script_args()")
            .expect("script_args() should evaluate");
        assert_eq!(arr.len(), 2, "expected 2 args");
        assert_eq!(arr[0].clone().cast::<String>(), "foo");
        assert_eq!(arr[1].clone().cast::<String>(), "bar");
    }

    // Task-7: parse_json is wired through the engine
    #[test]
    fn parse_json_callable() {
        let engine = build_engine();
        let result: i64 = engine
            .eval(r#"parse_json("{\"x\":1}").x"#)
            .expect("parse_json should work via engine");
        assert_eq!(result, 1_i64);
    }

    // Task-7: print via on_print does not throw
    #[test]
    fn print_via_engine_does_not_throw() {
        let engine = build_engine();
        let result = engine.run(r#"print("hello")"#);
        assert!(result.is_ok(), "print should not throw: {:?}", result);
    }
}
