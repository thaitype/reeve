# Autopilot Run Batch 10.1

## Mode
auto (scoped to task-10.1 only)

## Summary
Renamed `linux-readonly` → `unix-readonly` across all shipping
artifacts. The pact runs on Linux AND macOS via the per-OS path
resolver (D1); macOS is Darwin/BSD, not Linux — old name was technically
wrong. 60 tests still pass; clippy clean; zero leftover references in
shipping paths.

## Tasks Completed
- task-10.1 — Rename pact file, YAML `name:`, Rust constants/fns/tests,
  re-exports, README mention, and contract preset block.

## Files modified
- `pacts/unix-readonly.yaml` (renamed from `linux-readonly.yaml`)
- `crates/reeve-pact/src/presets.rs` (const, fn, tests)
- `crates/reeve-pact/src/lib.rs` (re-export)
- `crates/reeve-pact/src/engine.rs` (inline test YAML + call sites)
- `crates/reeve-pact/src/parse.rs` (inline YAML test fixture)
- `crates/reeve-core/src/executor.rs` (pact lookup + comments)
- `crates/reeve/tests/cli.rs` (comment)
- `README.md`
- `.chief/milestone-1/_contract/01-pact-schema.md` §"Embedded preset"

## Decisions Made (auto mode)
- **Issue:** Whether to rename historical context (drafts, grill log,
  past batch reports).
  - **Chosen:** No — leave historical refs as-is.
  - **Reason:** Drafts and reports are time-stamped artifacts. Editing
    them would falsify the historical record. The rename only applies
    to shipping artifacts and forward-looking contracts.

## Verification
- `cargo build --workspace` — zero warnings.
- `cargo test --workspace` — 60 passed (7 cli + 22 reeve-core + 31
  reeve-pact).
- `cargo clippy --workspace --all-targets` — zero warnings.
- grep `linux-readonly|linux_readonly` in `crates/`, `pacts/`,
  `README.md` — zero hits.

## Backlog
task-11 (GitHub Actions CI matrix) remains.

## User Action Needed
None. `/dump-commit` then optionally `/chief-autopilot only focus on
task-11`.
