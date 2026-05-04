# Milestone 1 — TODO

Tasks are sized for builder-agent. Order is dependency-driven; later tasks
assume earlier ones landed. Do not reorder without updating dependencies.

- [x] **task-1** — Scaffold Cargo workspace (`reeve-core`, `reeve-pact`,
      `reeve`), pin edition 2021 + MSRV 1.75, set workspace lints
      (`clippy::all`, `-D warnings`), add baseline deps from
      `draft/spec-v2.md` §Dependencies, ensure `cargo build` + `cargo test`
      pass on an empty workspace.
- [x] **task-2** — `reeve-pact`: schema structs (`Pact`, `BinarySpec`,
      `KindSpec`, `PositionalSpec`) per `_contract/01-pact-schema.md`,
      including per-OS `path` resolution and absolute-path validation at
      parse time. Deny unknown YAML fields. Unit tests for parse + reject.
- [x] **task-3** — `reeve-pact`: kind validators for `enum`, `number`,
      `string` (with shell-metacharacter blocklist from `_contract/01`).
      Generic allowlist engine that takes a parsed `Pact` + `(binary, argv)`
      and returns `Ok(ResolvedExec { abs_path, argv })` or a typed
      `PactError`. Unit tests for every error variant in `_contract/02`.
- [x] **task-4** — Embed `pacts/linux-readonly.yaml` via `include_str!` into
      `reeve-pact` and parse-validate at compile time (compile fails if
      YAML is malformed). Add `pacts/test-fixtures.yaml` behind
      `#[cfg(test)]`. Provide `linux_readonly()` and (test-only)
      `test_fixtures()` constructor fns.
- [x] **task-5** — `reeve-core`: Rhai engine setup with the resource limits
      and disabled symbols listed in `_contract/02-host-fns.md`. Register
      stub host fns (return `()` for now). Unit tests covering the engine
      sandbox rows of `_contract/03` (#7–#9).
- [x] **task-6** — `reeve-core`: implement `exec` + `exec_allow_fail` per
      `_contract/02`. Use `std::process::Command`, `argv` array form, no
      shell. Per-exec timeout via thread + kill on expiry (or
      `wait_timeout` crate if simpler). Output cap via byte-counted stdout
      + stderr buffers, kill on overflow. Map `PactError` and runtime
      errors to the typed Rhai error maps in `_contract/02`. Add
      `executor::trace!` macro (single-line stderr per call).
- [x] **task-7** — `reeve-core`: implement `parse_json`, `parse_yaml`,
      `script_args`, `print`, `log_info/warn/error`. Wire `script_args`
      from the CLI through the engine scope.
- [x] **task-8** — `reeve` CLI (clap, derive): subcommands `run <script>`
      and `version`. No flags. Exit codes per `draft/spec-v2.md` §"Exit
      codes" (collapsed for milestone 1: 0 ok, 1 script error, 2 pact
      violation, 3 config error). Wire it all together.
- [x] **task-9** — Examples: `examples/sysinfo.rhai` and
      `examples/noop.rhai`. Integration tests #10–#14 from
      `_contract/03-test-matrix.md`. Run on Linux + macOS in CI matrix
      (or document manual verification if CI deferred).
- [x] **task-10** — Measurement + README. Build release binary, record
      size and cold-start in `.chief/milestone-1/_report/measurement.md`,
      fail if either threshold breached. Write README "what is this" +
      "try it" sections per `_goal/01-scope.md`.

## Within-milestone followups

- [x] **task-10.1** — Rename `linux-readonly` → `unix-readonly`
      everywhere. Completed in batch 10.1 (commit `62bc7d3`).

- [x] **task-11.1** — Flatten layout: move `crates/reeve/{src,tests,
      pacts,Cargo.toml}` to repo root, delete `crates/` directory and
      workspace `Cargo.toml`. Decisions captured in
      `_report/autopilot-run-batch-11.1.md`: flat single-crate layout
      committed (Q1), tracked here as a within-milestone followup
      (Q2), forward-looking docs only updated (Q3). Verification: 60
      tests pass, clippy clean, `cargo publish --dry-run` clean.

- [x] **task-11** — Collapse to a single `reeve` crate, then publish
      to crates.io and automate releases via GitHub Actions.
      Two-phase task; both phases ship together as one PR.

      ### Phase A — Workspace collapse

      Refactor the three workspace crates (`reeve-core`, `reeve-pact`,
      `reeve`) into a single `crates/reeve` crate. File contents move
      1:1; only the directory layout and module declarations change.

      Target layout:

      ```
      crates/reeve/
      ├── Cargo.toml                       # the only crate
      ├── src/
      │   ├── lib.rs                       # pub(crate) mod core; pub(crate) mod pact;
      │   ├── core/
      │   │   ├── mod.rs
      │   │   ├── engine.rs
      │   │   ├── executor.rs
      │   │   ├── parse.rs
      │   │   └── logging.rs
      │   ├── pact/
      │   │   ├── mod.rs
      │   │   ├── schema.rs
      │   │   ├── parse.rs
      │   │   ├── engine.rs
      │   │   ├── kinds.rs
      │   │   ├── error.rs
      │   │   └── presets.rs
      │   └── bin/
      │       └── reeve.rs                 # was crates/reeve/src/main.rs
      └── tests/
          ├── cli.rs                       # was crates/reeve/tests/cli.rs
          └── fixtures/
              └── test-fixtures.yaml       # was crates/reeve-pact/tests/fixtures/
      ```

      Decisions (from `/grill-me` Q1–Q4):
      - **Q1:** module names `core` and `pact` mirror the old crate
        boundaries; files move 1:1, no rewrites.
      - **Q2:** `pub(crate) mod core; pub(crate) mod pact;` — no
        public library API. Internal refactors stay free until
        someone asks for embedding.
      - **Q3:** binaries live under `src/bin/`. `reeve.rs` today;
        `reeve-flex.rs` slots next to it later as a peer.
      - **Q4:** `pub(crate) fn test_fixtures() -> &'static Pact` in
        `pact::presets` shared by both `core::executor` and
        `pact::presets` test modules. Removes the cross-crate
        `include_str!` workaround documented in batch-6.

      Workspace `Cargo.toml` collapses to a single `members =
      ["crates/reeve"]`. Workspace-wide lint config and `[workspace.package]`
      stay; only the members list shrinks. All `path = "../..."`
      deps disappear from the per-crate `Cargo.toml`.

      Phase A verification:
      `cargo build --workspace`, `cargo test --workspace`, and
      `cargo clippy --workspace --all-targets -- -D warnings` all
      clean. The 60-test count from milestone-1 must hold. Manual
      smoke: `cargo run --release -p reeve -- run examples/sysinfo.rhai`
      prints the expected report.

      ### Phase B — Publish + workflow

      Strategy: publish at `0.1.0` (conventional Rust starter); the
      `0.x.y` series is free to break per SemVer minor bumps until a
      v1.0.0 commitment.

      Scope:

      1. **Version + metadata.** Set `version = "0.1.0"` in
         `crates/reeve/Cargo.toml`. Add `description`, `license =
         "Apache-2.0"`, `repository`, `readme`, `keywords`,
         `categories` — `cargo publish --dry-run` will list anything
         missing.
      2. **Workflow.** `.github/workflows/publish.yml` triggered by
         `workflow_dispatch` (manual run from the GitHub Actions
         UI). Inputs: `version` (e.g. `0.1.0`) and a `dry_run`
         boolean (default `true`). Manual trigger keeps a human in
         the loop on every release; safer than tag-triggered while
         the workflow is new and avoids burning a version on a
         workflow bug. Switch to tag-triggered later if cadence
         demands it. Steps:
         (a) Gate: `cargo test --workspace` and `cargo clippy
         --workspace --all-targets -- -D warnings` on
         `{ubuntu-latest, macos-latest}` × stable Rust.
         (b) Dry-run check: `cargo publish --dry-run -p reeve` —
         catches metadata problems without touching crates.io.
         (c) Publish: `cargo publish -p reeve` (only when
         `dry_run = false`). One crate, no ordering, no polls, no
         path-dep pins.
         (d) Release binaries: build
         `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`,
         upload to the GitHub Release.
         Requires `CARGO_REGISTRY_TOKEN` secret in repo settings.
      3. **Tooling.** Evaluate `cargo-release` or `release-plz` for
         version-bump + tag automation; pick one or document why
         we're rolling our own. Lower priority than the workflow
         itself — can land in a follow-up PR.

      Phase B verification:
      First run via `workflow_dispatch` with `dry_run = true`
      confirms the workflow logic, gate, and dry-run publish all
      succeed without touching crates.io. Then re-run with
      `dry_run = false` to publish for real. If the real run fails
      after publish (e.g. binary upload errors), the `0.1.0` version
      is burned — bump to `0.1.1` and re-run. After successful
      publish, create a `v0.1.0` git tag locally and push it for
      traceability (tag is record-keeping, not the trigger).
      Subsequent releases bump per SemVer (`0.1.1` patch, `0.2.0`
      minor breaking).

      ### Doc updates (final step before opening the PR)

      - `.chief/milestone-1/_contract/01-pact-schema.md` §"Test-only
        pact": path becomes `crates/reeve/tests/fixtures/test-fixtures.yaml`.
      - `.chief/_rules/_contract/pact-layout.md`: `include_str!` path
        in the "How to apply" section becomes
        `crates/reeve/src/pact/presets.rs`.
      - `.chief/_rules/_standard/test-artifacts.md`: trim the
        "don't reach across crates into another's `tests/` tree"
        line — dead text in a single-crate workspace.
      - `README.md` Development section: the "three crates" bullet
        list becomes one crate with `core/` and `pact/` modules.

      Historical artifacts (batch reports, retro, grill log, draft
      specs) are NOT rewritten — they are time-stamped records.

      ### Notes

      - We are deliberately NOT committing to a stable library API.
        `pub(crate)` modules mean `cargo add reeve` gives no public
        API; the README states this explicitly.
      - The CLI install path becomes `cargo install reeve` once the
        first `0.1.0` is on the registry.
      - When `reeve-flex` ships in a later milestone, it lands as
        `src/bin/reeve-flex.rs` in this same crate. `cargo install
        reeve` will then install both binaries; users wanting only
        one pass `--bin reeve` or `--bin reeve-flex`.

> Cross-milestone parking lot lives at `.chief/_backlog.md`. Items
> there are not yet planned; promote via `/chief-plan` when picking
> one up.
