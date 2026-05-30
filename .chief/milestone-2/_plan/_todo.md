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

- [x] **task-6** — Switch run-id from UUID v4 to UUID v7.
      In `Cargo.toml`, add `"v7"` to uuid features. In `src/bin/reeve.rs`, replace
      `Uuid::new_v4()` with `Uuid::now_v7()`. Verify `runs/` directories sort
      chronologically by name. No contract changes needed.

## Security-review follow-up batch (autopilot, see _report/security-review.md)

- [x] **task-7 (F2)** — Correct docs: release-notes claim `kind: filepath` / `regex`
      validators that don't exist (`KindSpec` is only enum/number/string).
      In `.chief/milestone-2/_report/release-notes.md`: line ~20 kind list → drop
      `filepath`/`regex`; Q&A line ~156-157 → use `kind: string` (exists) and note
      path-scoping is a v0.3.0 addition; line ~125 → clarify `kind: filepath` is
      forthcoming. Docs only, no code.

- [x] **task-8 (F1)** — Enforce `audit.capture_command` (currently a no-op, fail-open).
      Decision: ENFORCE (not remove). When `capture_command == false`, `exec_start`
      emits `argv: []` (field still present; `binary` kept). Default is `true` →
      behaviour unchanged. Plumb the bool from `SecurityConfig.audit.capture_command`
      through `run_exec_audited` into the `exec_start` event. Update contract
      `_contract/03-audit-log.md` to document the flag's effect. Add a unit test:
      capture_command=false ⇒ exec_start argv empty.

- [x] **task-9 (F5+F6)** — Remove dead timeout scaffolding + fix stale comments.
      Delete the `ExecError` event variant, `exec_error()` constructor, and
      `limit_ms` field from `src/core/audit.rs` (never emitted since timeout removal;
      its only `kind`s were Timeout/OutputLimitExceeded, both gone). Remove the
      `exec_error` section from `_contract/03-audit-log.md` and fix the stale
      "UUID v4" line (now v7). Fix stale comments: `engine.rs:87` ("+ cap"),
      `executor.rs:146` ("with byte cap"), `executor.rs:156-157` ("enforces the cap
      and timeout").

- [x] **task-10 (F4, optional)** — Byte-length guard before native parse.
      In `src/core/parse.rs`, reject input over a generous constant (10 MiB) in
      `parse_json` and `parse_yaml` with a `ParseError` before calling serde.
      NOTE: this bounds large-flat-input DoS only; it does NOT fully prevent YAML
      alias-bomb expansion (small input, huge tree) — that needs alias-limiting,
      deferred. Add one unit test per fn for the over-limit case.
