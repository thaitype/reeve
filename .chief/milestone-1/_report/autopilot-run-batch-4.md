# Autopilot Run Batch 4

## Mode
auto (scoped to task-4 only)

## Summary
Embedded `pacts/linux-readonly.yaml` (production) into `reeve-pact` via
`include_str!`, and `crates/reeve-pact/tests/fixtures/test-fixtures.yaml`
(test-only) behind `#[cfg(test)]`. Added `presets::linux_readonly()` and
(cfg-gated) `presets::test_fixtures()` constructors with `OnceLock`
caching. 31 tests pass; release build confirmed test pact is excluded.

## Tasks Completed
- task-4 — Embedded presets, runtime parse-cache, four sanity tests.

## Decisions Made (auto mode)
- **Issue:** True compile-time YAML validation would require a proc-macro.
  - **Chosen:** Pragmatic guard — `OnceLock` + `expect()` plus a `#[test]`
    that parses on every `cargo test`.
  - **Reason:** Catches malformed YAML before any release ship without
    pulling in `proc-macro2`/syn-style machinery. Re-evaluate if it's
    ever a real concern.

- **Issue:** `pacts/` not added to `Cargo.toml` `include` list.
  - **Chosen:** Defer.
  - **Reason:** Milestone 1 builds from a workspace checkout; publishing
    to crates.io isn't on the table. Add when distribution lands.

## Post-batch correction
Initial implementation placed `test-fixtures.yaml` inside the workspace
`pacts/` directory next to `linux-readonly.yaml`. User flagged this:
co-locating test and production pacts blurs the security-review surface
and risks accidental shipment if `pacts/**` ever becomes a Cargo
`include` glob. Corrected layout:

```
pacts/linux-readonly.yaml                              # prod only
crates/reeve-pact/tests/fixtures/test-fixtures.yaml   # test only
```

`include_str!` path in `presets.rs` updated to
`"../tests/fixtures/test-fixtures.yaml"`. Contract
`_contract/01-pact-schema.md` §"Test-only pact" updated with the new
path and rationale. All 31 tests still pass; clippy clean.

**Lesson for future tasks:** test-only assets belong under the consuming
crate's own `tests/` tree, not the workspace's production asset
directories. D2's intent is "never embedded in release" *and* "never
visible alongside production policy" — the test only said the former.

## Backlog
task-5 through task-10 remain (reeve-core engine, exec, host fns, CLI,
examples, measurement).

## User Action Needed
None. `/dump-commit` then `/chief-autopilot only focus on task-5`.
