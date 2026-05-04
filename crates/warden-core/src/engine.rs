use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use crate::executor;

pub fn build_engine() -> Engine {
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
            let argv: Vec<String> = args
                .into_iter()
                .map(|d| d.cast::<String>())
                .collect();
            executor::run_exec(&binary, &argv)
        },
    );

    // exec_allow_fail — like exec but non-zero exit returns map instead of throwing.
    engine.register_fn(
        "exec_allow_fail",
        |binary: String, args: Array| -> Result<Map, Box<EvalAltResult>> {
            let argv: Vec<String> = args
                .into_iter()
                .map(|d| d.cast::<String>())
                .collect();
            executor::run_exec_allow_fail(&binary, &argv)
        },
    );

    // stub — replaced by task-7
    engine.register_fn("parse_json", |_s: String| -> Dynamic { Dynamic::UNIT });

    // stub — replaced by task-7
    engine.register_fn("parse_yaml", |_s: String| -> Dynamic { Dynamic::UNIT });

    // stub — replaced by task-7
    engine.register_fn("script_args", || -> Array { Array::new() });

    // stub — replaced by task-7
    engine.register_fn("print", |_msg: String| {});

    // stub — replaced by task-7
    engine.register_fn("log_info", |_msg: String| {});

    // stub — replaced by task-7
    engine.register_fn("log_warn", |_msg: String| {});

    // stub — replaced by task-7
    engine.register_fn("log_error", |_msg: String| {});
}

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
            "stub log_info should be callable: {:?}",
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
}
