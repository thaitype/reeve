# Milestone 2 — Code Review Report

Reviewed by: deep code audit (2026-05-12)
Scope: all files introduced or modified in milestone-2.

---

## Security Findings

### SF-1 — Symlink Escape in Layer 1 FS (CRITICAL)

**File:** `src/core/fs.rs`, `validate_path()`

`validate_path` canonicalizes the **parent directory** of the candidate path, not the candidate itself. A symlink placed inside `workspace/` pointing to a path outside (e.g. `/etc/passwd`) passes validation and is followed by the subsequent `read_to_string` / `OpenOptions::open`. A script — or a binary spawned via `exec()` — that writes a symlink into workspace enables a later `read_file("link")` or `append_file("link", ...)` to escape the sandbox.

The design grill session (`_grill/closed/0001-reeve-design.md`) explicitly requires:
> `read_file("/workspace/symlink-to-secret") → reject (symlink resolved)` as a CI test.

This test does not exist, and the code does not resolve full-path symlinks.

**Fix:** For `read_file` / `read_lines` (file must exist): replace the parent-only canonicalize with `fs::canonicalize(&candidate)` and re-check that the result starts with `workspace_root`. For `write_file` / `append_file`: check `read_link(&candidate)` before open; if it is a symlink, resolve it and validate. Add the symlink rejection test.

---

### SF-2 — Spawned Processes Inherit Full Host Environment (HIGH)

**File:** `src/core/executor.rs`

`executor.rs` never receives `RunContext` or `SecurityConfig`. Spawned child processes inherit the full parent environment — including `AWS_SECRET_ACCESS_KEY`, `GITHUB_TOKEN`, and any other secrets. The `env_passthrough` list in `SecurityConfig` is wired to the `env()` Rhai host fn (script reads) but NOT to `Command` spawning. A pact that allows `env` (e.g. if `env` were ever added) or any binary that reads environment variables would receive the full secret-bearing env.

The milestone-2 contract (`02-host-fns.md`) states `env_passthrough` is "now sourced from SecurityConfig" but the child-env filtering was not implemented.

**Fix:** Pass `Arc<SecurityConfig>` (or just `env_passthrough: &[String]`) into `run_exec_with`. Call `cmd.env_clear()` then re-add only the passthrough keys before spawn.

---

### SF-3 — Naive `$HOME` String Substitution (LOW)

**File:** `src/security.rs`

`raw.reeve_home.replace("$HOME", &home)` is a substring match. A value like `"$HOME_DIR/.reeve"` would be silently mis-expanded. Not a critical vector (config is compile-time embedded), but fragile. Use `replace("$HOME/", &format!("{home}/"))` or match only when `$HOME` is at the start.

---

## Contract Violations

### CV-1 — `env_passthrough` Not Applied to Child Processes

Mirrors SF-2. The contract says the change was to source `env_passthrough` from `SecurityConfig` — the child-env filtering is the operational meaning of that change. Currently dead.

### CV-2 — Duplicate `kind` Key in Error Maps

**File:** `src/core/executor.rs`

`runtime_err_map(kind, fill)` inserts `"kind"` first. The `fill` closures for `Timeout`, `OutputLimitExceeded`, and `ExecFailed` each insert `"kind"` a second time. Silent (second write wins, same value) but a maintenance trap. The fill closures should not insert `"kind"`.

### CV-3 — H12 (`script_log` Audit Event) Not Integration-Tested

**File:** `tests/cli.rs`

No test runs a script that calls `log_info` and verifies the `script_log` event in `audit.jsonl`. Contract test H12 is unimplemented.

### CV-4 — H15 (Invalid YAML Fails Build) Not Implemented

`include_str!` + `serde_yaml::from_str` runs at startup, not at compile time. An invalid `security.yaml` produces a runtime exit-3, not a build failure. Contract H15 requires a build-time check (build script or `const`-context parse test).

### CV-5 — H7 (`env("PATH")`) Not Tested

Contract test H7 is absent. Minor — the code path is identical to H6 (`env("HOME")`), but the test should exist per the matrix.

---

## Technical Debt

| ID | Location | Issue |
|---|---|---|
| TD-1 | `src/core/engine.rs` | `Box::leak(Box::new(tmp))` in tests — leaks TempDir; dirs not cleaned up between tests |
| TD-2 | `src/core/executor.rs` | `into_inner().unwrap()` on Mutex — panics on poison; use `unwrap_or_else(|e| e.into_inner())` |
| TD-3 | `src/core/executor.rs` | `trace!` macro always writes `[exec] binary=...` to stderr — noise in production; needs a `REEVE_DEBUG` / `--verbose` gate |
| TD-4 | `src/core/fs.rs` | `read_lines` uses `split('\n')` + manual cleanup; `"line1\n\n"` yields a phantom empty line. Use `content.lines().map(str::to_owned).collect()` instead |
| TD-5 | `src/core/executor.rs` | `elapsed_ms` cast chain: `as_millis()` → `i64` → `u64`; negative silent wrap on Mutex re-use. Use `u64` throughout |
| TD-6 | `src/bin/reeve.rs` | `audit.lock().expect(...)` — panics if Mutex is poisoned (e.g. after a host fn panic). Use `unwrap_or_else(|e| e.into_inner())` |
| TD-7 | `src/lib.rs` | `pub mod core` exposes internal implementation surface; should be `pub(crate)` |
| TD-8 | `src/core/audit.rs` | `pub run_id` on `AuditWriter` causes callers to lock mutex just to read the ID; store `run_id` separately in `RunContext` |

---

## Test Coverage Gaps

| ID | Missing test | Severity |
|---|---|---|
| TCG-1 | Symlink inside workspace pointing outside → `PathDenied` on `read_file` | Critical (SF-1) |
| TCG-2 | `script_log` audit event produced by `log_info` call | Medium (CV-3 / H12) |
| TCG-3 | Invalid `security.yaml` → build failure | Medium (CV-4 / H15) |
| TCG-4 | `env("PATH")` returns non-empty string | Low (CV-5 / H7) |
| TCG-5 | `exists()` on symlink pointing outside workspace | Low |
| TCG-6 | `write_file` / `append_file` into subdirectory creates intermediate dirs | Low |
| TCG-7 | `append_file` on non-existent file in new subdirectory | Low |
| TCG-8 | `exec_allow_fail` with non-zero exit emits audit events | Low |
| TCG-9 | `audit.try_emit` lock-poisoning silently drops event (no WARN emitted) | Low |

---

## Minor / Style

| ID | Location | Note |
|---|---|---|
| MS-1 | `src/core/engine.rs` | `VarError::NotUnicode` throws `IoError` with `path: ""` — semantically wrong kind; should be a distinct `EnvError` or `EnvDenied` with `msg` |
| MS-2 | `tests/cli.rs` `n1_workspace_demo_runs_clean` | Writes to real `$HOME/.reeve/workspace/`; parallel tests may race with production state. Integration tests should use a controlled home |
| MS-3 | `src/core/executor.rs` | Dead `spawn_start` variable with a confusing comment — remove |
| MS-4 | `src/core/engine.rs` | `workspace_root: Arc<Path>` construction from temporary is correct but non-obvious; add a comment |

---

## Priority Fix List

| # | Severity | Action |
|---|---|---|
| 1 | CRITICAL | Fix symlink escape in `validate_path` — canonicalize full path for reads; check `read_link` before writes |
| 2 | CRITICAL | Add symlink-escape test: `read_file("symlink-outside")` → `PathDenied` |
| 3 | HIGH | Pass `SecurityConfig` into executor; call `cmd.env_clear()` + re-add passthrough keys before spawn |
| 4 | MEDIUM | Remove duplicate `kind` inserts from `fill` closures in `runtime_err_map` |
| 5 | MEDIUM | Add H12 test: `log_info` → `script_log` in audit |
| 6 | LOW | Gate `trace!` macro on `REEVE_DEBUG` env var or remove it |
| 7 | LOW | Replace `split('\n')` in `read_lines` with `lines()` |
| 8 | LOW | Fix Mutex unwrap chains to use `unwrap_or_else(|e| e.into_inner())` |
| 9 | LOW | Change `pub mod core` → `pub(crate) mod core` in `lib.rs` |
| 10 | LOW | Add missing tests: H7, TCG-5, TCG-6, TCG-7 |

---

## Overall Assessment

**Not ready to ship in current state.** The symlink escape (SF-1) is a genuine sandbox bypass and must be fixed before this milestone is considered complete. The child-env leak (SF-2) is a high-severity gap given the project's stated security goals. Items 1–5 in the fix list are blocking; 6–10 are recommended before the next milestone.
