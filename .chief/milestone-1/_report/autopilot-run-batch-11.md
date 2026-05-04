# Autopilot Run Batch 11

## Mode
auto (scoped to task-11 only)

## Summary
Collapsed three workspace crates into one (`crates/reeve` with `core/`
and `pact/` modules), bumped to `0.1.0` with full crates.io metadata,
added `.github/workflows/publish.yml` (workflow_dispatch with dry-run
input), updated affected docs. **60 tests pass** (53 lib unit + 7 CLI
integration), clippy clean, `cargo publish --dry-run` clean.

## Tasks Completed
- task-11 — Workspace collapse, publish metadata, GitHub Actions
  workflow, doc updates.

## Decisions Made (auto mode)

- **Issue:** Builder initially exposed `pub mod core; pub mod pact;`
  in `lib.rs` and `pub use engine::{build_engine, build_engine_with_args};`
  — broader public API than the Q2 grill decision allowed.
  - **Chosen:** Tightened to `mod core; mod pact;` (private modules)
    plus a single `pub use crate::core::build_engine_with_args;` at
    the lib root. `build_engine()` becomes test-only (`#[cfg(test)]`).
  - **Reason:** Q2 said "no public library API." The bin needs at
    least one public symbol because `src/bin/*.rs` is a separate
    compilation unit linking the lib. One symbol is the minimum to
    keep the bin working; everything else stays internal. `cargo add
    reeve` now exposes only `reeve::build_engine_with_args`, which
    we can keep stable trivially.

- **Issue:** Builder created `crates/reeve/pacts/unix-readonly.yaml`
  as a duplicate of the workspace-root `pacts/unix-readonly.yaml`
  (because `cargo publish` only packages files under the crate dir).
  Two sources of truth → drift risk.
  - **Chosen:** Deleted the workspace-root copy. Single canonical
    location: `crates/reeve/pacts/unix-readonly.yaml`. Updated
    `_rules/_contract/pact-layout.md` to reflect the new layout
    (production pacts live inside the publishable crate, not at
    workspace root).
  - **Reason:** Single-crate workspace makes "workspace root vs
    crate" distinction meaningless. The crate is the unit that
    ships; pacts live with it.

- **Issue:** Tooling — `cargo-release` vs `release-plz` vs the
  hand-rolled workflow.
  - **Chosen:** Hand-rolled `workflow_dispatch` workflow for v0.1.
  - **Reason:** Per builder eval — `cargo-release` automates version
    bumps + tagging but assumes tag-triggered release flow; `release-plz`
    adds changelog + PR-based version management, valuable at
    multi-release-per-month cadence. Neither earns its complexity for
    a single-crate occasional-release project. Re-evaluate
    `release-plz` if cadence picks up.

## Verification

| Check | Result |
|---|---|
| `cargo build --workspace` | zero warnings |
| `cargo test --workspace` | 60 tests pass (53 lib unit + 7 CLI) |
| `cargo clippy --workspace --all-targets -- -D warnings` | zero warnings |
| `cargo publish --dry-run -p reeve --allow-dirty` | clean |
| `cargo run --release -p reeve -- run examples/sysinfo.rhai` | sysinfo report prints |
| `cargo run --release -p reeve -- run examples/noop.rhai` | exits 0 silently |
| `cargo run --release -p reeve -- version` | prints `0.1.0` |

## Files

**Moved (via `git mv`, history preserved):**
- 11 source files: `crates/reeve-{core,pact}/src/*.rs` →
  `crates/reeve/src/{core,pact}/*.rs`
- 1 binary: `crates/reeve/src/main.rs` → `crates/reeve/src/bin/reeve.rs`
- 1 test fixture: `crates/reeve-pact/tests/fixtures/test-fixtures.yaml`
  → `crates/reeve/tests/fixtures/test-fixtures.yaml`

**Deleted:**
- `crates/reeve-core/` (empty after moves)
- `crates/reeve-pact/` (empty after moves)
- `pacts/unix-readonly.yaml` (workspace-root duplicate)

**New:**
- `crates/reeve/src/lib.rs` (single `pub use`)
- `crates/reeve/src/core/mod.rs`, `src/pact/mod.rs`
- `crates/reeve/pacts/unix-readonly.yaml` (canonical location)
- `.github/workflows/publish.yml`

**Modified:**
- Workspace `Cargo.toml` (`members = ["crates/reeve"]`)
- `crates/reeve/Cargo.toml` (merged deps + publish metadata)
- `.chief/milestone-1/_contract/01-pact-schema.md` (path updates)
- `.chief/_rules/_contract/pact-layout.md` (single-crate layout)
- `.chief/_rules/_standard/test-artifacts.md` (cross-crate bullet
  removed; single-crate note added)
- `README.md` (Development section updated for one-crate layout)

## Backlog
None for milestone 1. Cross-milestone parking lot at `.chief/_backlog.md`
unchanged (CI matrix item, `serde_yaml` migration).

## User Action Needed

Three follow-ups before the workflow is fully usable:

1. **Add `CARGO_REGISTRY_TOKEN` secret** to the GitHub repo settings
   (Settings → Secrets and variables → Actions). Required for the
   publish job. Get the token from <https://crates.io/me>.
2. **First release run:** trigger the workflow manually via the
   GitHub Actions UI with `version = 0.1.0`, `dry_run = true`. Verify
   the gate + dry-run publish succeed. Then re-run with `dry_run =
   false` to publish. Push `v0.1.0` git tag locally afterward for
   traceability.
3. **`/dump-commit`** to seal the local changes.
