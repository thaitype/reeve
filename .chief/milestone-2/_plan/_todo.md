# Milestone 2 — TODO

Tasks are dependency-ordered. Later tasks assume earlier ones landed.

- [x] **task-1** — `security.yaml` + `SecurityConfig` + `RunContext` + `init_home`.
      New `src/security.rs` with `SecurityConfig::load()`. New `src/core/run_context.rs`
      with `RunContext`. New `src/core/home.rs` with `init_home()`. Wire into `main()`:
      load config → init home → build RunContext. Exit code 3 on config/home error.
      Unit tests: parse valid yaml, reject invalid yaml, init_home idempotent.

- [x] **task-2** — `AuditWriter` + JSONL audit log infrastructure.
      New `src/core/audit.rs`. Implement `AuditWriter::open()` + `emit()` with
      flush-per-event. Define all 6 event types (`script_start`, `exec_start`,
      `exec_end`, `exec_error`, `script_log`, `script_end`) with RFC 3339 `ts` and
      `run_id`. Wire `script_start`/`script_end` into `main()`. Wire `exec_start`,
      `exec_end`, `exec_error` into executor. Audit write failures → stderr warn,
      don't abort. Unit tests: emit writes valid JSONL, flush called per event,
      script_start/end present after a run.

- [x] **task-3** — Layer 1 FS host functions.
      New `src/core/fs.rs` with `read_file`, `read_lines`, `exists`, `write_file`,
      `append_file`. Path validation: reject absolute paths and `..` traversal,
      scope all ops to `<reeve_home>/workspace/`. Register in `engine.rs` via
      `RunContext`. Error kinds: `PathDenied`, `FileNotFound`, `FileAlreadyExists`,
      `IoError` per `_contract/02-host-fns.md`. Unit tests: all bypass-resistance
      cases B1–B5 + happy-path H1–H5 from `_contract/04-test-matrix.md`.

- [x] **task-4** — `env()`, `to_json()`, `log_*` audit wiring, engine signature update.
      Add `env()` host fn (EnvDenied/EnvUnset per contract). Add `to_json()` host fn
      (serde_json serialise Dynamic). Update `log_info/warn/error` to emit `script_log`
      audit events via RunContext. Update `build_engine_with_args` signature to accept
      `Arc<RunContext>`. Update all callers. Unit tests: B6, B7, H6–H9 from test matrix.

- [x] **task-5** — Examples, integration tests, measurement.
      Create `examples/workspace-demo.rhai`. Integration tests: B8 (REEVE_HOME ignored),
      H10–H14 (audit log presence/content), N1 (workspace-demo runs clean), R1 regression.
      Verify binary size < 10 MB and cold start < 50 ms. Update README if needed.
