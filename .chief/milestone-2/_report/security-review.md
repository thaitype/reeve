# Security Review — Milestone 2 (v0.2.0 candidate)

**Date:** 2026-05-30
**Scope:** Full read of the security-relevant surface in `src/` against the
**AI-agent-as-caller** threat model.
**Reviewer:** code audit (read-only; no behaviour changed by this review).

---

## Threat model

The adversary is the **author of the Rhai script** — an AI agent that Reeve
runs on the operator's behalf. The agent is assumed *capable and curious*: it
will try to read host secrets, escape the workspace, run binaries the operator
never allowed, exfiltrate data through allowed binaries, or wedge/OOM the host.

What the agent controls:
- the full Rhai script source (any host-fn calls, any arguments),
- the *content* of arguments passed to `exec()` and the FS host fns,
- the *content* fed to `parse_json` / `parse_yaml`.

What the agent must **not** be able to do:
- run a binary not in the embedded pact, or pass an undeclared flag,
- swap or weaken the policy (pact / `security.yaml`),
- read host environment secrets not in `env_passthrough`,
- read or write files outside `<reeve_home>/workspace/`,
- crash Reeve in a way that *skips the audit trail*.

What is explicitly **out of scope** (same as the release-notes security model):
OS-level isolation, audit-log integrity against a same-UID process, pact
*correctness*, and secret exfil through a deliberately permissive pact.

---

## What holds up well

These are the load-bearing controls; the review found no bypass in any of them.

- **No shell anywhere.** `exec` goes through `std::process::Command` with an
  argv array and `stdin(null)` (`executor.rs:119-123`). There is no `sh -c`,
  so classic shell-injection via argument content is structurally impossible.
  The metacharacter filter in `kinds.rs` is defence-in-depth, not the primary
  barrier.
- **Allowlist is fail-closed.** Unknown binary → `BinaryNotAllowed`; unknown
  flag → `FlagNotAllowed`; extra positional → `PositionalRejected`
  (`pact/engine.rs`). A positional that starts with `-` is treated as a flag
  and rejected — inconvenient but safe.
- **Env isolation is real.** Child env is `env_clear()`-ed and only
  `env_passthrough` keys are re-added (`executor.rs:126-133`); verified by the
  SF-2 / SF-3 tests with a planted secret.
- **Workspace sandbox resists symlink escape.** `read_file` re-canonicalizes
  the resolved path and re-checks the prefix (`fs.rs:130-136`); `write_file`
  refuses any symlink target, dead or alive (`fs.rs:199-206`); `exists`
  resolves symlinks before answering (`fs.rs:173-186`). Absolute paths and
  `..` components are rejected by path *component* (not substring) check.
- **Policy is immutable at runtime.** Both the pact and `security.yaml` are
  `include_str!`-embedded at compile time; `REEVE_HOME` is ignored (B8 test).
  The agent cannot point Reeve at a weaker policy.
- **Rhai engine is hardened.** `eval` disabled, modules disabled
  (`set_max_modules(0)`), and operation / call-depth / string / array caps set
  (`engine.rs:62-67`).
- **Audit cannot be skipped by a type error.** Non-string `exec` args now
  return a catchable `TypeError` instead of panicking (`engine.rs:94-110`), and
  reader-thread panics propagate as `ExecFailed` (`executor.rs:172-179`).

---

## Findings — rated by impact × effort

Impact = damage under the threat model above. Effort = work to remediate.

| # | Finding | Impact | Effort |
|---|---------|--------|--------|
| F1 | `audit.capture_command` flag is parsed but never enforced — `exec_start` always logs full `argv` | Med | Low |
| F2 | Docs advertise `kind: filepath` / `regex` validators that do **not** exist in code | Med | Low |
| F3 | No `exec` timeout or output cap → agent can hang the run or exhaust memory | Med | Med |
| F4 | `parse_yaml` / `parse_json` parse agent input natively, outside Rhai's op budget (alias-bomb / deep-nest DoS) | Low | Low |
| F5 | Dead `ExecError` event + `exec_error()` + `limit_ms` field — never emitted since timeout removal | Low | Low |
| F6 | Stale comments still claim "enforces timeout + cap" (`engine.rs:87`, `executor.rs:35`,`156-157`) | Low | Low |
| F7 | Audit fallback writes to a predictable shared path (`$TMPDIR/reeve-audit-fallback`) | Low | Low |
| F8 | Flaky CLI test `h10_h14_audit_log_after_sysinfo` — shared global `~/.reeve/runs` + parallel tests | Low | Med |

### F1 — `capture_command` is a no-op (fail-open)

`SecurityConfig.audit.capture_command` is loaded (`security.rs:34`) but no code
reads it. `executor.rs:113` always builds `exec_start` with the full `argv`. An
operator who sets `capture_command: false` — e.g. precisely because a script
passes a token-shaped value as an argument — still gets that value written to
`audit.jsonl`. The release notes say "Command capture is on by default,"
implying it is toggleable; it is not. Direction of failure is *toward*
disclosure. **Fix:** honour the flag (omit/redact `argv` when false), or drop
the flag and document capture as always-on.

### F2 — phantom `filepath` / `regex` kinds

`KindSpec` implements only `enum`, `number`, `string` (`schema.rs:101-106`,
`kinds.rs`). The release notes claim arguments are "matched against the pact's
declared kinds (`enum`, `number`, `filepath`, `string`, regex)" and the Q&A
tells operators to give `cat` a `kind: filepath` argument. With
`deny_unknown_fields`, a pact using `kind: filepath` **fails to parse**
(exit 3). Worse, the wording implies path-confinement validation happens on
exec arguments today — it does not (that is Layer 2 / `allowed_roots`, deferred
to v0.3.0). An operator could ship a pact believing paths are fenced when they
are not. **Fix:** correct the release-notes wording to list only the kinds that
exist and label `filepath` as forthcoming. (README's "wanted contributions"
list is already correct — it lists `filepath` as *not yet built*.)

### F3 — no resource bound on `exec` (accepted, document as deferred)

The per-exec timeout and output cap were removed in `29b1129`; `exec` now
`wait()`s indefinitely and reads stdout/stderr unbounded into memory
(`executor.rs:162-190`). Under this threat model the agent can therefore hang
the run (a non-terminating allowed binary) or drive the host to OOM (a chatty
one). This is currently bounded only by an *external* supervisor. It is already
documented under "Resource exhaustion" in the release-notes security model.
**Recommendation:** acceptable for v0.2.0; re-introduce a streaming output cap +
optional `wait_timeout` behind `security.yaml` in a later milestone — note this
is exactly the code path just deleted, so it is a *re-add*, not new design.

### F4 — native parse DoS bypasses the op budget

`set_max_operations(1_000_000)` only governs Rhai bytecode. `parse_yaml`
(`parse.rs:20-25`) hands agent-controlled input straight to `serde_yaml`, which
honours YAML anchors/aliases — a "billion laughs" document expands in native
code the op counter never sees. Impact is one-process memory/CPU, but it is a
clean way around the engine's headline DoS guard. **Fix (cheap):** reject input
over a sane byte length before parsing (the engine already caps strings at
100 KiB via `set_max_string_size`, so a small explicit guard here is
consistent).

### F5 / F6 — dead timeout scaffolding and stale comments

`AuditEvent::ExecError { … limit_ms }` and `exec_error()` are no longer emitted
anywhere (`audit.rs:48-55,114`); `limit_ms` exists only to report a timeout
that can no longer occur. Comments at `engine.rs:87`, `executor.rs:1-5,35,
156-157` still describe timeout/cap enforcement. Both mislead a future
maintainer about what protections are live. **Fix:** delete the dead event +
constructor + field, and correct the comments.

### F7 — predictable audit fallback path

When the real run dir can't be opened, `main()` falls back to a fixed
`std::env::temp_dir().join("reeve-audit-fallback")` then `…-noop`
(`bin/reeve.rs:153-165`). In a shared `/tmp`, the predictable name lets another
local user pre-create or symlink it. Audit-log integrity against a same-UID
process is already declared out of scope, so impact is low, but the fallback is
easy to harden (per-run subdir / `O_EXCL`) or to drop in favour of failing
loudly. **Fix:** low priority.

### F8 — flaky audit CLI test (test quality, not a vuln)

`tests/cli.rs::h10_h14_audit_log_after_sysinfo` runs `sysinfo`, then scans the
**shared** `~/.reeve/runs` newest-first for any dir with a `whoami` exec_start
and asserts its last line is `script_end`. Because `cargo test` runs CLI tests
in parallel against the same real `$HOME`, the scan can latch onto another
test's *in-flight* run dir (last line still `exec_start`) and fail. It passes
in isolation (`--exact`). Not security-relevant and not caused by the timeout
removal, but it makes `cargo test` non-deterministic. **Fix:** isolate the
spawned process with a temp `HOME`, or assert against the specific run the test
created rather than a global scan.

### Informational (no action)

- **No `--` end-of-options** in argv parsing — a positional beginning with `-`
  can't be passed, but this fails *closed*. Fine.
- **Symlink TOCTOU** between `validate_path` and the FS write is only
  exploitable by a concurrent same-UID process; the Rhai script is
  single-threaded and cannot race itself. Out of threat model.

---

## Recommended for THIS milestone (all low-effort, docs + dead-code)

1. **F2 — docs:** fix the release-notes kind list and Q&A so `filepath`/`regex`
   are described as not-yet-implemented (aligns docs with `kinds.rs`).
2. **F1 — `capture_command`:** either enforce it in `exec_start` or remove the
   flag and state capture is always-on. (Smallest correct change: enforce.)
3. **F5 + F6 — dead code & comments:** remove the unused `ExecError` /
   `exec_error` / `limit_ms` and fix the stale "timeout + cap" comments.
4. **F4 — parse guard (optional):** add a byte-length check before
   `serde_yaml` / `serde_json` parse.

Defer F3 (re-add resource caps) and F7 (fallback hardening) to a later
milestone; fold F8 into general test-isolation cleanup.
