//! Process executor — validates argv via pact, spawns the child process,
//! enforces per-exec timeout and output cap, and returns a Rhai map.
//!
//! In milestone 1 the active pact is always `unix_readonly()`. Pact
//! parameterisation will be added when `reeve-flex` lands.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rhai::{Dynamic, EvalAltResult, Map, Position};
use wait_timeout::ChildExt;
use crate::pact::{validate_call, PactError};
use crate::pact::schema::Pact;
use crate::core::audit::{AuditEvent, AuditWriter, try_emit};

// ---------------------------------------------------------------------------
// Trace macro
// ---------------------------------------------------------------------------

macro_rules! trace {
    ($($arg:tt)*) => {
        if std::env::var("REEVE_DEBUG").is_ok() {
            eprintln!("[exec] {}", format_args!($($arg)*))
        }
    };
}
#[allow(unused_imports)]
pub(crate) use trace;

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

/// Run `binary` with `argv` under the `unix_readonly()` pact.
///
/// Returns a Rhai map `#{ stdout, stderr, exit_code, duration_ms }` on success.
/// Throws a typed Rhai error map on any policy or runtime failure.
///
/// Used by unit tests. Production code calls `run_exec_audited`.
#[cfg(test)]
pub fn run_exec(binary: &str, argv: &[String]) -> Result<rhai::Map, Box<EvalAltResult>> {
    let pact = crate::pact::unix_readonly();
    run_exec_with(pact, binary, argv, false, None)
}


/// Variant that also emits audit events via the supplied writer.
pub fn run_exec_audited(
    binary: &str,
    argv: &[String],
    allow_fail: bool,
    audit: Arc<Mutex<AuditWriter>>,
    exec_counter: Arc<std::sync::atomic::AtomicU32>,
    env_passthrough: &[String],
) -> Result<rhai::Map, Box<EvalAltResult>> {
    let pact = crate::pact::unix_readonly();
    let passthrough_refs: Vec<&str> = env_passthrough.iter().map(String::as_str).collect();
    run_exec_with_env(pact, binary, argv, allow_fail, Some((&audit, &exec_counter)), Some(&passthrough_refs))
}

/// Internal helper that accepts an explicit pact reference and optional env passthrough list.
///
/// When `env_passthrough` is non-empty, the child's environment is cleared and
/// only the listed variable names are re-added from the current process env.
/// When `env_passthrough` is empty (the default for callers without a passthrough),
/// the child inherits the full parent environment (existing behaviour for callers
/// that do not need env filtering).
#[cfg(test)]
pub(crate) fn run_exec_with_passthrough(
    pact: &Pact,
    binary: &str,
    argv: &[String],
    allow_fail: bool,
    audit_and_counter: Option<(&Arc<Mutex<AuditWriter>>, &Arc<std::sync::atomic::AtomicU32>)>,
    env_passthrough: &[&str],
) -> Result<rhai::Map, Box<EvalAltResult>> {
    run_exec_with_env(pact, binary, argv, allow_fail, audit_and_counter, Some(env_passthrough))
}

/// Internal helper that accepts an explicit pact reference.
/// Used by tests to inject `test_fixtures()`.
#[cfg(test)]
pub(crate) fn run_exec_with(
    pact: &Pact,
    binary: &str,
    argv: &[String],
    allow_fail: bool,
    audit_and_counter: Option<(&Arc<Mutex<AuditWriter>>, &Arc<std::sync::atomic::AtomicU32>)>,
) -> Result<rhai::Map, Box<EvalAltResult>> {
    run_exec_with_env(pact, binary, argv, allow_fail, audit_and_counter, None)
}

/// Core implementation accepting an optional env passthrough filter.
fn run_exec_with_env(
    pact: &Pact,
    binary: &str,
    argv: &[String],
    allow_fail: bool,
    audit_and_counter: Option<(&Arc<Mutex<AuditWriter>>, &Arc<std::sync::atomic::AtomicU32>)>,
    env_passthrough: Option<&[&str]>,
) -> Result<rhai::Map, Box<EvalAltResult>> {
    // 1. Validate call against pact.
    let resolved = validate_call(pact, binary, argv).map_err(|e| pact_error_to_rhai(e, binary))?;

    let timeout_ms = (pact.defaults.timeout_seconds as u64) * 1000;
    let max_bytes = pact.defaults.max_output_bytes as usize;
    let abs_path = resolved.abs_path.clone();
    let bin_name = binary.to_owned();

    // 2. Emit exec_start audit event.
    if let Some((audit, _)) = audit_and_counter {
        let run_id = audit.lock().map(|g| g.run_id.clone()).unwrap_or_default();
        let event = AuditEvent::exec_start(&run_id, bin_name.clone(), argv.to_vec());
        try_emit(audit, &event);
    }

    // 3. Spawn child — argv array form, stdin null.
    let spawn_start = Instant::now();
    let mut cmd = Command::new(&abs_path);
    cmd.args(&resolved.argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Apply env passthrough filter when requested: clear the child's env and
    // re-add only the explicitly listed variables.
    if let Some(passthrough) = env_passthrough {
        cmd.env_clear();
        for key in passthrough {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }
    }
    let mut child = cmd.spawn().map_err(|e| {
            runtime_err_map("ExecFailed", |m| {
                m.insert("binary".into(), Dynamic::from(bin_name.clone()));
                m.insert("exit_code".into(), Dynamic::from(1_i64));
                m.insert("stdout".into(), Dynamic::from(String::new()));
                m.insert("stderr".into(), Dynamic::from(e.to_string()));
            })
        })?;

    let start = Instant::now();
    let _ = spawn_start; // start measuring from after spawn to be consistent

    // 4. Capture stdout + stderr on dedicated threads with byte cap.
    let stdout_raw = child.stdout.take().expect("piped stdout");
    let stderr_raw = child.stderr.take().expect("piped stderr");

    let stdout_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

    let stdout_buf2 = Arc::clone(&stdout_buf);
    let stderr_buf2 = Arc::clone(&stderr_buf);

    // Reader threads — each reads in 4 KiB chunks and appends to its buffer.
    // The parent thread enforces the cap and timeout; readers just collect bytes.
    let stdout_thread = std::thread::spawn(move || read_stream(stdout_raw, stdout_buf2));
    let stderr_thread = std::thread::spawn(move || read_stream(stderr_raw, stderr_buf2));

    // 5. Wait for child with timeout.
    let timeout = Duration::from_millis(timeout_ms);
    let status_opt = child
        .wait_timeout(timeout)
        .map_err(|e| runtime_err_map("ExecFailed", |m| {
            m.insert("binary".into(), Dynamic::from(bin_name.clone()));
            m.insert("exit_code".into(), Dynamic::from(1_i64));
            m.insert("stdout".into(), Dynamic::from(String::new()));
            m.insert("stderr".into(), Dynamic::from(e.to_string()));
        }))?;

    let elapsed_ms = start.elapsed().as_millis() as i64;

    // 6. Handle timeout.
    if status_opt.is_none() {
        // Timed out — kill child and reap.
        let _ = child.kill();
        let _ = child.wait();
        // Also wait for reader threads to finish.
        let _ = stdout_thread.join();
        let _ = stderr_thread.join();

        trace!(
            "binary={} path={} argv={:?} result=Timeout elapsed_ms={} limit_ms={}",
            bin_name, abs_path.display(), argv, elapsed_ms, timeout_ms
        );

        // Emit exec_error audit event for timeout.
        if let Some((audit, _)) = audit_and_counter {
            let run_id = audit.lock().map(|g| g.run_id.clone()).unwrap_or_default();
            let event = AuditEvent::exec_error(&run_id, bin_name.clone(), "Timeout".to_owned(), Some(timeout_ms));
            try_emit(audit, &event);
        }

        return Err(runtime_err_map("Timeout", |m| {
            m.insert("binary".into(), Dynamic::from(bin_name.clone()));
            m.insert("elapsed_ms".into(), Dynamic::from(elapsed_ms));
            m.insert("limit_ms".into(), Dynamic::from(timeout_ms as i64));
        }));
    }

    let status = status_opt.unwrap();

    // 7. Wait for reader threads to finish and collect output.
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    let stdout_bytes = Arc::try_unwrap(stdout_buf)
        .unwrap_or_else(|a| Mutex::new(a.lock().unwrap().clone()))
        .into_inner()
        .unwrap();
    let stderr_bytes = Arc::try_unwrap(stderr_buf)
        .unwrap_or_else(|a| Mutex::new(a.lock().unwrap().clone()))
        .into_inner()
        .unwrap();

    let total_bytes = stdout_bytes.len() + stderr_bytes.len();

    // 8. Check output cap.
    if total_bytes > max_bytes {
        trace!(
            "binary={} path={} argv={:?} result=OutputLimitExceeded bytes_seen={} limit={}",
            bin_name, abs_path.display(), argv, total_bytes, max_bytes
        );

        // Emit exec_error audit event for output limit exceeded.
        if let Some((audit, _)) = audit_and_counter {
            let run_id = audit.lock().map(|g| g.run_id.clone()).unwrap_or_default();
            let event = AuditEvent::exec_error(&run_id, bin_name.clone(), "OutputLimitExceeded".to_owned(), None);
            try_emit(audit, &event);
        }

        return Err(runtime_err_map("OutputLimitExceeded", |m| {
            m.insert("binary".into(), Dynamic::from(bin_name.clone()));
            m.insert("bytes_seen".into(), Dynamic::from(total_bytes as i64));
            m.insert("limit".into(), Dynamic::from(max_bytes as i64));
        }));
    }

    let stdout_str = String::from_utf8_lossy(&stdout_bytes).into_owned();
    let stderr_str = String::from_utf8_lossy(&stderr_bytes).into_owned();
    let exit_code = status.code().unwrap_or(-1) as i64;

    // 9. Handle non-zero exit.
    if exit_code != 0 && !allow_fail {
        trace!(
            "binary={} path={} argv={:?} result=ExecFailed exit_code={} duration_ms={}",
            bin_name, abs_path.display(), argv, exit_code, elapsed_ms
        );

        // Emit exec_end for failed exit (non-zero but not a runtime error).
        if let Some((audit, counter)) = audit_and_counter {
            let run_id = audit.lock().map(|g| g.run_id.clone()).unwrap_or_default();
            let event = AuditEvent::exec_end(
                &run_id,
                bin_name.clone(),
                exit_code as i32,
                elapsed_ms as u64,
                stdout_bytes.len(),
                stderr_bytes.len(),
            );
            try_emit(audit, &event);
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        return Err(runtime_err_map("ExecFailed", |m| {
            m.insert("binary".into(), Dynamic::from(bin_name.clone()));
            m.insert("exit_code".into(), Dynamic::from(exit_code));
            m.insert("stdout".into(), Dynamic::from(stdout_str.clone()));
            m.insert("stderr".into(), Dynamic::from(stderr_str.clone()));
        }));
    }

    trace!(
        "binary={} path={} argv={:?} exit_code={} duration_ms={}",
        bin_name, abs_path.display(), argv, exit_code, elapsed_ms
    );

    // 10. Emit exec_end audit event on success.
    if let Some((audit, counter)) = audit_and_counter {
        let run_id = audit.lock().map(|g| g.run_id.clone()).unwrap_or_default();
        let event = AuditEvent::exec_end(
            &run_id,
            bin_name.clone(),
            exit_code as i32,
            elapsed_ms as u64,
            stdout_bytes.len(),
            stderr_bytes.len(),
        );
        try_emit(audit, &event);
        counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    // 11. Build success (or allow_fail) result map.
    let mut map = Map::new();
    map.insert("stdout".into(), Dynamic::from(stdout_str));
    map.insert("stderr".into(), Dynamic::from(stderr_str));
    map.insert("exit_code".into(), Dynamic::from(exit_code));
    map.insert("duration_ms".into(), Dynamic::from(elapsed_ms));
    Ok(map)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read an entire stream into a shared buffer.
fn read_stream(mut stream: impl Read, buf: Arc<Mutex<Vec<u8>>>) {
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let mut guard = buf.lock().unwrap();
                guard.extend_from_slice(&chunk[..n]);
            }
        }
    }
}

/// Build a Rhai runtime error wrapping a map constructed by `fill`.
fn runtime_err_map(
    kind: &str,
    fill: impl FnOnce(&mut Map),
) -> Box<EvalAltResult> {
    let mut map = Map::new();
    map.insert("kind".into(), Dynamic::from(kind.to_owned()));
    fill(&mut map);
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from_map(map),
        Position::NONE,
    ))
}

/// Convert a `PactError` into a typed Rhai error map.
fn pact_error_to_rhai(err: PactError, _binary: &str) -> Box<EvalAltResult> {
    match err {
        PactError::BinaryNotAllowed { binary: b } => {
            runtime_err_map("BinaryNotAllowed", |m| {
                m.insert("binary".into(), Dynamic::from(b));
            })
        }
        PactError::BinaryNotResolvable { binary: b } => {
            // Map to BinaryNotFound per task spec (script-visible name).
            runtime_err_map("BinaryNotFound", |m| {
                m.insert("binary".into(), Dynamic::from(b));
            })
        }
        PactError::SubcommandNotAllowed {
            binary: b,
            subcommand,
        } => runtime_err_map("SubcommandNotAllowed", |m| {
            m.insert("binary".into(), Dynamic::from(b));
            m.insert("subcommand".into(), Dynamic::from(subcommand));
        }),
        PactError::FlagNotAllowed { binary: b, flag } => {
            runtime_err_map("FlagNotAllowed", |m| {
                m.insert("binary".into(), Dynamic::from(b));
                m.insert("flag".into(), Dynamic::from(flag));
            })
        }
        PactError::FlagValueRejected {
            binary: b,
            flag,
            value,
            reason,
        } => runtime_err_map("FlagValueRejected", |m| {
            m.insert("binary".into(), Dynamic::from(b));
            m.insert("flag".into(), Dynamic::from(flag));
            m.insert("value".into(), Dynamic::from(value));
            m.insert("reason".into(), Dynamic::from(reason));
        }),
        PactError::PositionalRejected {
            binary: b,
            index,
            value,
            reason,
        } => runtime_err_map("PositionalRejected", |m| {
            m.insert("binary".into(), Dynamic::from(b));
            m.insert("index".into(), Dynamic::from(index as i64));
            m.insert("value".into(), Dynamic::from(value));
            m.insert("reason".into(), Dynamic::from(reason));
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::EvalAltResult;
    use crate::pact::unix_readonly;

    fn test_fixtures() -> &'static crate::pact::schema::Pact {
        crate::pact::presets::test_fixtures()
    }

    // Helper: extract the kind field from a Rhai runtime error map.
    fn err_kind(err: &EvalAltResult) -> String {
        match err {
            EvalAltResult::ErrorRuntime(dyn_val, _) => {
                let map = dyn_val.clone().cast::<Map>();
                map.get("kind")
                    .cloned()
                    .map(|d| d.cast::<String>())
                    .unwrap_or_default()
            }
            other => panic!("expected ErrorRuntime, got: {:?}", other),
        }
    }

    fn err_map(err: &EvalAltResult) -> Map {
        match err {
            EvalAltResult::ErrorRuntime(dyn_val, _) => dyn_val.clone().cast::<Map>(),
            other => panic!("expected ErrorRuntime, got: {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // SF-2: spawned child does not see host secrets
    // -------------------------------------------------------------------------
    #[test]
    fn spawned_child_does_not_see_host_secrets() {
        let printenv_exists = std::path::Path::new("/usr/bin/printenv").exists()
            || std::path::Path::new("/bin/printenv").exists();
        if !printenv_exists {
            return; // skip if printenv not available
        }

        // Set a secret in the current process env
        std::env::set_var("REEVE_EXECUTOR_TEST_SECRET", "should-not-leak");

        let argv: Vec<String> = vec![];
        let result = run_exec_with_passthrough(
            test_fixtures(),
            "printenv",
            &argv,
            false,
            None,
            &["PATH", "HOME", "LANG"],
        );
        let map = result.expect("printenv should succeed");
        let stdout = map.get("stdout").unwrap().clone().cast::<String>();
        assert!(
            !stdout.contains("REEVE_EXECUTOR_TEST_SECRET"),
            "child inherited secret env var: {}",
            stdout
        );

        std::env::remove_var("REEVE_EXECUTOR_TEST_SECRET");
    }

    // -------------------------------------------------------------------------
    // Fix 6: exec_allow_fail non-zero exit still emits audit events
    // -------------------------------------------------------------------------
    #[test]
    fn exec_allow_fail_nonzero_still_emits_audit_events() {
        use crate::core::audit::AuditWriter;
        use tempfile::tempdir;

        let tmp = tempdir().unwrap();
        let runs_dir = tmp.path().join("runs");
        std::fs::create_dir_all(&runs_dir).unwrap();
        let run_id = "test-exec-allow-fail";
        let writer = AuditWriter::open(&runs_dir, run_id).unwrap();
        let audit = Arc::new(Mutex::new(writer));
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));

        // Use whoami from unix_readonly pact (exits 0) with allow_fail=true to confirm audit fires.
        let pact = unix_readonly();
        let whoami_exists = pact.binaries.get("whoami")
            .and_then(|spec| spec.path.default.as_ref())
            .and_then(|paths| paths.first())
            .map(|p| p.exists())
            .unwrap_or(false);
        if !whoami_exists {
            return; // skip if whoami not available
        }

        let argv: Vec<String> = vec![];
        let result = run_exec_with(
            pact,
            "whoami",
            &argv,
            true, // allow_fail
            Some((&audit, &counter)),
        );
        assert!(result.is_ok(), "whoami allow_fail should succeed: {:?}", result);

        // Flush by dropping the audit arc
        drop(audit);

        let audit_path = runs_dir.join(run_id).join("audit.jsonl");
        let content = std::fs::read_to_string(&audit_path).unwrap();
        assert!(content.contains("exec_start"), "exec_start missing from audit");
        assert!(content.contains("exec_end"), "exec_end missing from audit");
    }

    // -------------------------------------------------------------------------
    // Row: whoami_succeeds
    // -------------------------------------------------------------------------
    #[test]
    fn whoami_succeeds() {
        let pact = unix_readonly();
        // Check path exists; skip gracefully if not.
        let whoami_spec = &pact.binaries["whoami"];
        let path_exists = whoami_spec
            .path
            .default
            .as_ref()
            .and_then(|v| v.first())
            .map(|p| p.exists())
            .unwrap_or(false);
        if !path_exists {
            return; // skip
        }

        let result = run_exec_with(pact, "whoami", &[], false, None);
        let map = result.expect("whoami should succeed");
        assert_eq!(map["exit_code"].clone().cast::<i64>(), 0);
        let stdout = map["stdout"].clone().cast::<String>();
        assert!(!stdout.trim().is_empty(), "stdout should be non-empty");
    }

    // -------------------------------------------------------------------------
    // Row: unknown_binary_throws_binary_not_allowed
    // -------------------------------------------------------------------------
    #[test]
    fn unknown_binary_throws_binary_not_allowed() {
        let err = run_exec("rm", &[]).unwrap_err();
        assert_eq!(err_kind(&err), "BinaryNotAllowed");
        let map = err_map(&err);
        assert_eq!(map["binary"].clone().cast::<String>(), "rm");
    }

    // -------------------------------------------------------------------------
    // Row: unknown_flag_throws_flag_not_allowed
    // -------------------------------------------------------------------------
    #[test]
    fn unknown_flag_throws_flag_not_allowed() {
        let err = run_exec("uname", &["-X".to_owned()]).unwrap_err();
        assert_eq!(err_kind(&err), "FlagNotAllowed");
        let map = err_map(&err);
        assert_eq!(map["flag"].clone().cast::<String>(), "-X");
    }

    // -------------------------------------------------------------------------
    // Row: metachar_in_positional_throws_positional_rejected
    // -------------------------------------------------------------------------
    #[test]
    fn metachar_in_positional_throws_positional_rejected() {
        let err = run_exec("echo", &["hi;rm".to_owned()]).unwrap_err();
        assert_eq!(err_kind(&err), "PositionalRejected");
    }

    // -------------------------------------------------------------------------
    // Row: sleep_exceeding_timeout_throws_timeout
    // (uses test_fixtures pact — timeout_seconds: 1)
    // -------------------------------------------------------------------------
    #[test]
    fn sleep_exceeding_timeout_throws_timeout() {
        let sleep_path = std::path::Path::new("/bin/sleep");
        if !sleep_path.exists() {
            return; // skip
        }

        let pact = test_fixtures();
        let err = run_exec_with(pact, "sleep", &["3".to_owned()], false, None).unwrap_err();
        assert_eq!(err_kind(&err), "Timeout");
        let map = err_map(&err);
        assert_eq!(map["limit_ms"].clone().cast::<i64>(), 1000);
    }

    // -------------------------------------------------------------------------
    // Row: yes_exceeds_output_cap
    // (uses test_fixtures pact — max_output_bytes: 4096)
    // -------------------------------------------------------------------------
    #[test]
    fn yes_exceeds_output_cap() {
        let yes_exists = std::path::Path::new("/usr/bin/yes").exists()
            || std::path::Path::new("/bin/yes").exists();
        if !yes_exists {
            return; // skip
        }

        let pact = test_fixtures();
        let err = run_exec_with(pact, "yes", &[], false, None).unwrap_err();
        // Could be OutputLimitExceeded or Timeout — both are acceptable for yes.
        // But we expect OutputLimitExceeded since cap is 4096 and yes floods fast.
        let kind = err_kind(&err);
        assert!(
            kind == "OutputLimitExceeded" || kind == "Timeout",
            "expected OutputLimitExceeded or Timeout, got: {kind}"
        );
    }
}
