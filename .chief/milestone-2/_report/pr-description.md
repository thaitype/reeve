# PR Description — Milestone 2 → main

**Title:** feat(milestone-2): persistent state, Layer 1 FS, JSONL audit, env/json host fns

---

## Summary

This PR delivers milestone-2: persistent user state and a compile-time security boundary for `reeve`. All 100 tests pass (89 unit + 11 CLI). Binary 5 MB, cold start 6 ms.

### What's new

- **`security.yaml`** — compile-time embedded into the binary. Declares `reeve_home`, `env_passthrough`, and audit capture flags. Loaded once at startup into `SecurityConfig`. Operator customisation requires a rebuild — no runtime override by AI.

- **`$HOME/.reeve/` lazy init** — `workspace/` and `runs/` directories are created on first run. Idempotent. No sentinel file this milestone.

- **Layer 1 FS host functions** — `read_file`, `read_lines`, `exists`, `write_file`, `append_file`. All scoped to `<reeve_home>/workspace/`. Append-only write semantics: `write_file` throws `FileAlreadyExists` on collision. Symlink escape is detected and rejected.

- **JSONL audit log** — every run writes to `$HOME/.reeve/runs/<run-id>/audit.jsonl`. Five event types: `script_start`, `exec_start`, `exec_end`, `script_log`, `script_end`. Flushed after each event for crash-safety.

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

## Security review follow-ups

A full read of the security-relevant surface (`_report/security-review.md`, AI-agent-as-caller threat model) found no bypass in the load-bearing controls. Four low-effort findings were fixed before this PR:

| # | Finding | Fix |
|---|---|---|
| F1 | `audit.capture_command` was parsed but never enforced (fail-open — full `argv` always logged) | Flag is now honoured: when `false`, `exec_start` emits `argv: []` (`binary` retained). Default `true` → unchanged. |
| F2 | Release notes advertised `kind: filepath` / `regex` validators that don't exist (`KindSpec` is only `enum`/`number`/`string`) | Corrected the kind list and Q&A; `filepath` + `allowed_roots` labelled as forthcoming in v0.3.0. Docs only. |
| F5/F6 | Dead `ExecError` event, `exec_error()` constructor, and `limit_ms` field (orphaned since the timeout/output-cap removal); stale "enforces timeout + cap" comments | Deleted the dead code and contract section; corrected the stale comments. |
| F4 | `parse_json` / `parse_yaml` fed agent input straight to serde, outside Rhai's op budget (large-flat-input DoS) | Added a 10 MiB `MAX_PARSE_BYTES` guard before serde runs. Does not fully prevent YAML alias-bomb expansion (deferred). |

The per-exec timeout and output cap were also removed earlier on this branch (`exec()` now waits for the child indefinitely and reads output unbounded, matching bash's default). This is documented under "Resource exhaustion" in the release-notes security model; re-introducing a streaming cap + optional `wait_timeout` is deferred to a later milestone.

---

## What this PR does NOT include

These are explicitly deferred to future milestones per the milestone-2 scope:

- `reeve-flex` binary
- `pipe()` / `pipe_allow_fail()` host fns
- `exec()` opts (`stdin:`, `stdout_to:`); a `wait_timeout` / output cap (the just-removed code path) is a candidate re-add
- Layer 2 / `allowed_roots` filepath validation in `exec()`
- `glob()` FS fn
- `check`, `preset list`, `preset show` CLI subcommands
- Three canonical presets (`core-tools`, `k8s-readonly`, `git-readonly`)

---

## Test plan

- [x] 89 unit tests pass (`cargo test --lib`)
- [x] 11 CLI integration tests pass (`cargo test --test cli`)
- [x] Binary size: 5.0 MB < 10 MB target
- [x] Cold start: 6 ms < 50 ms target
- [x] Symlink escape rejected in `read_file`, `write_file`, `append_file`, `exists`
- [x] Child env isolation: `printenv` does not show host secrets (SF-3 CLI test)
- [x] REEVE_HOME env var ignored at runtime (B8)
- [x] Audit log contains `exec_start` with correct binary after sysinfo run (H10–H14)
- [x] `exec("echo", [42])` returns `TypeError` error, does not panic (security fix)
- [x] `capture_command: false` ⇒ `exec_start` emits empty `argv` (F1)
- [x] `parse_json` / `parse_yaml` reject input over 10 MiB before serde runs (F4)
- [x] `workspace-demo.rhai` runs clean end-to-end (N1)
- [ ] `cargo clippy --all-targets -- -D warnings` — run in a connected environment before merge (clippy unavailable in this offline env; `cargo check --all-targets` is clean)
