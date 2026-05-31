# Milestone 2 — Architectural Decisions

Decisions locked during the Phase 0 grill. Do not revisit without a new
grill session.

## D1 — `RunContext` as the shared state carrier

All host functions receive shared state via a single
`Arc<RunContext>` captured in their closures at engine-build time.

```rust
pub struct RunContext {
    pub security: Arc<SecurityConfig>,
    pub audit:    Arc<Mutex<AuditWriter>>,
}
```

`build_engine_with_args` gains a `ctx: Arc<RunContext>` parameter.
Each closure clones `Arc::clone(&ctx)`. No global state; no
`OnceLock`; tests are isolated by constructing independent `RunContext`
instances.

## D2 — Audit flush strategy

`AuditWriter::emit()` flushes after every event (`writer.flush()`
after each `writeln!`). Audit integrity over performance: partial logs
remain readable on crash or timeout. Events are small JSON lines —
per-event flush cost is negligible.

## D3 — No `script_sha256` in `script_start`

The `ts`, `run_id`, `script_path`, `args` fields are sufficient for
this increment. `script_sha256` is a forensics nicety — add with
`sha2` dep when `--isolated` or a forensics use case drives it.

## D4 — `script_path` canonicalized before logging

`fs::canonicalize(script_path)` before emitting `script_start`. Falls
back to raw path string on error (should not occur — script must
exist to be loaded). Audit log is unambiguous regardless of invocation
directory.

## D5 — Home init is idempotent, no sentinel

`create_dir_all(workspace/)` + `create_dir_all(runs/)` only. Both
calls are idempotent — safe on every run. No `.reeve-managed` sentinel
this milestone: it has no enforcement value until `reeve-flex` ships
and `REEVE_HOME` becomes caller-settable.

## D6 — `log_*` host fns emit audit events

`log_info`, `log_warn`, `log_error` write to stderr AND emit
`script_log` audit events via `RunContext.audit`. One additional
`emit()` call per log fn — no structural change.

## D7 — `env()` throws, does not probe

`env("KEY")` throws `EnvDenied` if `KEY` is not in `env_passthrough`.
Throws `EnvUnset` if `KEY` is in `env_passthrough` but absent from
the process environment. Scripts that need optional env vars must
guard explicitly. No probe/default pattern.

## D8 — Audit event timestamp format

Every event carries `"ts": "<RFC 3339>"` produced by
`chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)`.
Example: `"2026-05-12T10:23:45.123Z"`.
