# Milestone 1 — Scope

Smallest useful slice that proves the central thesis of `draft/spec-v2.md`:
**Rhai script + pact allowlist + sandboxed `exec()`** working end-to-end with
zero external infrastructure.

## In scope

- Three crates: `reeve-core`, `reeve-pact`, `reeve` (CLI).
- Rhai engine with all spec-v2 resource limits and `eval`/module disabling.
- `exec` / `exec_allow_fail` host fns enforcing per-exec timeout + output cap.
- `parse_json`, `parse_yaml`, `script_args`, `print`, `log_info/warn/error`
  host fns. **No** filesystem host fns.
- Pure-allowlist pact engine with three named kinds: `enum`, `number`,
  `string` (string rejects shell metacharacters + null bytes).
- One embedded production preset: `linux-readonly` covering `echo`, `date`,
  `uname`, `whoami`, `hostname`.
- One test-only pact (`pacts/test-fixtures.yaml`, `#[cfg(test)]`) covering
  `sleep` and `yes` for timeout / output-cap tests.
- CLI: `reeve run <script>` and `reeve version`. No flags.
- Single-line stderr trace per `exec` via `executor::trace!` macro
  (audit JSONL hooks here in a later milestone).
- Bypass-resistance test suite (subset; see `_contract/02-test-matrix.md`).
- Measurement: binary size and cold-start time recorded.

## Out of scope (deferred per `draft/increment-1.md`)

`reeve-flex`, `security.yaml`, `runtime.yaml`, Layer 1 FS host fns,
`.reeve/<run-id>/` workspace, `stdout_to`, JSONL audit, script-total
timeout, `env()` host fn, `env_overrides`, custom validator escape hatch,
additional named kinds, `k8s-readonly` / `git-readonly` presets, `check` /
`preset list` / `preset show` / `init` subcommands, `to_json`, `parse_toml`,
`exec_parallel`, all CLI flags.

## Non-goals (dropped — re-add only if needed)

- Risk-category vocabulary (PureRead/Standard/RequiresAudit/Forbidden).
- Distribution polish (Homebrew, Docker, pre-built binaries).

## Done when

- All bypass-resistance tests in `_contract/02-test-matrix.md` pass.
- `reeve run examples/sysinfo.rhai` prints whoami / hostname / uname /
  date output on Linux **and** macOS dev boxes (zero infra setup).
- Release binary `reeve` < 10 MB.
- Cold start of trivial script < 50 ms.
- README has 5-line "what is this" + 10-line "try it" section runnable
  with only Rust + this repo.
