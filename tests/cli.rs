use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;
use std::fs;

fn reeve() -> Command {
    Command::cargo_bin("reeve").expect("reeve binary should be built")
}

fn write_temp_script(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("create temp file");
    f.write_all(content.as_bytes())
        .expect("write temp script");
    f
}

// ---------------------------------------------------------------------------
// version subcommand
// ---------------------------------------------------------------------------

#[test]
fn version_subcommand_prints_version() {
    reeve()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"\d+\.\d+\.\d+").unwrap());
}

// ---------------------------------------------------------------------------
// Row #12 — unknown flag rejected by clap
//
// With trailing_var_arg=true, args AFTER the script path are captured as
// script_args.  But a flag placed BEFORE the script path (where clap still
// owns argument parsing) will be rejected with a parse error.
// "reeve run --pact x.yaml <script>" → clap unknown-argument error, exit ≠ 0.
// ---------------------------------------------------------------------------

#[test]
fn unknown_flag_pact_rejected_by_clap() {
    let script = write_temp_script(r#"print("hello");"#);
    // --pact is placed before the script path so clap's normal parser sees it
    // and rejects it (trailing_var_arg only captures args after the positional).
    reeve()
        .arg("run")
        .arg("--pact")
        .arg("x.yaml")
        .arg(script.path())
        .assert()
        .failure(); // clap exits non-zero for unknown arguments
}

// ---------------------------------------------------------------------------
// Row #13 — missing script file → exit 3
// ---------------------------------------------------------------------------

#[test]
fn missing_script_file_exits_3() {
    reeve()
        .arg("run")
        .arg("/nonexistent/path/no-such-script.rhai")
        .assert()
        .code(3)
        .stderr(predicate::str::contains("cannot read"));
}

// ---------------------------------------------------------------------------
// Happy path — simple print script → exit 0
// ---------------------------------------------------------------------------

#[test]
fn runs_simple_script() {
    let script = write_temp_script(r#"print("hi");"#);
    reeve()
        .arg("run")
        .arg(script.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("hi"));
}

// ---------------------------------------------------------------------------
// Pact violation → exit 2
// ---------------------------------------------------------------------------

#[test]
fn pact_violation_exits_2() {
    // "rm" is not in unix-readonly pact → BinaryNotAllowed → exit 2
    let script = write_temp_script(r#"exec("rm", []);"#);
    reeve()
        .arg("run")
        .arg(script.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("BinaryNotAllowed"));
}

// ---------------------------------------------------------------------------
// Row #14 — sysinfo happy path end-to-end
// ---------------------------------------------------------------------------

#[test]
fn examples_sysinfo_runs_end_to_end() {
    // CARGO_MANIFEST_DIR is the project root after the flat-layout move.
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/sysinfo.rhai");

    let assert = reeve()
        .arg("run")
        .arg(&script)
        .assert()
        .success()
        .stdout(predicate::str::contains("=== sysinfo ==="))
        .stdout(predicate::str::contains("user:"))
        .stdout(predicate::str::contains("host:"))
        .stdout(predicate::str::contains("kernel:"))
        .stdout(predicate::str::contains("date:"));

    // user: line must contain a non-empty username
    assert.stdout(predicate::str::is_match(r"user:\s+\S+").unwrap());
}

// ---------------------------------------------------------------------------
// script_args passthrough
// ---------------------------------------------------------------------------

#[test]
fn script_args_passthrough() {
    let script = write_temp_script(r#"print(script_args()[0]);"#);
    reeve()
        .arg("run")
        .arg(script.path())
        .arg("foo")
        .arg("bar")
        .assert()
        .success()
        .stdout(predicate::str::contains("foo"));
}

// ---------------------------------------------------------------------------
// N1 — workspace-demo runs clean
// ---------------------------------------------------------------------------

#[test]
fn n1_workspace_demo_runs_clean() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/workspace-demo.rhai");

    // Remove demo-output.txt from workspace so write_file succeeds on first run.
    let home = std::env::var("HOME").expect("HOME must be set");
    let workspace_file = Path::new(&home)
        .join(".reeve")
        .join("workspace")
        .join("demo-output.txt");
    let _ = fs::remove_file(&workspace_file);

    reeve()
        .arg("run")
        .arg(&script)
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// H10–H14 — Audit log presence and content (uses sysinfo.rhai)
// ---------------------------------------------------------------------------

#[test]
fn h10_h14_audit_log_after_sysinfo() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/sysinfo.rhai");

    let home = std::env::var("HOME").expect("HOME must be set");
    let runs_dir = Path::new(&home).join(".reeve").join("runs");

    // Run sysinfo so a fresh run dir is created.
    reeve()
        .arg("run")
        .arg(&script)
        .assert()
        .success();

    // Find the run dir that contains an exec_start event for "whoami" (from sysinfo).
    // We scan all run dirs and pick the one whose audit.jsonl has exec_start with binary=whoami.
    let audit_path = {
        let mut found = None;
        let entries: Vec<_> = fs::read_dir(&runs_dir)
            .expect("runs dir should exist")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        // Sort newest-first by modified time to find the sysinfo run quickly.
        let mut entries = entries;
        entries.sort_by_key(|e| {
            std::cmp::Reverse(
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
        });

        for entry in &entries {
            let ap = entry.path().join("audit.jsonl");
            if !ap.exists() {
                continue;
            }
            if let Ok(contents) = fs::read_to_string(&ap) {
                let has_whoami = contents.lines().any(|line| {
                    serde_json::from_str::<serde_json::Value>(line)
                        .map(|v| v["event"] == "exec_start" && v["binary"] == "whoami")
                        .unwrap_or(false)
                });
                if has_whoami {
                    found = Some(ap);
                    break;
                }
            }
        }
        found.expect("could not find a sysinfo run dir with exec_start for whoami")
    };

    // H10: audit.jsonl exists.
    assert!(
        audit_path.exists(),
        "H10: audit.jsonl should exist at {:?}",
        audit_path
    );

    let contents = fs::read_to_string(&audit_path).expect("read audit.jsonl");
    let lines: Vec<&str> = contents.lines().collect();
    assert!(!lines.is_empty(), "audit.jsonl should not be empty");

    // H11: every line is valid JSON with event, ts, run_id.
    for (i, line) in lines.iter().enumerate() {
        let v: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("H11: line {} not valid JSON: {e}", i));
        assert!(v["event"].is_string(), "H11: line {} missing 'event' field", i);
        assert!(v["ts"].is_string(), "H11: line {} missing 'ts' field", i);
        assert!(v["run_id"].is_string(), "H11: line {} missing 'run_id' field", i);
    }

    // H13: first line is script_start, last line is script_end.
    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let last: serde_json::Value = serde_json::from_str(lines[lines.len() - 1]).unwrap();
    assert_eq!(first["event"], "script_start", "H13: first event should be script_start");
    assert_eq!(last["event"], "script_end", "H13: last event should be script_end");

    // H14: at least one exec_start event exists (sysinfo calls exec()).
    let has_exec_start = lines.iter().any(|line| {
        serde_json::from_str::<serde_json::Value>(line)
            .map(|v| v["event"] == "exec_start")
            .unwrap_or(false)
    });
    assert!(has_exec_start, "H14: at least one exec_start event expected");
}

// ---------------------------------------------------------------------------
// SF-3 — production exec path does not leak host env vars to child processes
// ---------------------------------------------------------------------------

#[test]
fn sf3_exec_does_not_leak_env_to_child() {
    let printenv_exists = std::path::Path::new("/usr/bin/printenv").exists()
        || std::path::Path::new("/bin/printenv").exists();
    if !printenv_exists {
        return; // skip on systems without printenv
    }

    // Script calls printenv (allowed by unix-readonly pact) — child inherits
    // only env_passthrough vars, so REEVE_ENGINE_LEAK_TEST must not appear.
    let script = write_temp_script(r#"
let r = exec("printenv", []);
print(r.stdout);
"#);

    reeve()
        .arg("run")
        .arg(script.path())
        .env("REEVE_ENGINE_LEAK_TEST", "leaked")
        .assert()
        .success()
        .stdout(predicate::str::contains("REEVE_ENGINE_LEAK_TEST").not());
}

// ---------------------------------------------------------------------------
// B8 — REEVE_HOME env var is ignored; compiled-in home is always used
// ---------------------------------------------------------------------------

#[test]
fn b8_reeve_home_env_ignored() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/sysinfo.rhai");

    let fake_home = "/tmp/reeve-test-ignored-b8";
    // Ensure the fake home does not exist before the run.
    let _ = fs::remove_dir_all(fake_home);

    let home = std::env::var("HOME").expect("HOME must be set");
    let runs_dir = Path::new(&home).join(".reeve").join("runs");

    // Count existing run dirs before the run.
    let before_count = fs::read_dir(&runs_dir)
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).count())
        .unwrap_or(0);

    // Run with REEVE_HOME set to a fake path — should be ignored.
    reeve()
        .arg("run")
        .arg(&script)
        .env("REEVE_HOME", fake_home)
        .assert()
        .success();

    // B8a: fake home was NOT created.
    assert!(
        !Path::new(fake_home).exists(),
        "B8: /tmp/reeve-test-ignored-b8 should NOT have been created"
    );

    // B8b: real home DID get a new run entry.
    let after_count = fs::read_dir(&runs_dir)
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).count())
        .unwrap_or(0);
    assert!(
        after_count > before_count,
        "B8: real runs dir should have gained at least one new entry (before={before_count}, after={after_count})"
    );
}
