use rhai::{Array, Dynamic, Engine, Map};

pub fn build_engine() -> Engine {
    let mut engine = Engine::new();

    // Resource limits (per _contract/02-host-fns.md)
    engine.set_max_operations(1_000_000);
    engine.set_max_call_levels(32);
    engine.set_max_string_size(102_400);
    engine.set_max_array_size(10_000);
    engine.set_max_modules(0);
    engine.disable_symbol("eval");

    // Register stub host fns (real impls land in tasks 6 & 7)
    register_stubs(&mut engine);

    engine
}

fn register_stubs(engine: &mut Engine) {
    // stub — replaced by task-6
    engine.register_fn("exec", |_binary: String, _args: Array| -> Map { Map::new() });

    // stub — replaced by task-6
    engine.register_fn("exec_allow_fail", |_binary: String, _args: Array| -> Map {
        Map::new()
    });

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
