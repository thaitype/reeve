# Autopilot Run Batch 11.1

## Mode
auto (scoped to task-11.1 only — within-milestone followup to task-11)

## Summary
Flattened the project layout: `crates/reeve/{src,tests,pacts,Cargo.toml}`
moved to repo root; `crates/` dir and workspace `Cargo.toml` deleted.
Top-level `Cargo.toml` is now a regular `[package]`, not `[workspace]`.
60 tests pass, clippy clean, `cargo publish --dry-run` clean.

## Tasks Completed
- task-11.1 — Flat layout move + 4 forward-looking docs + workflow YAML.

## Decisions Made (auto mode — captured during /grill-me)

- **Q1: Flat single-crate layout vs keep workspace.**
  - **Chosen:** Flat. `Cargo.toml` is `[package]`; no `[workspace]`.
  - **Reason:** Workspace was only earning its keep when there were
    three crates; with one crate it's noise. If a sibling ever shows
    up (`reeve-mcp-server`, etc.) the migration back is mechanical
    (~10 minutes of `git mv`). Don't pre-engineer for hypotheticals.

- **Q2: How to track this work.**
  - **Chosen:** task-11.1, peer to task-10.1.
  - **Reason:** Same precedent — refactor that touches milestone-1
    structure but lands after the milestone is "done." `.1`-suffix
    explicitly signals "post-milestone followup, in scope."

- **Q3: Doc-update breadth.**
  - **Chosen:** Forward-looking only (7 of 11 affected files).
  - **Reason:** Historical reports and the retro are time-stamped
    records of state-at-batch-N. Rewriting them lies about history.
    Same precedent as both prior renames (warden→reeve,
    linux-readonly→unix-readonly).

## Decisions made during execution

- **Issue:** `tests/cli.rs` row-#14 test resolved `examples/sysinfo.rhai`
  via `CARGO_MANIFEST_DIR + "../../examples/..."` (assuming `crates/reeve`
  was the manifest dir). After flatten, `CARGO_MANIFEST_DIR` IS the
  repo root, so the `../..` walk overshot.
  - **Fix:** dropped the `../../` prefix; test now resolves
    `<MANIFEST_DIR>/examples/sysinfo.rhai`.

- **Issue:** Workflow YAML referenced `cargo test --workspace` and
  `-p reeve` flags that no longer apply.
  - **Fix:** simplified to plain `cargo test` and dropped `-p reeve`
    everywhere. Behavior unchanged.

- **Issue:** `Cargo.toml` `readme = "../../README.md"` pointed across
  the dropped directory levels.
  - **Fix:** `readme = "README.md"` (root-relative).

## Verification

| Check | Result |
|---|---|
| `cargo build` | zero warnings |
| `cargo test` | 60 tests pass (53 lib unit + 7 CLI) |
| `cargo clippy --all-targets -- -D warnings` | zero warnings |
| `cargo publish --dry-run --allow-dirty` | clean |

## Files

**Moved (via `git mv`, history preserved):**
- `crates/reeve/src/` → `src/`
- `crates/reeve/tests/` → `tests/`
- `crates/reeve/pacts/` → `pacts/`
- `crates/reeve/Cargo.toml` → `Cargo.toml` (replacing the old
  workspace `Cargo.toml`)

**Deleted:**
- `crates/reeve/`
- `crates/`
- workspace-level `Cargo.toml` (subsumed by the package one)

**Modified (forward-looking docs):**
- `Cargo.toml` (merged: package metadata from old crate + lints from
  old workspace)
- `tests/cli.rs` (manifest-dir path fix)
- `.github/workflows/publish.yml` (`-p reeve`, `--workspace` removed)
- `README.md` (Install + Development sections)
- `.chief/_rules/_contract/pact-layout.md`
- `.chief/_rules/_contract/error-maps.md`
- `.chief/_rules/_standard/test-artifacts.md`
- `.chief/milestone-1/_contract/01-pact-schema.md`
- `.chief/milestone-1/_plan/_todo.md` (task-11.1 entry)

**Intentionally NOT modified (historical):**
- `_report/autopilot-run-batch-{4,6,10.1,11}.md`
- `_report/retro-milestone.md`
- `_report/measurement.md`
- `_grill/closed/0001-reeve-design.md`
- Anything in `draft/`

## Backlog
None changed. Cross-milestone parking lot at `.chief/_backlog.md`
unchanged (CI matrix, `serde_yaml` migration).

## User Action Needed
- `/dump-commit` to seal.
