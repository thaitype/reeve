# Standard — Verify Library API Names Before Coding

## Rule

When a task spec, design doc, or contract references a third-party
library API by name (e.g. `engine.set_max_call_stack_depth(32)`), the
implementing builder MUST verify that name against the actual library
version pinned in `Cargo.toml` (or equivalent) before writing the call
site.

If the spec name is wrong:
1. Use the real API name in code.
2. Update the contract / spec with a one-line correction note in the
   same change set, including a comment that names the conceptual
   reference (so future readers see the mapping).

## Why

Specs evolve faster than they get re-checked against libraries.
Aspirational or conceptual names creep in. Catching this at "the
compiler said no" stage is cheap; catching it after refactoring around
a fictional API is not.

## How to apply

Before opening an editor for a new module:

- Open the library's docs.rs page or grep its source for the symbol.
- Confirm the function exists with the documented signature in the
  pinned version.
- If anything's off, surface it: name the real API, fix the contract,
  proceed.

For tasks that delegate to builder-agent: the prompt should already
quote the spec API. The builder is responsible for verification, not
the planner.

## Origin

Milestone 1, batch 5 — spec-v2 used the conceptual name
`set_max_call_stack_depth(32)` for what `rhai 1.x` actually exposes as
`set_max_call_levels(32)`. Builder caught it during writing; contract
was updated mid-flight. Cheap fix, but cheaper to verify upfront.
