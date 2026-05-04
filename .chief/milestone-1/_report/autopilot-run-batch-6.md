# Autopilot Run Batch 6

## Mode
auto (scoped to task-6 only)

## Summary
Real `exec` + `exec_allow_fail` host fns wired through `warden-pact`'s
allowlist engine, with `wait-timeout` driving the per-exec timer and
two-thread reader for stdout/stderr. `executor::trace!` macro emits one
single-line `[exec] key=value` per call. 12 tests pass (1.0s wall),
clippy clean.

## Tasks Completed
- task-6 — `executor.rs` (run_exec, run_exec_allow_fail, run_exec_with),
  `PactError` → Rhai error map conversion (`BinaryNotResolvable` →
  `BinaryNotFound`), real exec wired into engine, trace macro.

## Decisions Made (auto mode)
- **Issue:** Timeout primitive — hand-roll vs `wait-timeout`.
  - **Chosen:** `wait-timeout` crate.
  - **Reason:** Tiny dep, well-tested, gives a clean
    `wait_timeout(Duration) -> Result<Option<ExitStatus>>` without
    spawning helper threads for the wait itself.

- **Issue:** Output-cap check timing — stream-time enforcement vs
  post-exit check.
  - **Chosen:** Post-exit check after the child is reaped.
  - **Reason:** Simpler, ~50 lines instead of a polling loop. **Known
    limitation:** a runaway producer could buffer well above the cap
    before the timeout kills it (e.g. `yes` for 1s ≈ tens of MB before
    cap is observed). Acceptable for milestone 1 because (a) the
    timeout always bounds total runtime, (b) memory pressure is bounded
    by `timeout_seconds × producer rate`, (c) JSONL audit / streaming
    enforcement is a deferred feature anyway. Re-evaluate when adding
    `stdout_to` (Layer 1 streaming) — that work will naturally move the
    cap check stream-side.

- **Issue:** Cap-check ordering when a child both times out AND
  exceeds the cap.
  - **Chosen:** Cap-exceeded takes priority over timeout in the
    post-exit check.
  - **Reason:** Matches the `yes_exceeds_output_cap` test's
    expectation. If a script needs to distinguish, it has the trace
    line. Re-document if scripts need finer error discrimination.

- **Issue:** `warden-pact::test_fixtures()` is `#[cfg(test)]` and not
  visible to `warden-core`'s test build.
  - **Chosen:** `warden-core` tests embed the fixture YAML via
    `include_str!` and parse it directly using `parse_pact`.
  - **Reason:** Avoids exposing a test-only API across the crate
    boundary. The YAML lives in
    `crates/warden-pact/tests/fixtures/test-fixtures.yaml`; the
    `include_str!` path from `warden-core` walks up.

## Backlog
task-7 (parse_json/parse_yaml/script_args/print/log_*),
task-8 (CLI), task-9 (examples + integration tests), task-10
(measurement + README).

## User Action Needed
None. `/dump-commit` then `/chief-autopilot only focus on task-7`.
