use std::sync::Arc;

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map, Position};

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
    // Layer 1 FS functions — scoped to <reeve_home>/workspace/
    let workspace_root: Arc<std::path::Path> =
        Arc::from(ctx.security.reeve_home.join("workspace").as_path());
    crate::core::fs::register(engine, workspace_root);

    let ctx_exec = Arc::clone(&ctx);
    let ctx_exec_af = Arc::clone(&ctx);
    let ctx_log_info = Arc::clone(&ctx);
    let ctx_log_warn = Arc::clone(&ctx);
    let ctx_log_error = Arc::clone(&ctx);
    let ctx_env = Arc::clone(&ctx);

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
                &ctx_exec.security.env_passthrough,
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
                &ctx_exec_af.security.env_passthrough,
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

    // env — reads an environment variable; key must be in env_passthrough.
    engine.register_fn(
        "env",
        move |key: &str| -> Result<String, Box<EvalAltResult>> {
            if !ctx_env.security.env_passthrough.iter().any(|k| k == key) {
                let mut map = Map::new();
                map.insert("kind".into(), Dynamic::from("EnvDenied".to_owned()));
                map.insert("key".into(), Dynamic::from(key.to_owned()));
                return Err(Box::new(EvalAltResult::ErrorRuntime(
                    Dynamic::from(map),
                    Position::NONE,
                )));
            }
            match std::env::var(key) {
                Ok(val) => Ok(val),
                Err(std::env::VarError::NotPresent) => {
                    let mut map = Map::new();
                    map.insert("kind".into(), Dynamic::from("EnvUnset".to_owned()));
                    map.insert("key".into(), Dynamic::from(key.to_owned()));
                    Err(Box::new(EvalAltResult::ErrorRuntime(
                        Dynamic::from(map),
                        Position::NONE,
                    )))
                }
                Err(std::env::VarError::NotUnicode(_)) => {
                    let mut map = Map::new();
                    map.insert("kind".into(), Dynamic::from("IoError".to_owned()));
                    map.insert("path".into(), Dynamic::from(String::new()));
                    map.insert(
                        "msg".into(),
                        Dynamic::from("env var not valid unicode".to_owned()),
                    );
                    Err(Box::new(EvalAltResult::ErrorRuntime(
                        Dynamic::from(map),
                        Position::NONE,
                    )))
                }
            }
        },
    );

    // to_json — serialise any Dynamic value to a JSON string.
    engine.register_fn(
        "to_json",
        |v: Dynamic| -> Result<String, Box<EvalAltResult>> {
            serde_json::to_string(&v).map_err(|e| {
                let mut map = Map::new();
                map.insert("kind".into(), Dynamic::from("SerializeError".to_owned()));
                map.insert("msg".into(), Dynamic::from(e.to_string()));
                Box::new(EvalAltResult::ErrorRuntime(
                    Dynamic::from(map),
                    Position::NONE,
                ))
            })
        },
    );
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

    // Helper: build an engine with a custom SecurityConfig.
    fn build_engine_with_security(security: crate::security::SecurityConfig) -> Engine {
        use crate::core::audit::AuditWriter;
        use crate::core::run_context::RunContext;
        use std::sync::{atomic::AtomicU32, Mutex};
        use tempfile::TempDir;

        let tmp = TempDir::new().expect("tempdir");
        let run_id = uuid::Uuid::new_v4().to_string();
        let audit = AuditWriter::open(tmp.path(), &run_id).expect("open audit");
        Box::leak(Box::new(tmp));
        let ctx = Arc::new(RunContext {
            security: Arc::new(security),
            audit: Arc::new(Mutex::new(audit)),
            exec_counter: Arc::new(AtomicU32::new(0)),
        });
        build_engine_with_args(Vec::new(), ctx)
    }

    // B6: env("AWS_SECRET_ACCESS_KEY") throws EnvDenied (key not in passthrough).
    #[test]
    fn env_denied_for_key_not_in_passthrough() {
        let engine = build_engine();
        let result = engine.run(r#"env("AWS_SECRET_ACCESS_KEY")"#);
        let err = result.unwrap_err();
        match err.as_ref() {
            EvalAltResult::ErrorRuntime(dyn_val, _) => {
                let map = dyn_val.clone().cast::<rhai::Map>();
                assert_eq!(
                    map.get("kind").unwrap().clone().cast::<String>(),
                    "EnvDenied"
                );
                assert_eq!(
                    map.get("key").unwrap().clone().cast::<String>(),
                    "AWS_SECRET_ACCESS_KEY"
                );
            }
            other => panic!("expected ErrorRuntime, got: {:?}", other),
        }
    }

    // B7: env("REEVE_TEST_UNSET_VAR") throws EnvUnset (key in passthrough but not set).
    #[test]
    fn env_unset_for_key_in_passthrough_but_not_set() {
        use crate::security::{AuditConfig, SecurityConfig};
        use tempfile::TempDir;

        let tmp = TempDir::new().expect("tempdir");
        let security = SecurityConfig {
            reeve_home: tmp.path().to_path_buf(),
            allowed_roots: vec![],
            deny_traversal: true,
            env_passthrough: vec!["REEVE_TEST_UNSET_VAR".to_string()],
            audit: AuditConfig {
                capture_command: false,
                capture_stdout: false,
                capture_stderr: false,
            },
        };
        // Ensure the variable is actually unset.
        std::env::remove_var("REEVE_TEST_UNSET_VAR");
        let engine = build_engine_with_security(security);
        let result = engine.run(r#"env("REEVE_TEST_UNSET_VAR")"#);
        let err = result.unwrap_err();
        match err.as_ref() {
            EvalAltResult::ErrorRuntime(dyn_val, _) => {
                let map = dyn_val.clone().cast::<rhai::Map>();
                assert_eq!(
                    map.get("kind").unwrap().clone().cast::<String>(),
                    "EnvUnset"
                );
                assert_eq!(
                    map.get("key").unwrap().clone().cast::<String>(),
                    "REEVE_TEST_UNSET_VAR"
                );
            }
            other => panic!("expected ErrorRuntime, got: {:?}", other),
        }
    }

    // H12: log_info emits a script_log audit event.
    #[test]
    fn log_info_emits_script_log_audit_event() {
        use crate::core::audit::AuditWriter;
        use crate::core::run_context::RunContext;
        use crate::security::{AuditConfig, SecurityConfig};
        use std::sync::{atomic::AtomicU32, Arc, Mutex};
        use tempfile::TempDir;

        let tmp = TempDir::new().expect("tempdir");
        let runs_dir = tmp.path().join("runs");
        std::fs::create_dir_all(&runs_dir).unwrap();

        let run_id = "test-run-h12";
        let writer = AuditWriter::open(&runs_dir, run_id).expect("open audit");
        let audit = Arc::new(Mutex::new(writer));

        let security = Arc::new(SecurityConfig {
            reeve_home: tmp.path().to_path_buf(),
            allowed_roots: vec![],
            deny_traversal: true,
            env_passthrough: vec!["PATH".to_string(), "HOME".to_string(), "LANG".to_string()],
            audit: AuditConfig {
                capture_command: false,
                capture_stdout: false,
                capture_stderr: false,
            },
        });

        let ctx = Arc::new(RunContext {
            security,
            audit: Arc::clone(&audit),
            exec_counter: Arc::new(AtomicU32::new(0)),
        });

        let engine = build_engine_with_args(vec![], ctx);
        engine.run(r#"log_info("hello audit")"#).unwrap();

        // Drop engine to allow audit arc to be released before reading.
        drop(engine);

        // Read the audit file.
        let audit_path = runs_dir.join(run_id).join("audit.jsonl");
        let content = std::fs::read_to_string(&audit_path).unwrap();

        let found = content.lines().any(|line| {
            let v: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
            v["event"] == "script_log" && v["level"] == "info" && v["msg"] == "hello audit"
        });
        assert!(
            found,
            "expected script_log event in audit, got:\n{}",
            content
        );
    }

    // H7: env("PATH") returns a non-empty string (PATH is in default passthrough).
    #[test]
    fn env_path_returns_value() {
        let engine = build_engine();
        let result: String = engine.eval(r#"env("PATH")"#).expect("env(PATH) should work");
        assert!(!result.is_empty(), "PATH should be non-empty");
    }

    // H6: env("HOME") returns a non-empty string (HOME is in default passthrough).
    #[test]
    fn env_home_returns_value() {
        let engine = build_engine();
        let result: String = engine.eval(r#"env("HOME")"#).expect("env(HOME) should succeed");
        assert!(!result.is_empty(), "HOME should be non-empty");
    }

    // H8: to_json(#{"x": 1}) returns a string that round-trips through serde_json.
    #[test]
    fn to_json_map_round_trips() {
        let engine = build_engine();
        let json: String = engine
            .eval(r#"to_json(#{"x": 1})"#)
            .expect("to_json should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("should parse as JSON");
        assert_eq!(parsed["x"], serde_json::json!(1));
    }

    // H9: to_json([1, 2, 3]) returns a string parseable as a JSON array.
    #[test]
    fn to_json_array_is_json_array() {
        let engine = build_engine();
        let json: String = engine
            .eval(r#"to_json([1, 2, 3])"#)
            .expect("to_json should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("should parse as JSON");
        assert!(parsed.is_array(), "result should be a JSON array");
        assert_eq!(parsed.as_array().unwrap().len(), 3);
    }
}
