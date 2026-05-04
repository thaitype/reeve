# Milestone 1 — Key Decisions (from grill session)

Decisions made during `/chief-plan` grill on 2026-05-04 that bind milestone 1
implementation. Background context lives in `draft/spec-v2.md` and
`.chief/_grill/closed/0001-reeve-design.md`.

## D1 — Per-OS path resolution in pact YAML

`binaries.<name>.path` is an object keyed by OS name plus an optional
`default`. Loader picks the host-OS array if present, else `default`. Within
the chosen array, picks the first existing absolute path. If none resolve,
throw `BinaryNotFound` at engine startup (fail fast, before any script runs).

```yaml
binaries:
  uname:
    path:
      default: [/usr/bin/uname]      # used when no OS override matches
  hostname:
    path:
      linux: [/bin/hostname, /usr/bin/hostname]
      macos: [/bin/hostname]
```

Rationale: single pact file, paths still absolute (security invariant
preserved), distro variance handled by the array. Supported OS keys for
milestone 1: `linux`, `macos`.

## D2 — Test-only pact, never embedded in release

`pacts/test-fixtures.yaml` contains `sleep` and `yes` for exercising
timeout + output-cap. It is loaded **only** behind `#[cfg(test)]` via a
test-only `include_str!` constant. Production builds embed only
`pacts/linux-readonly.yaml`.

Rationale: keeps the shipped allowlist honest; reviewers see only binaries a
real script would call.

## D3 — Single embedded preset, always active

Milestone 1 has no preset selection. `linux-readonly` is the only embedded
production pact and is always the active policy for `reeve run`. Multi-
preset selection is a `reeve-flex` concern, fully deferred.

Consequence: the increment-1 test "missing pact preset → exit 3" is
**dropped from milestone 1's must-pass list**; it does not apply when there
is no selection mechanism.

## D4 — Toolchain and lint posture

- Rust edition: 2021.
- MSRV: 1.75.
- Workspace lints: `clippy::all` + `-D warnings`.
- `clippy::pedantic` is **deferred** to a later milestone (one focused
  cleanup pass once the architecture stabilizes).
