use std::{
    path::PathBuf,
    process::ExitCode,
    sync::{atomic::AtomicU32, Arc, Mutex},
    time::Instant,
};

use clap::{Parser, Subcommand};
use rhai::{Dynamic, EvalAltResult, Map};
use uuid::Uuid;

use reeve::{
    security::SecurityConfig,
    core::{
        audit::{AuditEvent, AuditWriter},
        home::init_home,
        run_context::RunContext,
    },
};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "reeve", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a Rhai script
    Run {
        /// Path to .rhai script
        script: PathBuf,
        /// Args passed through to script_args() inside the script
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        script_args: Vec<String>,
    },
    /// Print version
    Version,
}

// ---------------------------------------------------------------------------
// Error classification
// ---------------------------------------------------------------------------

/// Pact-violation error kinds that map to exit code 2.
const PACT_VIOLATION_KINDS: &[&str] = &[
    "BinaryNotAllowed",
    "BinaryNotFound",
    "SubcommandNotAllowed",
    "FlagNotAllowed",
    "FlagValueRejected",
    "PositionalRejected",
];

/// Classify a Rhai eval error into (exit_code, human-readable message).
///
/// Exit code semantics (per spec-v2 §"Exit codes", collapsed for milestone 1):
///   0 — success
///   1 — script error (runtime / parse / type errors)
///   2 — pact violation
///   3 — config error (handled before reaching the engine)
pub fn classify_error(err: &EvalAltResult) -> (ExitCode, String) {
    if let EvalAltResult::ErrorRuntime(dyn_val, _) = err {
        if let Some(map) = try_as_map(dyn_val) {
            let kind = map_str(&map, "kind");
            if PACT_VIOLATION_KINDS.contains(&kind.as_str()) {
                let msg = format_map_error(&kind, &map);
                return (ExitCode::from(2), msg);
            }
            // Runtime map with a non-pact kind → script error (exit 1)
            let msg = format_map_error(&kind, &map);
            return (ExitCode::from(1), msg);
        }
    }
    // All other EvalAltResult variants → script error (exit 1)
    (ExitCode::from(1), format!("{err}"))
}

/// Try to cast a `Dynamic` to a `Map`.
fn try_as_map(dyn_val: &Dynamic) -> Option<Map> {
    if dyn_val.is::<Map>() {
        Some(dyn_val.clone().cast::<Map>())
    } else {
        None
    }
}

/// Extract a string field from a map, returning empty string if absent.
fn map_str(map: &Map, key: &str) -> String {
    map.get(key)
        .map(|v| {
            if v.is::<String>() {
                v.clone().cast::<String>()
            } else {
                v.to_string()
            }
        })
        .unwrap_or_default()
}

/// Format a runtime error map as `<kind>: key=value key=value ...`.
fn format_map_error(kind: &str, map: &Map) -> String {
    let mut parts: Vec<String> = map
        .iter()
        .filter(|(k, _)| k.as_str() != "kind")
        .map(|(k, v)| {
            if v.is::<String>() {
                format!("{}={}", k, v.clone().cast::<String>())
            } else {
                format!("{}={}", k, v)
            }
        })
        .collect();
    parts.sort(); // deterministic ordering for tests
    if parts.is_empty() {
        kind.to_owned()
    } else {
        format!("{}: {}", kind, parts.join(" "))
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Step 0 — load security config and initialise home directory (exit 3 on error)
    let security = match SecurityConfig::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("error: failed to load security config: {e}");
            return ExitCode::from(3);
        }
    };
    if let Err(e) = init_home(&security.reeve_home) {
        eprintln!("error: failed to initialise reeve home: {e}");
        return ExitCode::from(3);
    }

    let run_id = Uuid::new_v4().to_string();
    let runs_dir = security.reeve_home.join("runs");
    let audit = match AuditWriter::open(&runs_dir, &run_id) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("WARN: could not open audit log: {e}");
            // Create a fallback in a temp dir so the rest of the code doesn't need to handle Option.
            let tmp = std::env::temp_dir().join("reeve-audit-fallback");
            let _ = std::fs::create_dir_all(&tmp);
            match AuditWriter::open(&tmp, &run_id) {
                Ok(w) => w,
                Err(_) => {
                    // Give up on audit entirely — use a dummy file in /tmp.
                    let fallback = std::env::temp_dir().join("reeve-audit-noop");
                    let _ = std::fs::create_dir_all(&fallback);
                    AuditWriter::open(&fallback, &run_id)
                        .expect("fallback audit open should always succeed")
                }
            }
        }
    };

    let security = Arc::new(security);
    let exec_counter = Arc::new(AtomicU32::new(0));
    let audit = Arc::new(Mutex::new(audit));
    let ctx = Arc::new(RunContext {
        security: Arc::clone(&security),
        audit: Arc::clone(&audit),
        exec_counter: Arc::clone(&exec_counter),
    });

    match cli.cmd {
        Cmd::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }

        Cmd::Run { script, script_args } => {
            // Step 1 — read script source (config error → exit 3)
            let script_source = match std::fs::read_to_string(&script) {
                Ok(src) => src,
                Err(e) => {
                    eprintln!("error: cannot read script: {}: {}", script.display(), e);
                    return ExitCode::from(3);
                }
            };

            // Step 2 — canonicalize script path for audit log (D4)
            let script_path_str = std::fs::canonicalize(&script)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| script.to_string_lossy().into_owned());

            // Step 3 — emit script_start
            {
                let event = AuditEvent::script_start(&run_id, script_path_str, script_args.clone());
                let mut guard = audit.lock().expect("audit lock");
                if let Err(e) = guard.emit(&event) {
                    eprintln!("WARN: audit write failed: {e}");
                }
            }

            let script_start = Instant::now();

            // Step 4 — build engine with forwarded args and RunContext
            let engine = reeve::build_engine_with_args(script_args, Arc::clone(&ctx));

            // Step 5 — run script
            let result = engine.run(&script_source);

            let duration_ms = script_start.elapsed().as_millis() as u64;
            let exec_count = exec_counter.load(std::sync::atomic::Ordering::Relaxed);

            match result {
                Ok(()) => {
                    // emit script_end with ok status
                    let event = AuditEvent::script_end(&run_id, "ok".to_owned(), duration_ms, exec_count);
                    let mut guard = audit.lock().expect("audit lock");
                    if let Err(e) = guard.emit(&event) {
                        eprintln!("WARN: audit write failed: {e}");
                    }
                    ExitCode::SUCCESS
                }
                Err(eval_err) => {
                    let (code, msg) = classify_error(&eval_err);
                    eprintln!("error: {msg}");
                    // emit script_end with error status
                    let event = AuditEvent::script_end(&run_id, "error".to_owned(), duration_ms, exec_count);
                    let mut guard = audit.lock().expect("audit lock");
                    if let Err(e) = guard.emit(&event) {
                        eprintln!("WARN: audit write failed: {e}");
                    }
                    code
                }
            }
        }
    }
}
