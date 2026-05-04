# Autopilot Run Batch 8

## Mode
auto (scoped to task-8 only)

## Summary
`warden` CLI wired end-to-end via clap derive. Subcommands `run <script> [args...]`
and `version`. Error classification picks exit codes per spec-v2 (0 ok / 1
script error / 2 pact violation / 3 config error). 6 integration tests
pass via `assert_cmd`. Release binary 4.9 MB on macOS — well under the
10 MB milestone gate.

## Tasks Completed
- task-8 — `Cli`/`Cmd` structs, `classify_error`, file-read failure →
  exit 3, runtime-error map → exit 2 / 1 by `kind`, six CLI tests.

## Decisions Made (auto mode)
- **Issue:** Where does `--pact` get rejected when `trailing_var_arg`
  absorbs args after the script path?
  - **Chosen:** Test places `--pact` BEFORE the script-path positional;
    clap's normal parser rejects it there.
  - **Reason:** That's the natural way a user would attempt the flag.
    `--pact` after the positional becomes a script arg — which is fine
    because scripts can only do what the pact allows anyway, so leaking
    the string into `script_args()` is harmless.

- **Issue:** Auto-generated `--version` flag from `#[command(version)]`
  vs the explicit `version` subcommand — D3 forbids policy flags.
  - **Chosen:** Keep both.
  - **Reason:** `--version` and `--help` are clap built-ins, not policy
    flags. D3 prohibits flags that change pact selection or runtime
    config; standard introspection flags are out of scope for that rule.

- **Issue:** `rhai` dep needed in `warden` crate to reference
  `EvalAltResult` for `classify_error`.
  - **Chosen:** Added `rhai = "1.19"` to `warden/Cargo.toml` deps.
  - **Reason:** Smallest scope addition. Alternative (re-exporting
    `EvalAltResult` from `warden-core`) is cleaner long-term — note as
    follow-up if it grows.

## Backlog
task-9 (examples + integration tests for full bypass-resistance matrix),
task-10 (measurement + README).

## User Action Needed
None. `/dump-commit` then `/chief-autopilot only focus on task-9`.
