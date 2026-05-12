use std::sync::Arc;

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use crate::core::{audit::AuditEvent, executor, logging, parse, run_context::RunContext};

// ---------------------------------------------------------------------------
// Public constructors
// ---------------------------------------------------------------------------

/// Build a Rhai engine with the supplied `args` available to scripts via
/// `script_args()`, and `ctx` for audit/security.
pub fn build_engine_with_args(args: Vec<String>, ctx: Arc<RunContext>) -> Engine {
    let mut engine = build_engine_inner(ctx);

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

/// Convenience wrapper that builds an engine with no script args.  Test-only.
#[cfg(test)]
fn build_engine() -> Engine {
    use crate::core::audit::AuditWriter;
    use crate::security::SecurityConfig;
    use std::sync::{atomic::AtomicU32, Mutex};
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("tempdir");
    let run_id = uuid::Uuid::new_v4().to_string();
    let audit = AuditWriter::open(tmp.path(), &run_id).expect("open audit");
    // Keep the TempDir alive via a Box leak so the test doesn't delete the dir
    // before the writer is dropped — acceptable for short-lived tests.
    Box::leak(Box::new(tmp));

    let security = Arc::new(SecurityConfig::load().expect("load security config"));
    let ctx = Arc::new(RunContext {
        security,
        audit: Arc::new(Mutex::new(audit)),
        exec_counter: Arc::new(AtomicU32::new(0)),
    });
    build_engine_with_args(Vec::new(), ctx)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Configure resource limits and all host functions except `script_args`.
fn build_engine_inner(ctx: Arc<RunContext>) -> Engine {
    let mut engine = Engine::new();

    // Resource limits (per _contract/02-host-fns.md)
    engine.set_max_operations(1_000_000);
    engine.set_max_call_levels(32);
    engine.set_max_string_size(102_400);
    engine.set_max_array_size(10_000);
    engine.set_max_modules(0);
    engine.disable_symbol("eval");

    register_host_fns(&mut engine, ctx);

    engine
}

fn register_host_fns(engine: &mut Engine, ctx: Arc<RunContext>) {
    let ctx_exec = Arc::clone(&ctx);
    let ctx_exec_af = Arc::clone(&ctx);
    let ctx_log_info = Arc::clone(&ctx);
    let ctx_log_warn = Arc::clone(&ctx);
    let ctx_log_error = Arc::clone(&ctx);

    // exec — validates argv via pact, spawns process, enforces timeout + cap.
    engine.register_fn(
        "exec",
        move |binary: String, args: Array| -> Result<Map, Box<EvalAltResult>> {
            let argv: Vec<String> = args.into_iter().map(|d| d.cast::<String>()).collect();
            executor::run_exec_audited(
                &binary,
                &argv,
                false,
                Arc::clone(&ctx_exec.audit),
                Arc::clone(&ctx_exec.exec_counter),
            )
        },
    );

    // exec_allow_fail — like exec but non-zero exit returns map instead of throwing.
    engine.register_fn(
        "exec_allow_fail",
        move |binary: String, args: Array| -> Result<Map, Box<EvalAltResult>> {
            let argv: Vec<String> = args.into_iter().map(|d| d.cast::<String>()).collect();
            executor::run_exec_audited(
                &binary,
                &argv,
                true,
                Arc::clone(&ctx_exec_af.audit),
                Arc::clone(&ctx_exec_af.exec_counter),
            )
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

    // log_info / log_warn / log_error — single-string stderr logging + audit.
    engine.register_fn("log_info", move |msg: &str| {
        logging::log_info(msg);
        let run_id = ctx_log_info.audit.lock().map(|g| g.run_id.clone()).unwrap_or_default();
        let event = AuditEvent::script_log(&run_id, "info".to_owned(), msg.to_owned());
        crate::core::audit::try_emit(&ctx_log_info.audit, &event);
    });
    engine.register_fn("log_warn", move |msg: &str| {
        logging::log_warn(msg);
        let run_id = ctx_log_warn.audit.lock().map(|g| g.run_id.clone()).unwrap_or_default();
        let event = AuditEvent::script_log(&run_id, "warn".to_owned(), msg.to_owned());
        crate::core::audit::try_emit(&ctx_log_warn.audit, &event);
    });
    engine.register_fn("log_error", move |msg: &str| {
        logging::log_error(msg);
        let run_id = ctx_log_error.audit.lock().map(|g| g.run_id.clone()).unwrap_or_default();
        let event = AuditEvent::script_log(&run_id, "error".to_owned(), msg.to_owned());
        crate::core::audit::try_emit(&ctx_log_error.audit, &event);
    });
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
        use crate::core::audit::AuditWriter;
        use crate::core::run_context::RunContext;
        use crate::security::SecurityConfig;
        use std::sync::{atomic::AtomicU32, Arc, Mutex};
        use tempfile::TempDir;

        let tmp = TempDir::new().expect("tempdir");
        let run_id = uuid::Uuid::new_v4().to_string();
        let audit = AuditWriter::open(tmp.path(), &run_id).expect("open");
        let ctx = Arc::new(RunContext {
            security: Arc::new(SecurityConfig::load().expect("load security config")),
            audit: Arc::new(Mutex::new(audit)),
            exec_counter: Arc::new(AtomicU32::new(0)),
        });
        let engine =
            build_engine_with_args(vec!["foo".to_owned(), "bar".to_owned()], ctx);
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
