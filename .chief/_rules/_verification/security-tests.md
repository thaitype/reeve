# Verification — Security Bypass Tests

## Rule 1 — Bypass tests from grill sessions must land in the contract

Any security bypass-resistance test identified during a grill session
MUST be copied into the milestone's test matrix contract
(`_contract/XX-test-matrix.md`) before planning ends.

Grill logs (`_grill/`) are not consulted during autopilot execution.
If a bypass test lives only in the grill log, it will not be
implemented.

## Rule 2 — Security fixes must be proven via the production call path

When a security mechanism (path validation, env filtering, access
control) is implemented, the verification test MUST exercise the same
code path that a real script execution uses — not a test-only wrapper
or alternative constructor.

A test that calls `run_exec_with_passthrough` does NOT verify that
`run_exec_audited` filters env. Only a test that runs through
`run_exec_audited` (or the CLI binary end-to-end) closes the
verification gap.

## Why

**Rule 1:** The symlink escape bypass test was explicitly called out in
grill session `0001-reeve-design.md` but never promoted to the test
matrix. The autopilot implemented what was in the contract; the grill
log was not checked. A post-delivery review caught it — expensive.

**Rule 2:** SF-2 (child env leak) was "fixed" in a TDD round by
proving the mechanism in `run_exec_with_passthrough`. The production
path `run_exec_audited` still passed `None` and leaked env. Only a
second fix round (prompted by user) closed it. The test gave false
confidence.

## How to apply

**At planning time (chief-agent):**
- After each grill session, scan the grill log for any statement of
  the form "X → reject" or "X must throw" or "test that Y is denied."
- Copy each one as a row in the milestone test matrix before writing
  the first task spec.

**At implementation time (builder-agent):**
- For every security fix, identify the production entry point
  (the function called by real Rhai scripts or the CLI binary).
- The assertion must land on output from that entry point.
- If testing through the full binary is impractical, document
  explicitly which production function is being exercised and why
  a lower-level call is equivalent.

## Origin

Milestone 2 — SF-1 (symlink escape) and SF-2 (child env leak).
Both were identified only in post-delivery review, not during
implementation. SF-2 required two separate fix rounds because the
first fix proved the mechanism in a test wrapper, not the production
path.
