# Autopilot Run Batch 1

## Mode
auto

## Summary

Implemented the full milestone-2 scope: `security.yaml` compile-time embedding,
`SecurityConfig` + `RunContext`, lazy home init, JSONL audit log, Layer 1 FS host
fns, `env()`, `to_json()`, and integration tests. All 86 tests pass; binary 5 MB;
cold start 6 ms.

## Tasks Completed

- **task-1:** `security.yaml` + `SecurityConfig::load()` + `RunContext` + `init_home()`. Wired into `main()` with exit-code-3 error handling.
- **task-2:** Full `AuditWriter` (flush-per-event). All 6 event types. `script_start`/`script_end` in `main()`; `exec_start`/`exec_end`/`exec_error` in executor. `log_*` emit `script_log` audit events. `build_engine_with_args` updated to accept `Arc<RunContext>`.
- **task-3:** Layer 1 FS host fns (`read_file`, `read_lines`, `exists`, `write_file`, `append_file`). Path validation (absolute + traversal rejection). All B1–B5 bypass-resistance and H1–H5 happy-path tests pass.
- **task-4:** `env()` (EnvDenied/EnvUnset) and `to_json()` host fns. Unit tests B6, B7, H6, H8, H9 all pass.
- **task-5:** `examples/workspace-demo.rhai`. Integration tests B8 (REEVE_HOME ignored), H10–H14 (audit log content), N1 (workspace-demo), R1 (sysinfo regression). Measurement report written.

## Decisions Made

- **Issue:** task-2 scope overlap with task-4 (`build_engine_with_args` signature + `log_*` audit wiring).
  - **Chosen:** builder-agent for task-2 completed both items early.
  - **Reason:** Natural to wire the engine signature when implementing `RunContext` — avoided a second pass.

- **Issue:** Integration test for audit log content (H10–H14) must find the correct `audit.jsonl` under `$HOME/.reeve/runs/` without knowing the UUID.
  - **Chosen:** Scan `runs/` dir for the entry whose log contains `exec_start` with `binary=whoami` (sysinfo-specific).
  - **Reason:** Parallel-test-safe; doesn't rely on timestamp ordering.

- **Issue:** `workspace-demo.rhai` would throw `FileAlreadyExists` on second run.
  - **Chosen:** Script uses `exists()` to guard before `write_file`.
  - **Reason:** Makes the example rerunnable without manual cleanup; consistent with spec's "loud failure" intent only applying when the collision is unexpected.

## Backlog

None — all milestone-2 goals met.

## User Action Needed

None. Milestone-2 is complete. Suggested next step: commit and push the branch, then plan milestone-3 (`reeve-flex` binary + `pipe()` + Layer 2).
