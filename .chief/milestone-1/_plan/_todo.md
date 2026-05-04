# Milestone 1 — TODO

Tasks are sized for builder-agent. Order is dependency-driven; later tasks
assume earlier ones landed. Do not reorder without updating dependencies.

- [x] **task-1** — Scaffold Cargo workspace (`warden-core`, `warden-pact`,
      `warden`), pin edition 2021 + MSRV 1.75, set workspace lints
      (`clippy::all`, `-D warnings`), add baseline deps from
      `draft/spec-v2.md` §Dependencies, ensure `cargo build` + `cargo test`
      pass on an empty workspace.
- [x] **task-2** — `warden-pact`: schema structs (`Pact`, `BinarySpec`,
      `KindSpec`, `PositionalSpec`) per `_contract/01-pact-schema.md`,
      including per-OS `path` resolution and absolute-path validation at
      parse time. Deny unknown YAML fields. Unit tests for parse + reject.
- [x] **task-3** — `warden-pact`: kind validators for `enum`, `number`,
      `string` (with shell-metacharacter blocklist from `_contract/01`).
      Generic allowlist engine that takes a parsed `Pact` + `(binary, argv)`
      and returns `Ok(ResolvedExec { abs_path, argv })` or a typed
      `PactError`. Unit tests for every error variant in `_contract/02`.
- [x] **task-4** — Embed `pacts/linux-readonly.yaml` via `include_str!` into
      `warden-pact` and parse-validate at compile time (compile fails if
      YAML is malformed). Add `pacts/test-fixtures.yaml` behind
      `#[cfg(test)]`. Provide `linux_readonly()` and (test-only)
      `test_fixtures()` constructor fns.
- [x] **task-5** — `warden-core`: Rhai engine setup with the resource limits
      and disabled symbols listed in `_contract/02-host-fns.md`. Register
      stub host fns (return `()` for now). Unit tests covering the engine
      sandbox rows of `_contract/03` (#7–#9).
- [x] **task-6** — `warden-core`: implement `exec` + `exec_allow_fail` per
      `_contract/02`. Use `std::process::Command`, `argv` array form, no
      shell. Per-exec timeout via thread + kill on expiry (or
      `wait_timeout` crate if simpler). Output cap via byte-counted stdout
      + stderr buffers, kill on overflow. Map `PactError` and runtime
      errors to the typed Rhai error maps in `_contract/02`. Add
      `executor::trace!` macro (single-line stderr per call).
- [x] **task-7** — `warden-core`: implement `parse_json`, `parse_yaml`,
      `script_args`, `print`, `log_info/warn/error`. Wire `script_args`
      from the CLI through the engine scope.
- [x] **task-8** — `warden` CLI (clap, derive): subcommands `run <script>`
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

## Backlog (post-milestone followups)

- [ ] **task-11** — GitHub Actions cross-platform CI. Add
      `.github/workflows/ci.yml` running `cargo build`, `cargo test
      --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      on a matrix of `{ubuntu-latest, macos-latest}` × stable Rust.
      Cache `~/.cargo` and `target/` per-OS. Fail-fast off so both
      platforms always report. Replaces the manual cross-platform
      verification deferred during milestone 1; closes the gap between
      `_goal/01-scope.md`'s "Linux AND macOS" claim and what was
      actually verified locally (macOS only). Source of this followup:
      retro proposal R5.
