use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn warden() -> Command {
    Command::cargo_bin("warden").expect("warden binary should be built")
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
    warden()
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
// "warden run --pact x.yaml <script>" → clap unknown-argument error, exit ≠ 0.
// ---------------------------------------------------------------------------

#[test]
fn unknown_flag_pact_rejected_by_clap() {
    let script = write_temp_script(r#"print("hello");"#);
    // --pact is placed before the script path so clap's normal parser sees it
    // and rejects it (trailing_var_arg only captures args after the positional).
    warden()
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
    warden()
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
    warden()
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
    // "rm" is not in linux-readonly pact → BinaryNotAllowed → exit 2
    let script = write_temp_script(r#"exec("rm", []);"#);
    warden()
        .arg("run")
        .arg(script.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("BinaryNotAllowed"));
}

// ---------------------------------------------------------------------------
// script_args passthrough
// ---------------------------------------------------------------------------

#[test]
fn script_args_passthrough() {
    let script = write_temp_script(r#"print(script_args()[0]);"#);
    warden()
        .arg("run")
        .arg(script.path())
        .arg("foo")
        .arg("bar")
        .assert()
        .success()
        .stdout(predicate::str::contains("foo"));
}
