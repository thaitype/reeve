# Contract — Rhai Runtime Error Map Shape

## Rule

Any host function that signals an error to a Rhai script MUST throw an
`EvalAltResult::ErrorRuntime(Dynamic, Position::NONE)` where the
`Dynamic` is a `rhai::Map` containing AT MINIMUM:

```rhai
#{ kind: "<ErrorName>", ...payload }
```

- `kind` is a unique, stable, PascalCase string identifying the error
  class. Scripts may match on `kind` to branch.
- Additional payload fields are typed (string / int) and documented
  per error in the active milestone's
  `_contract/02-host-fns.md` §"Throws" table.

## Why

The CLI's `classify_error` function (in `src/bin/reeve.rs`)
inspects the `kind` field to choose the process exit code:

- Pact-violation kinds → exit code 2.
- Everything else → exit code 1.

Tools downstream of reeve (CI parsers, agents, MCP servers) will rely
on the same `kind` discriminator to route errors. A consistent shape is
contract-level, not stylistic.

## How to apply

When adding a new host function that can fail:
1. Decide the `kind` name (PascalCase, descriptive,
   non-conflicting with existing kinds).
2. Decide the payload fields (binary, path, limit_ms, etc.).
3. Add a row to the active milestone's
   `_contract/02-host-fns.md` §"Throws" table.
4. Update `classify_error` in the CLI if the new kind belongs to a
   non-default exit-code class.
5. Implement the throw using the Rhai map-error helper pattern
   established in `src/core/executor.rs`.

When introducing a new exit-code class:
1. Update `draft/spec-v2.md` §"Exit codes" or whichever ratified
   contract owns it.
2. Update `classify_error`.
3. Update the milestone goal's "Done when" if it's gating.

## Origin

Pattern emerged across milestone 1 tasks 6 (executor errors), 7
(parser errors), 8 (CLI classification). Codifying before Layer 1 FS
host fns or audit instrumentation add new error classes.
