//! JSONL audit log — `AuditWriter`, `AuditEvent`, and `AuditError`.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
#[error("audit io error: {0}")]
pub struct AuditError(#[from] std::io::Error);

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// All events share `ts` and `run_id`. Variant-specific fields are inlined.
#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AuditEvent {
    ScriptStart {
        ts: String,
        run_id: String,
        script_path: String,
        args: Vec<String>,
    },
    ExecStart {
        ts: String,
        run_id: String,
        binary: String,
        argv: Vec<String>,
    },
    ExecEnd {
        ts: String,
        run_id: String,
        binary: String,
        exit_code: i32,
        duration_ms: u64,
        stdout_bytes: usize,
        stderr_bytes: usize,
    },
    ScriptLog {
        ts: String,
        run_id: String,
        level: String,
        msg: String,
    },
    ScriptEnd {
        ts: String,
        run_id: String,
        exit_status: String,
        duration_ms: u64,
        exec_count: u32,
    },
}

impl AuditEvent {
    /// Returns a timestamp string in RFC 3339 with millisecond precision.
    pub fn now_ts() -> String {
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    pub fn script_start(run_id: &str, script_path: String, args: Vec<String>) -> Self {
        Self::ScriptStart {
            ts: Self::now_ts(),
            run_id: run_id.to_owned(),
            script_path,
            args,
        }
    }

    pub fn exec_start(run_id: &str, binary: String, argv: Vec<String>) -> Self {
        Self::ExecStart {
            ts: Self::now_ts(),
            run_id: run_id.to_owned(),
            binary,
            argv,
        }
    }

    pub fn exec_end(
        run_id: &str,
        binary: String,
        exit_code: i32,
        duration_ms: u64,
        stdout_bytes: usize,
        stderr_bytes: usize,
    ) -> Self {
        Self::ExecEnd {
            ts: Self::now_ts(),
            run_id: run_id.to_owned(),
            binary,
            exit_code,
            duration_ms,
            stdout_bytes,
            stderr_bytes,
        }
    }

    pub fn script_log(run_id: &str, level: String, msg: String) -> Self {
        Self::ScriptLog {
            ts: Self::now_ts(),
            run_id: run_id.to_owned(),
            level,
            msg,
        }
    }

    pub fn script_end(run_id: &str, exit_status: String, duration_ms: u64, exec_count: u32) -> Self {
        Self::ScriptEnd {
            ts: Self::now_ts(),
            run_id: run_id.to_owned(),
            exit_status,
            duration_ms,
            exec_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

pub struct AuditWriter {
    file: BufWriter<File>,
    pub run_id: String,
}

impl AuditWriter {
    /// Creates `runs_dir/<run_id>/audit.jsonl` and returns an `AuditWriter`.
    pub fn open(runs_dir: &Path, run_id: &str) -> Result<Self, AuditError> {
        let run_dir = runs_dir.join(run_id);
        fs::create_dir_all(&run_dir)?;
        let file = File::create(run_dir.join("audit.jsonl"))?;
        Ok(Self {
            file: BufWriter::new(file),
            run_id: run_id.to_owned(),
        })
    }

    /// Serialises `event` to one JSON line + newline, then flushes.
    pub fn emit(&mut self, event: &AuditEvent) -> Result<(), AuditError> {
        let line = serde_json::to_string(event)
            .map_err(std::io::Error::other)?;
        self.file.write_all(line.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.file.flush()?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helper — emit with warn-on-error semantics
// ---------------------------------------------------------------------------

/// Emit an event via a locked `AuditWriter`. On failure, print a WARN to
/// stderr and continue — audit is best-effort.
pub fn try_emit(writer: &std::sync::Mutex<AuditWriter>, event: &AuditEvent) {
    let mut guard = match writer.lock() {
        Ok(g) => g,
        Err(_) => return, // poisoned — best-effort
    };
    if let Err(e) = guard.emit(event) {
        eprintln!("WARN: audit write failed: {e}");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_runs_dir() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn open_creates_run_dir_and_file() {
        let tmp = temp_runs_dir();
        let run_id = "test-run-1";
        let _writer = AuditWriter::open(tmp.path(), run_id).expect("open should succeed");
        let audit_path = tmp.path().join(run_id).join("audit.jsonl");
        assert!(audit_path.exists(), "audit.jsonl should exist after open()");
    }

    #[test]
    fn emit_writes_valid_json_line() {
        let tmp = temp_runs_dir();
        let run_id = "test-run-emit";
        let mut writer = AuditWriter::open(tmp.path(), run_id).expect("open");

        let event = AuditEvent::script_start(run_id, "/tmp/test.rhai".to_owned(), vec![]);
        writer.emit(&event).expect("emit");

        let contents = std::fs::read_to_string(tmp.path().join(run_id).join("audit.jsonl"))
            .expect("read audit.jsonl");
        let line = contents.trim();
        let val: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
        assert_eq!(val["event"], "script_start");
        assert!(val["ts"].is_string(), "ts should be a string");
        assert_eq!(val["run_id"], run_id);
    }

    #[test]
    fn emit_flushes_after_each_call() {
        let tmp = temp_runs_dir();
        let run_id = "test-run-flush";
        let mut writer = AuditWriter::open(tmp.path(), run_id).expect("open");

        let event = AuditEvent::exec_start(run_id, "whoami".to_owned(), vec![]);
        writer.emit(&event).expect("emit");

        // File should be readable immediately — no explicit flush needed
        let audit_path = tmp.path().join(run_id).join("audit.jsonl");
        let contents = std::fs::read_to_string(&audit_path).expect("read");
        assert!(!contents.is_empty(), "file should be non-empty after emit without explicit flush");
    }

    #[test]
    fn emit_script_start_then_end_produces_two_parseable_lines() {
        let tmp = temp_runs_dir();
        let run_id = "test-run-two";
        let mut writer = AuditWriter::open(tmp.path(), run_id).expect("open");

        let start = AuditEvent::script_start(run_id, "/script.rhai".to_owned(), vec![]);
        let end = AuditEvent::script_end(run_id, "ok".to_owned(), 100, 2);
        writer.emit(&start).expect("emit start");
        writer.emit(&end).expect("emit end");

        let contents = std::fs::read_to_string(tmp.path().join(run_id).join("audit.jsonl"))
            .expect("read");
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2, "expected 2 lines");

        let v0: serde_json::Value = serde_json::from_str(lines[0]).expect("line 0 valid JSON");
        let v1: serde_json::Value = serde_json::from_str(lines[1]).expect("line 1 valid JSON");
        assert_eq!(v0["event"], "script_start");
        assert_eq!(v1["event"], "script_end");
    }
}
