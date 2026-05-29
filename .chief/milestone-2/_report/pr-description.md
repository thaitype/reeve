# PR Description — Milestone 2 → main

**Title:** feat(milestone-2): persistent state, Layer 1 FS, JSONL audit, env/json host fns

---

## Summary

This PR delivers milestone-2: persistent user state and a compile-time security boundary for `reeve`. All 99 tests pass (88 unit + 11 CLI). Binary 5 MB, cold start 6 ms.

### What's new

- **`security.yaml`** — compile-time embedded into the binary. Declares `reeve_home`, `env_passthrough`, and audit capture flags. Loaded once at startup into `SecurityConfig`. Operator customisation requires a rebuild — no runtime override by AI.

- **`$HOME/.reeve/` lazy init** — `workspace/` and `runs/` directories are created on first run. Idempotent. No sentinel file this milestone.

- **Layer 1 FS host functions** — `read_file`, `read_lines`, `exists`, `write_file`, `append_file`. All scoped to `<reeve_home>/workspace/`. Append-only write semantics: `write_file` throws `FileAlreadyExists` on collision. Symlink escape is detected and rejected.

- **JSONL audit log** — every run writes to `$HOME/.reeve/runs/<run-id>/audit.jsonl`. Six event types: `script_start`, `exec_start`, `exec_end`, `exec_error`, `script_log`, `script_end`. Flushed after each event for crash-safety.

- **`env()` host fn** — reads env vars gated by `env_passthrough`. Throws `EnvDenied` for unlisted keys, `EnvUnset` for listed-but-absent. Strict — no probe/default pattern.

- **`to_json()` host fn** — serialises any Rhai `Dynamic` to a JSON string. Complement to `parse_json()`.

- **`exec()` env filtering** — spawned child processes run with `env_clear()` + only the `env_passthrough` keys re-added. Secrets in the host environment do not leak to child processes.

- **UUID v7 run IDs** — `runs/` directories now sort chronologically by name (`ls -1 ~/.reeve/runs/` gives time order).

---

## Security fixes included

Four issues found during pre-merge review and fixed before this PR:

| # | File | Issue | Fix |
|---|---|---|---|
| 1 | `executor.rs` | Reader thread panics silently swallowed; partial output used with no error signal | Propagate `thread::join()` errors as `ExecFailed` |
| 2 | `fs.rs` | `path.contains("..")` substring check falsely rejects filenames like `v2..1/out.txt` | Replaced with component-level check: `path.split('/').any(\|c\| c == "..")` |
| 3 | `executor.rs` | `elapsed_ms` captured before reader threads drain; audit underreports wall time | Moved measurement to after `join()` calls |
| 4 | `engine.rs` | `d.cast::<String>()` panics on non-string exec arg, killing process and skipping audit | Replaced with `try_cast` returning a catchable `TypeError` error |

Additionally, `run_id` is now a plain field on `RunContext` (instead of locked inside `AuditWriter`), eliminating 4 redundant Mutex acquisitions per exec call.

---

## What this PR does NOT include

These are explicitly deferred to future milestones per the milestone-2 scope:

- `reeve-flex` binary
- `pipe()` / `pipe_allow_fail()` host fns
- `exec()` opts (`stdin:`, `stdout_to:`, `timeout_seconds:`)
- Layer 2 / `allowed_roots` filepath validation in `exec()`
- `glob()` FS fn
- `check`, `preset list`, `preset show` CLI subcommands
- Three canonical presets (`core-tools`, `k8s-readonly`, `git-readonly`)

---

## Test plan

- [x] 88 unit tests pass (`cargo test --lib`)
- [x] 11 CLI integration tests pass (`cargo test --test cli`)
- [x] Binary size: 5.0 MB < 10 MB target
- [x] Cold start: 6 ms < 50 ms target
- [x] Symlink escape rejected in `read_file`, `write_file`, `append_file`, `exists`
- [x] Child env isolation: `printenv` does not show host secrets (SF-3 CLI test)
- [x] REEVE_HOME env var ignored at runtime (B8)
- [x] Audit log contains `exec_start` with correct binary after sysinfo run (H10–H14)
- [x] `exec("echo", [42])` returns `TypeError` error, does not panic (security fix)
- [x] `workspace-demo.rhai` runs clean end-to-end (N1)
