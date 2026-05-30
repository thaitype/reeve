# Autopilot Run Batch 2

## Mode
auto

## Summary
Implemented the four low-effort security-review follow-ups (F2, F1, F5+F6, F4
from `_report/security-review.md`) selected by the user. Goal/contract review
was skipped per explicit user instruction. One builder-agent executed the whole
batch (the tasks share files and could not be safely parallelised); chief
reviewed the diff, ran the verification gate, fixed two report artifacts the
builder's scope didn't cover, and updated the audit-log contract to match.

All code and docs are now self-consistent: the `exec_error` audit event and the
`filepath`/`regex` argument kinds — both advertised but never implemented — are
gone from code and from the published docs.

## Tasks Completed
- **task-7 (F2)** — Release-notes no longer advertise `kind: filepath` / `regex`
  validators that don't exist. Kind list corrected to `enum, number, string`;
  the `cat` Q&A example now uses `kind: string`; `filepath` + `allowed_roots`
  labelled as forthcoming in v0.3.0. Docs only.
- **task-8 (F1)** — `audit.capture_command` is now enforced (was a parsed-but-
  ignored, fail-open flag). When `false`, `exec_start` emits `argv: []` (the
  `binary` field is kept). Default `true` → behaviour unchanged. Plumbed through
  `run_exec_audited` → `run_exec_with_env`; both `exec`/`exec_allow_fail` call
  sites pass `ctx.security.audit.capture_command`. Contract `03-audit-log.md`
  updated; new test `capture_command_false_emits_empty_argv_in_exec_start`.
- **task-9 (F5+F6)** — Deleted the dead `ExecError` event variant, the
  `exec_error()` constructor, and the `limit_ms` field (orphaned since the
  timeout/output-cap feature was removed; its only kinds were `Timeout` /
  `OutputLimitExceeded`). Removed the `### exec_error` section from the contract
  and corrected the stale "UUID v4 / `new_v4()`" line to "UUID v7 / `now_v7()`".
  Fixed three stale comments in `engine.rs`/`executor.rs` that claimed timeout +
  cap enforcement. Also corrected two report artifacts that still listed
  `exec_error` (`release-notes.md` audit-event list, `pr-description.md`
  "Six event types" → "Five").
- **task-10 (F4)** — Added a 10 MiB `MAX_PARSE_BYTES` guard in front of
  `parse_json` and `parse_yaml`; over-limit input returns `ParseError` before
  serde runs. Two new tests. (See decision below on why 10 MiB and its limits.)

## Decisions Made (auto mode)

- **F1 — enforce vs. remove `capture_command`.**
  - *Options:* (a) enforce the flag; (b) delete the flag and document capture as
    always-on.
  - *Chosen:* enforce.
  - *Reason:* enforcing gives operators a real lever to keep secret-bearing argv
    out of the audit log, which is the more security-useful direction and a
    smaller change than ripping out a documented, shipped config field. When
    `false`, only `argv` is redacted (to `[]`); `binary` is retained because the
    binary name is already fully known from the pact and is low-sensitivity.

- **F5 — delete `exec_error` despite it being a contracted event type.**
  - *Options:* (a) delete it (user's explicit request); (b) keep the contracted
    event and wire it to the existing unaudited failure paths (spawn failure /
    reader-panic).
  - *Chosen:* delete (option a).
  - *Reason:* the user listed "delete dead ExecError" explicitly, and the
    event's contract was defined *entirely* around `Timeout` / `OutputLimitExceeded`
    kinds that no longer exist, so it had no remaining trigger. Documented
    consequence below under **User Action Needed**.

- **F4 — limit value and scope.**
  - *Options:* small cap (~100 KiB, matches `set_max_string_size`) vs. generous
    cap (10 MiB).
  - *Chosen:* 10 MiB.
  - *Reason:* a 100 KiB cap would reject legitimate large tool output (e.g.
    `kubectl get pods -o json` across many pods). 10 MiB bounds pathological
    large-flat input without breaking realistic use. Explicitly documented (in
    code and the security review) that this does **not** fully prevent YAML
    alias-bomb expansion — a small input can still amplify into a huge tree;
    proper mitigation (alias-limiting) is deferred.

## Verification
- `cargo test --lib`: **89 passed** (was 86; +3 new tests).
- `cargo test --test cli`: **11 passed** (including `h10_h14_audit_log_after_sysinfo`,
  the known parallel-pollution flake — passed this run).
- `cargo check --all-targets`: clean, no warnings.
- `cargo clippy`: **could not run — clippy component is not installed and the
  environment is offline** (`rustup component add clippy` failed). The README
  mandates clippy as a pre-PR gate; it must be run in a connected environment
  before merge. See **User Action Needed**.

## Backlog
Deferred security-review findings (not in this batch, by design):
- **F3** — re-introduce an `exec` output cap + optional `wait_timeout` behind
  `security.yaml` (a re-add of the just-removed code path). Medium effort.
- **F4 (proper)** — YAML alias-bomb mitigation via alias-limiting; serde_yaml
  has no easy stable API for this. Low/medium effort, needs design.
- **F7** — harden the audit fallback path (predictable shared `/tmp` name).
- **F8** — fix the flaky CLI test by isolating `HOME` per spawned process
  instead of scanning the shared `~/.reeve/runs`. Test-quality, medium effort.

## User Action Needed
1. **Run `cargo clippy --all-targets -- -D warnings` in a connected
   environment** before opening the milestone-2 → main PR; it could not be run
   here. `cargo check` is clean, so no warnings are expected, but the gate is
   unverified.
2. **Audit gap acknowledgement (from F5 decision):** with `exec_error` removed,
   `exec` *spawn failures* and *reader-thread panics* now produce an
   `exec_start` with no following `exec_end` (they surface only as the Rhai
   `ExecFailed` error and the eventual `script_end` "error"). If per-exec
   failure events in the audit log are desired, a future milestone should emit
   `exec_end` (or a re-introduced, wired error event) on those paths. The
   forward-looking `draft/spec-v3.md` still describes an `exec_error` event with
   a `Timeout` kind — appropriate to revisit if/when F3 re-adds timeouts.
