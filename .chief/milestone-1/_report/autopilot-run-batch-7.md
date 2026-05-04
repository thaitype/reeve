# Autopilot Run Batch 7

## Mode
auto (scoped to task-7 only)

## Summary
Replaced the remaining seven host-fn stubs with real implementations:
`parse_json`/`parse_yaml` via `serde_json`/`serde_yaml` →
`rhai::serde::to_dynamic`; `log_info/warn/error` via `chrono` UTC RFC3339
formatter; `print` via `engine.on_print`; `script_args` via captured
`Arc<Vec<String>>` and a new `build_engine_with_args(args)` constructor.
22 tests pass, clippy clean.

## Tasks Completed
- task-7 — `parse.rs`, `logging.rs`, real wiring in `engine.rs`,
  `build_engine_with_args` for CLI integration.

## Decisions Made (auto mode)
- **Issue:** Enable `rhai`'s `serde` feature to use `to_dynamic`?
  - **Chosen:** Yes — `features = ["sync", "serde"]`.
  - **Reason:** Required for `parse_json`/`parse_yaml`. Tiny compile-time
    cost; no new external dep.

- **Issue:** `print` registration — `register_fn` overload vs
  `engine.on_print`.
  - **Chosen:** `engine.on_print(logging::print_line)`.
  - **Reason:** Rhai's built-in `print` handles variadic string
    formatting and routes to the `on_print` callback. Saves us the
    overload juggling.

- **Issue:** `script_args` needs CLI args at engine-construction time.
  - **Chosen:** Two constructors — `build_engine()` for tests / no-args
    callers, `build_engine_with_args(Vec<String>)` for the CLI.
  - **Reason:** Keeps a clean default while letting task-8's CLI pass
    args without a global mutable state. `Arc<Vec<String>>` capture
    satisfies the `'static + Send + Sync` requirement Rhai's `sync`
    feature imposes.

## Backlog
task-8 (warden CLI), task-9 (examples + integration tests), task-10
(measurement + README).

## User Action Needed
None. `/dump-commit` then `/chief-autopilot only focus on task-8`.
