# Autopilot Run Batch 1

## Mode
auto (scoped to task-1 only per user directive)

## Summary
Scaffolded the Cargo workspace for milestone 1: three crates
(`reeve-core`, `reeve-pact`, `reeve`), edition 2021, MSRV 1.75,
workspace-wide deny-warnings, all stub builds and clippy clean.

## Tasks Completed
- task-1 — Cargo workspace scaffolded; `cargo build`, `cargo test`,
  `cargo clippy --workspace --all-targets` all pass with zero warnings.

## Decisions Made (auto mode)
- **Issue:** Rust toolchain version pinning — pin to a specific stable
  version, or track latest stable.
  - **Options:** pin a date-stable (e.g. 1.83) for full reproducibility,
    or `channel = "stable"` for minimal maintenance.
  - **Chosen:** `channel = "stable"` (no version).
  - **Reason:** Matches D4 (MSRV 1.75 in `Cargo.toml`); avoids bumping
    the toolchain pin every release. Re-evaluate if CI flakes.

- **Issue:** `rhai` resolved to 1.24.0 (semver-compatible with spec's
  `"1.19"`).
  - **Chosen:** Accept the minor bump.
  - **Reason:** Inside the declared semver range; spec uses `"1.19"` as
    a floor, not a ceiling.

- **Issue:** `thiserror` v2 available but spec pins `"1"`.
  - **Chosen:** Stay on v1 (Cargo picked 1.0.69).
  - **Reason:** Honor the spec; v2 migration is a separate decision.

- **Issue:** Rust not pre-installed on dev machine.
  - **Chosen:** Installed via `brew install rust` (1.95.0).
  - **Reason:** Required to verify the workspace builds. One-time setup.

## Backlog
task-2 through task-10 remain (see `_plan/_todo.md`).

## User Action Needed
None. Run `/dump-commit` when ready, then `/chief-autopilot` (optionally
scoped) to continue with task-2.
