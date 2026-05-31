# Contract — Test Matrix (Milestone 2)

Extends `milestone-1/_contract/03-test-matrix.md`. All milestone-1 tests
must continue to pass (regression gate). New tests are listed below.

Tests are unit or integration. No external infra required — all run on
any Linux/macOS dev box with `cargo test`.

---

## Bypass-resistance tests (must all pass)

### FS Layer 1

| # | Script / action | Expected result |
|---|---|---|
| B1 | `read_file("/etc/passwd")` | Throws `PathDenied` |
| B2 | `read_file("../../etc/passwd")` | Throws `PathDenied` (traversal) |
| B3 | `write_file("out.json", "x"); write_file("out.json", "y")` | Second call throws `FileAlreadyExists` |
| B4 | `append_file("/etc/hosts", "x")` | Throws `PathDenied` |
| B5 | `read_file("../runs/anything/audit.jsonl")` | Throws `PathDenied` (outside workspace) |

### `env()`

| # | Action | Expected result |
|---|---|---|
| B6 | `env("AWS_SECRET_ACCESS_KEY")` | Throws `EnvDenied` |
| B7 | `env("UNSET_ALLOWED_VAR")` where key is in `env_passthrough` but absent from test process env | Throws `EnvUnset` |

### `REEVE_HOME` env var

| # | Action | Expected result |
|---|---|---|
| B8 | `REEVE_HOME=/tmp/x reeve run script.rhai` | Home path comes from compiled `security.yaml`, not from env var. `/tmp/x` is ignored. |

---

## Happy-path tests

### FS Layer 1

| # | Script | Expected result |
|---|---|---|
| H1 | `write_file("out.txt", "hello"); read_file("out.txt")` | Returns `"hello"` |
| H2 | `append_file("log.txt", "a"); append_file("log.txt", "b"); read_file("log.txt")` | Returns `"ab"` |
| H3 | `write_file("x.txt", "y"); exists("x.txt")` | Returns `true` |
| H4 | `exists("missing.txt")` | Returns `false` |
| H5 | `read_lines("out.txt")` on a file with two lines | Returns array of two strings without trailing newlines |

### `env()`

| # | Script | Expected result |
|---|---|---|
| H6 | `env("HOME")` | Returns non-empty string |
| H7 | `env("PATH")` | Returns non-empty string |

### `to_json()`

| # | Script | Expected result |
|---|---|---|
| H8 | `to_json(#{"x": 1, "y": "hello"})` | Returns valid JSON string parseable back to original map |
| H9 | `to_json([1, 2, 3])` | Returns `"[1,2,3]"` (or equivalent) |

### Audit log

| # | Action | Expected result |
|---|---|---|
| H10 | Run any script to completion | `audit.jsonl` exists at `<reeve_home>/runs/<run-id>/audit.jsonl` |
| H11 | Parse `audit.jsonl` lines | Every line is valid JSON with `event`, `ts`, `run_id` fields |
| H12 | Run a script that calls `log_info("hi")` | `audit.jsonl` contains a `script_log` event with `level:"info"` and `msg:"hi"` |
| H13 | Run any script | `audit.jsonl` starts with `script_start` and ends with `script_end` |
| H14 | Run a script calling `exec("whoami", [])` | `audit.jsonl` contains `exec_start` and `exec_end` events for `whoami` |

### `security.yaml`

| # | Action | Expected result |
|---|---|---|
| H15 | Invalid `security.yaml` embedded at compile time | `cargo build` fails |

---

## Example / integration tests

### Regression

| # | Script | Expected result |
|---|---|---|
| R1 | `examples/sysinfo.rhai` | Runs clean; stdout contains whoami/hostname/uname/date output |

### New example

| # | Script | Expected result |
|---|---|---|
| N1 | `examples/workspace-demo.rhai` | Writes a file, appends to it, reads it back, prints contents. Exits 0. Audit file written. |

`workspace-demo.rhai` must be created as part of this milestone.

---

## Notes

- FS tests that write to `workspace/` should use `tempfile::TempDir` for
  `reeve_home` to avoid cross-test contamination. `RunContext` accepts a
  `SecurityConfig` with a custom `reeve_home` path.
- B8 is an integration test (`assert_cmd`) that sets `REEVE_HOME` in the
  child process environment and asserts the actual home used is from
  `security.yaml`.
- H15 requires a compile-time test (build script or `include_str!` +
  `serde_yaml::from_str` in a `const` context, or a doc-test that
  panics on parse failure at test time).
