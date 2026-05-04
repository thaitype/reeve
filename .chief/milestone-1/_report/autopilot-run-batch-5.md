# Autopilot Run Batch 5

## Mode
auto (scoped to task-5 only)

## Summary
`reeve-core::engine::build_engine()` ships with all six Rhai resource
limits and `eval` disabled. Nine host fns registered as stubs (real
behavior in tasks 6 and 7). 6 tests pass; clippy clean.

## Tasks Completed
- task-5 — Rhai engine constructor + stub host fns + sandbox tests
  (rows #7 module import, #8 eval, #9 max operations).

## Decisions Made (auto mode)
- **Issue:** Contract `02-host-fns.md` and `draft/spec-v2.md` both wrote
  `set_max_call_stack_depth(32)` — but the actual rhai 1.x API is
  `set_max_call_levels(...)`.
  - **Chosen:** Use the real API in code; updated contract `02-host-fns.md`
    to reflect the correct method name with a note that
    `set_max_call_stack_depth` was the conceptual name in spec-v2.
  - **Reason:** Code must match the library; spec was aspirational.
    Avoids the next reviewer hitting the same trap.

## Backlog
task-6 (real `exec` / `exec_allow_fail` impls), task-7 (parse/log/args),
task-8 (CLI), task-9 (examples + integration tests), task-10
(measurement + README).

## User Action Needed
None. `/dump-commit` then `/chief-autopilot only focus on task-6`.
