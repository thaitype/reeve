# Verification — Test Matrix Ownership

## Rule

Every item in a milestone's test matrix (`_contract/XX-test-matrix.md`)
must be traceable to a specific task in `_plan/_todo.md`. Items with no
owner task are silently skipped during autopilot execution.

## How to apply

**At planning time (chief-agent):**
When writing `_plan/_todo.md`, each task entry must reference the test
matrix rows it covers. Example:

```md
- [ ] **task-3** — Layer 1 FS host functions.
      ...
      Tests: B1–B5, H1–H5 from `_contract/04-test-matrix.md`.
```

Before starting autopilot, do a coverage pass: every row in the test
matrix must appear in at least one task. Rows with no owning task
must be assigned before execution begins or explicitly deferred with
a backlog entry.

**At retro time (chief-agent):**
Check the test matrix against what was delivered. Any row not covered
by a passing test is an open item — add it to the next milestone's
backlog or fix it before closing the current milestone.

## Why

H15 ("invalid `security.yaml` → `cargo build` fails") was in the
milestone-2 contract test matrix but owned by no task. It was silently
skipped by the autopilot, by the TDD fix round, and by the second fix
round. It remains unimplemented. The only reason it was noticed at all
was the retro's coverage check.

## Origin

Milestone 2 retro — H15 had no task owner and was skipped by all
three implementation rounds.
