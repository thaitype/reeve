# Autopilot Run Batch 3

## Mode
auto (scoped to task-3 only)

## Summary
Implemented `reeve-pact` kind validators (`enum`/`number`/`string` with
shell-metacharacter blocklist) and the generic allowlist engine
(`validate_call`). Added `PactError` matching the contract's error map
shapes. 27 tests pass; clippy clean.

## Tasks Completed
- task-3 — `kinds::validate`, `engine::validate_call`, `ResolvedExec`,
  `PactError` (6 variants per contract).

## Decisions Made (auto mode)
- **Issue:** No `required` marker exists on `PositionalSpec`; what to do
  when positional specs are unfilled?
  - **Chosen:** Skip required-positional checking in milestone 1.
  - **Reason:** Schema has no marker (only `optional` and `repeated`).
    Adding it now would expand scope. Re-evaluate when a preset needs it
    (likely with `k8s-readonly`'s `k8s_name` positionals).

- **Issue:** Contract's `02-host-fns.md` lists `BinaryNotFound` as a
  startup-time error, but milestone 1 has no separate startup phase —
  path resolution runs at `validate_call` time.
  - **Chosen:** Named the variant `BinaryNotResolvable`; task-6 will map
    it to the script-visible `BinaryNotFound` kind in the Rhai error map.
  - **Reason:** Keeps Rust-side naming honest about WHEN the error
    fires; preserves the contract's external naming for scripts.

- **Issue:** Flag-value parsing — support `--flag=value` form?
  - **Chosen:** Space-separated only (`--flag value`).
  - **Reason:** No milestone-1 binary uses `=`-form. Easier to add later
    than to support and quietly break.

- **Issue:** `WithSubcommands` test coverage — no binary in
  `linux-readonly` uses subcommands.
  - **Chosen:** Skipped the engine test; branch is reachable but not
    exercised in milestone 1.
  - **Reason:** Will be covered when `k8s-readonly` lands (kubectl uses
    subcommands).

## Backlog
task-4 through task-10 remain.

## User Action Needed
None. `/dump-commit` then `/chief-autopilot only focus on task-4`.
