# Standard — Test Artifact Placement

## Rule

Any test fixture (YAML, JSON, sample data, mock binaries, sample
scripts, etc.) that is gated by `#[cfg(test)]` or only consumed by a
test suite MUST live under the consuming crate's `tests/` directory:

```
crates/<crate-name>/tests/fixtures/<fixture-name>.<ext>
```

Test fixtures MUST NOT live in workspace-root directories that hold
production assets — for example `pacts/`, `assets/`, `templates/`,
`config/`. These directories are reserved for files that ship in
release artifacts.

## Why

A `pacts/` directory next to a production allowlist is read by
reviewers as "every file in here is policy that ships." A test fixture
sitting alongside it blurs that signal. Worse, when packaging metadata
later adds something like `include = ["pacts/**"]` (Cargo publish,
Docker `COPY`, release tarball glob), the test fixture ships too —
silently — unless someone remembers to add an exclusion.

`#[cfg(test)]` only protects the *Rust constant* that embeds the file.
It does not protect the file on disk.

## How to apply

When adding a fixture:
1. Place the file under `crates/<consuming-crate>/tests/fixtures/`.
2. `include_str!` it from a `#[cfg(test)]` constant in the same crate.
3. If multiple crates need the same fixture, copy it (small files) or
   factor a `*-testkit` crate (large or many files). Don't reach across
   crates into another's `tests/` tree.

## Origin

Milestone 1, batch 4 — `pacts/test-fixtures.yaml` was initially placed
next to `pacts/linux-readonly.yaml`. User flagged it during review;
fixture moved to `crates/reeve-pact/tests/fixtures/test-fixtures.yaml`.
