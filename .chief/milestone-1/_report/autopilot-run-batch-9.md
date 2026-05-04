# Autopilot Run Batch 9

## Mode
auto (scoped to task-9 only)

## Summary
Added `examples/noop.rhai` (used for cold-start measurement) and
`examples/sysinfo.rhai` (Linux+macOS happy path). Added the row #14
end-to-end CLI test. Audited every row of the bypass-resistance matrix
against the codebase — all 14 rows have a passing test. Workspace
totals: **60 tests pass**; clippy clean; release binary 4.9 MB.

## Tasks Completed
- task-9 — Examples + row #14 test + full coverage audit.

## Coverage audit (canonical map)

| Row | Description | Test location |
|-----|-------------|---------------|
| 1 | rm not allowed | `reeve-pact/src/engine.rs::rejects_unknown_binary` |
| 2 | uname -X | `reeve-pact/src/engine.rs::rejects_unknown_flag` |
| 3 | echo metachar `;` | `reeve-pact/src/engine.rs::rejects_string_metachar_positional` |
| 4 | echo `$` | `reeve-pact/src/engine.rs::rejects_dollar_sign_positional` |
| 5 | echo `\n` | `reeve-pact/src/engine.rs::rejects_newline_positional` |
| 6 | whoami extra positional | `reeve-pact/src/engine.rs::rejects_extra_positional_on_whoami` |
| 7 | import "fs" | `reeve-core/src/engine.rs::rejects_module_import` |
| 8 | eval | `reeve-core/src/engine.rs::rejects_eval` |
| 9 | max operations | `reeve-core/src/engine.rs::rejects_excessive_operations` |
| 10 | sleep timeout | `reeve-core/src/executor.rs::sleep_exceeding_timeout_throws_timeout` |
| 11 | yes output cap | `reeve-core/src/executor.rs::yes_exceeds_output_cap` |
| 12 | --pact rejected by clap | `reeve/tests/cli.rs::unknown_flag_pact_rejected_by_clap` |
| 13 | missing script file | `reeve/tests/cli.rs::missing_script_file_exits_3` |
| 14 | sysinfo end-to-end | `reeve/tests/cli.rs::examples_sysinfo_runs_end_to_end` |

## Decisions Made (auto mode)
- **Issue:** Rhai's `trim()` mutates in place and returns `()` — caught
  during `sysinfo.rhai` authoring.
  - **Chosen:** Call `trim()` as a statement on the variable, then
    interpolate the variable into the template literal.
  - **Reason:** Matches Rhai's actual API. Documenting here so future
    examples don't trip on `let t = s.trim()` returning unit.

- **Issue:** CI matrix.
  - **Chosen:** Defer (no `.github/workflows/` added).
  - **Reason:** Per task spec, manual verification is the milestone-1
    contract. macOS verified locally (darwin 25.2.0/arm64). Linux
    verification + GitHub Actions matrix folded into a future
    milestone.

- **Issue:** Pact edits needed for example to run cross-platform?
  - **Chosen:** None required.
  - **Reason:** `linux-readonly` pact already has paths + flags for
    `whoami`, `hostname -s`, `uname -a`, `date -I` on both OSes.

## Backlog
task-10 (measurement: cold-start timing + binary size into a report;
README "what is this" + "try it" sections).

## User Action Needed
None. `/dump-commit` then `/chief-autopilot only focus on task-10`.
