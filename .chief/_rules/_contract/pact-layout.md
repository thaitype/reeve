# Contract — Pact File Layout

## Rule

### Production pacts

Live at workspace root:

```
pacts/<scope>-<posture>.yaml
```

- `<scope>` names the binary family or domain: `linux`, `git`, `k8s`,
  `aws`, `db`, etc.
- `<posture>` declares the intent:
  - `readonly` — only side-effect-free operations are allowlisted.
  - `rw` — read AND write operations are allowlisted (use sparingly;
    requires extra reviewer scrutiny).
  - Other postures may be added when justified (e.g. `audit`,
    `restricted`).

Example: `pacts/linux-readonly.yaml`, `pacts/k8s-readonly.yaml`,
`pacts/git-readonly.yaml`.

### Test pacts

Live under the consuming crate's test tree (per
`_standard/test-artifacts.md`):

```
crates/<crate-name>/tests/fixtures/<name>.yaml
```

Naming is free-form here (e.g. `test-fixtures.yaml`,
`malformed-version.yaml`) — these never ship.

## Why

A consistent pacts directory layout makes the security review surface
obvious. A reviewer auditing what binaries can be called only needs to
read `pacts/*.yaml` — no hunting through test trees or hidden
fixtures.

The naming convention prevents bikeshedding when adding new presets
and signals posture (`-readonly` vs `-rw`) at-a-glance.

## How to apply

When adding a new pact:
1. Decide scope (`<scope>`) — pick a single-word domain.
2. Decide posture — start `-readonly` unless the use case demands
   writes.
3. Place file at `pacts/<scope>-<posture>.yaml`.
4. Embed via `include_str!` in `crates/warden-pact/src/presets.rs`,
   add a constructor `pub fn <scope>_<posture>() -> &'static Pact`.
5. Document the binaries + their risk-class in the file's header
   comment.

## Origin

Milestone 1 established `linux-readonly` as the precedent. Codifying
the pattern now (before `k8s-readonly`, `git-readonly`, etc. land)
prevents future drift.
