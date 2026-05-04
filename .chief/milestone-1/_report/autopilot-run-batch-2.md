# Autopilot Run Batch 2

## Mode
auto (scoped to task-2 only per user directive)

## Summary
Implemented `warden-pact` schema, parser, and error types per
`_contract/01-pact-schema.md`. 10 unit tests covering the full happy path
and every reject variant. Build, test, clippy all clean.

## Tasks Completed
- task-2 — `Pact`, `BinarySpec`, `BinaryBody` (XOR enum),
  `ActionSpec`, `PathSpec`, `KindSpec`, `PositionalSpec` + `parse_pact`
  with post-parse validation; `ParseError` via `thiserror`.

## Decisions Made (auto mode)
- **Issue:** Subcommands-vs-direct XOR — enforce at deserialize time
  (custom `Deserialize`) or at post-parse?
  - **Chosen:** Custom `Deserialize` via a flat `BinaryBodyRaw` helper.
  - **Reason:** Catches the conflict earlier and produces clearer YAML
    error spans. Side effect: `rejects_subcommands_with_direct` matches
    `ParseError::Yaml(_)` rather than the dedicated
    `SubcommandsConflictWithDirect` variant. Acceptable — the error
    still names the conflict; rename or refactor only if a downstream
    consumer needs to discriminate programmatically.

- **Issue:** Absolute-path check — `PathBuf::is_absolute()` or leading
  `/` byte?
  - **Chosen:** Leading `/` byte.
  - **Reason:** Pacts target POSIX absolute paths only (per D1 — OS keys
    are `linux`/`macos`). `is_absolute()` would false-positive on `C:\…`
    if the YAML is parsed under a Windows cross-compile context.

- **Issue:** Unknown `kind:` tags (e.g. `filepath`) — custom error or
  let serde reject?
  - **Chosen:** Let serde reject (no dedicated variant).
  - **Reason:** Adding new kinds is a code change anyway; the serde
    error is informative enough. Re-evaluate when `kind: custom` lands.

## Backlog
task-3 through task-10 remain.

## User Action Needed
None. `/dump-commit` then `/chief-autopilot only focus on task-3`.
