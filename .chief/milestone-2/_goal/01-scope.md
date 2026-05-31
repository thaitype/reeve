# Milestone 2 — Scope

## One-line goal

Add persistent state to `reeve`: a compile-time security boundary
(`security.yaml`), a per-user home directory (`$HOME/.reeve/`),
workspace-scoped file I/O (Layer 1 FS), a JSONL audit trail, and
the `env()` + `to_json()` host fns.

## What this milestone delivers

1. **`security.yaml`** — compile-time embedded into the `reeve` binary.
   Declares `reeve_home`, `env_passthrough`, `allowed_roots` (used later),
   and audit capture flags. Loaded once at startup into `SecurityConfig`.

2. **`$HOME/.reeve/` lazy init** — `create_dir_all(workspace/)` +
   `create_dir_all(runs/)` on first run. Idempotent on subsequent runs.
   No sentinel file (`.reeve-managed`) this milestone — deferred to
   `reeve-flex` increment where it has a real threat model.

3. **Layer 1 FS host fns** — scoped to `<reeve_home>/workspace/` only:
   `read_file`, `read_lines`, `exists`, `write_file`, `append_file`.
   Append-only write semantics: `write_file` throws `FileAlreadyExists`
   on collision. No `delete`, `rename`, `glob` this milestone.

4. **JSONL audit log** — every run writes to
   `$HOME/.reeve/runs/<run-id>/audit.jsonl` (UUID v4 run-id).
   Events: `script_start`, `exec_start`, `exec_end`, `exec_error`,
   `script_log`, `script_end`. Every event carries a `ts` field
   (RFC 3339, via `chrono::Utc::now()`). Flushed after each event.
   `capture_stdout`/`capture_stderr` off by default.

5. **`env()` host fn** — reads env vars gated by `env_passthrough`
   from `security.yaml`. Throws `EnvDenied` for unlisted keys,
   `EnvUnset` for listed-but-absent keys.

6. **`to_json()` host fn** — serialises any Rhai `Dynamic` to a JSON
   string. Complement to the existing `parse_json()`.

7. **`exec()` env behaviour update** — executor reads `env_passthrough`
   from `SecurityConfig` instead of the hardcoded constant from
   milestone-1.

## What this milestone does NOT deliver

- `reeve-flex` binary (its own increment).
- `pipe()` / `pipe_allow_fail()` (deferred).
- `exec()` opts (`stdin:`, `stdout_to:`, `timeout_seconds:`).
- Layer 2 / `allowed_roots` filepath validation in `exec()`.
- `glob()` FS fn + `glob` crate dep.
- `.reeve-managed` sentinel.
- `config.json` / `max_workspace_bytes` / `auto_cleanup`.
- `check`, `preset list`, `preset show` CLI subcommands.
- Three canonical presets (`core-tools`, `k8s-readonly`, `git-readonly`).
- `parse_toml()`.
- CLI flags (`--timeout`, `--quiet`, `--json`).

## Binary / performance targets (unchanged from milestone-1)

- Binary size: < 10 MB.
- Cold start: < 50 ms.

## Source reference

`draft/increment-2.md` — the authoritative increment spec this
milestone is built from.
