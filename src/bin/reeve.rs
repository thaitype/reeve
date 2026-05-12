use std::{path::PathBuf, process::ExitCode, sync::{Arc, Mutex}};

use clap::{Parser, Subcommand};
use rhai::{Dynamic, EvalAltResult, Map};

use reeve::{
    security::SecurityConfig,
    core::{home::init_home, audit::AuditWriter, run_context::RunContext},
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
    let security = Arc::new(security);
    let audit = Arc::new(Mutex::new(AuditWriter));
    let _ctx = Arc::new(RunContext { security, audit });

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

            // Step 2 — build engine with forwarded args
            let engine = reeve::build_engine_with_args(script_args);

            // Step 3 — run script
            match engine.run(&script_source) {
                Ok(()) => ExitCode::SUCCESS,
                Err(eval_err) => {
                    let (code, msg) = classify_error(&eval_err);
                    eprintln!("error: {msg}");
                    code
                }
            }
        }
    }
}
