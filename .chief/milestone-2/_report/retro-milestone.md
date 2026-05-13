# Retro: Milestone 2 — Milestone

Date: 2026-05-13
Commits: `aed8b4a` → `da81711` (9 commits)
Final test count: 97 (86 lib + 11 CLI)
Binary: 5.0 MB / Cold start: 6 ms

---

## Coverage Check

### Goals

| File | Status | Notes |
|------|--------|-------|
| `_goal/01-scope.md` — security.yaml embedded | ✅ Satisfied | `src/security.rs` + `include_str!` in place |
| `_goal/01-scope.md` — $HOME/.reeve/ lazy init | ✅ Satisfied | `init_home()` in `src/core/home.rs`; idempotent |
| `_goal/01-scope.md` — Layer 1 FS (5 fns) | ✅ Satisfied | `read_file`, `read_lines`, `exists`, `write_file`, `append_file` in `src/core/fs.rs` |
| `_goal/01-scope.md` — JSONL audit (6 events + ts) | ✅ Satisfied | All 6 events, RFC 3339 ts, flush-per-event |
| `_goal/01-scope.md` — `env()` host fn | ✅ Satisfied | EnvDenied / EnvUnset per contract |
| `_goal/01-scope.md` — `to_json()` host fn | ✅ Satisfied | serde_json serialisation |
| `_goal/01-scope.md` — exec env from SecurityConfig | ✅ Satisfied | Wired after post-delivery fix; `env_clear()` + selective re-add |
| `_goal/01-scope.md` — binary < 10 MB / cold < 50 ms | ✅ Satisfied | 5 MB / 6 ms |
| `_goal/02-decisions.md` — RunContext via Arc | ✅ Satisfied | `Arc<RunContext>` in all closures |
| `_goal/02-decisions.md` — flush-per-event audit | ✅ Satisfied | `flush()` after each `writeln!` |
| `_goal/02-decisions.md` — no script_sha256 | ✅ Satisfied | Field omitted |
| `_goal/02-decisions.md` — script_path canonicalized | ✅ Satisfied | `fs::canonicalize` before `script_start` |
| `_goal/02-decisions.md` — no sentinel | ✅ Satisfied | No `.reeve-managed` this milestone |
| `_goal/02-decisions.md` — log_* emit audit events | ✅ Satisfied | `script_log` event on every log call |
| `_goal/02-decisions.md` — env() throws, no probe | ✅ Satisfied | EnvDenied / EnvUnset, no default |

### Contracts

| File | Status | Notes |
|------|--------|-------|
| `_contract/01-security-config.md` | ✅ Satisfied | SecurityConfig, AuditConfig, RunContext, init_home all match spec |
| `_contract/02-host-fns.md` — FS fns + error kinds | ✅ Satisfied | PathDenied/FileNotFound/FileAlreadyExists/IoError all correct shapes |
| `_contract/02-host-fns.md` — env() / to_json() | ✅ Satisfied | Correct error kinds and payloads |
| `_contract/02-host-fns.md` — build_engine_with_args signature | ✅ Satisfied | Now accepts `Arc<RunContext>` |
| `_contract/02-host-fns.md` — exec env from SecurityConfig | ⚠️ Partial | Initial delivery missed child-env filtering; fixed post-review via TDD cycles |
| `_contract/03-audit-log.md` — AuditWriter + 6 events | ✅ Satisfied | All events match schema |
| `_contract/03-audit-log.md` — flush-per-event | ✅ Satisfied | Verified by unit test |
| `_contract/03-audit-log.md` — write failure → warn, continue | ✅ Satisfied | `try_emit` pattern |
| `_contract/04-test-matrix.md` — B1–B5 bypass resistance | ✅ Satisfied | All pass |
| `_contract/04-test-matrix.md` — B6–B7 env bypass | ✅ Satisfied | EnvDenied / EnvUnset tests pass |
| `_contract/04-test-matrix.md` — B8 REEVE_HOME ignored | ✅ Satisfied | CLI integration test |
| `_contract/04-test-matrix.md` — H1–H5 FS happy path | ✅ Satisfied | All pass |
| `_contract/04-test-matrix.md` — H6–H9 env/json happy path | ✅ Satisfied | All pass |
| `_contract/04-test-matrix.md` — H10–H14 audit content | ✅ Satisfied | All pass |
| `_contract/04-test-matrix.md` — H12 script_log event | ✅ Satisfied | Added post-review |
| `_contract/04-test-matrix.md` — H15 invalid yaml fails build | ❌ Missing | Not implemented; runtime exit-3 only |
| `_contract/04-test-matrix.md` — N1 workspace-demo | ✅ Satisfied | `examples/workspace-demo.rhai` + CLI test |
| `_contract/04-test-matrix.md` — R1 sysinfo regression | ✅ Satisfied | Passes |

---

## Planned vs Delivered

### Planned (5 tasks)

All 5 tasks completed as specified. No tasks skipped or rolled back.

### Unplanned work (added after delivery)

Three rounds of post-delivery work were required before the milestone was sound:

1. **Code review** — identified 2 critical/high security issues (SF-1 symlink escape, SF-2 env leak), 3 contract violations, 9 test coverage gaps, 8 technical debt items.
2. **TDD fix cycle (7 issues)** — symlink escape fix, trace! gate, read_lines cleanup, duplicate kind cleanup, env/audit tests wired. env_passthrough mechanism proven in test helper but NOT yet in production path.
3. **Production env-passthrough wiring + 6 more tests** — SF-2 fully closed; symlink exists() fix; dead-symlink write_file test; subdir creation tests; exec_allow_fail audit test.

Net result: 97 tests vs 86 after autopilot batch. Milestone is sound.

### What changed mid-execution

- `build_engine_with_args` signature update (task-4 scope) was absorbed by task-2's builder-agent early. Led to no duplication but required careful tracking.
- H15 (invalid YAML → build failure) was scoped in the contract but not implemented — neither the autopilot nor the review fix cycles addressed it. Remains open.

---

## Blockers Hit

1. **SF-1 (symlink escape)** — the initial `validate_path` only canonicalized the parent directory, not the full candidate path. Symlinks inside workspace pointing outside were silently followed. Discovered in post-delivery review; fixed via TDD.

2. **SF-2 (child env leak)** — `env_passthrough` was wired to `env()` host fn (script reads) but not to `Command` spawning. Found in review; the TDD cycle proved the mechanism in a test helper but left the production path (`run_exec_audited`) still passing `None`. Required a second fix round to fully close.

3. **task-2 / task-4 scope bleed** — builder for task-2 implemented engine signature changes and `log_*` audit wiring that belonged to task-4. No harm done (task-4 became lighter), but the deviation was undetected until task-4 ran.

---

## Lessons Learned

### What worked well

- **`RunContext` via Arc** — threading shared state into all closures without globals worked cleanly. Tests are fully isolated; no shared-state test interference.
- **Flush-per-event audit** — the decision paid off: partial runs (crash or timeout) still produce readable audit logs. No compromises needed.
- **Grill-first planning** — every architectural decision made in the grill had a clear rationale that held up through implementation. No design reversals.
- **TDD for security fixes** — writing the failing test first caught that the symlink escape wasn't fixed by just the parent-dir canonicalize. The red→green discipline prevented half-fixes.

### What went wrong

- **Security tests not required upfront** — the bypass-resistance test for symlinks was in the design grill (0001-reeve-design.md) but never made it into the contract test matrix. The autopilot shipped without it. A code review caught it — but that's an expensive way to discover a missing security test.

- **Production vs test path parity** — SF-2 was "fixed" in the first TDD round by proving the mechanism in `run_exec_with_passthrough` (test helper) but the actual production path `run_exec_audited` still passed `None`. This is a recurring risk pattern: fix visible in tests, invisible in production.

- **H15 (compile-time YAML validation) never implemented** — it was in the contract but silently skipped by both autopilot and review fix rounds. No one was explicitly accountable for it.

- **Post-delivery review required three separate rounds** — review → TDD fixes → second round of fixes. A single thorough pass would have been more efficient.

---

## Proposed Rule Updates

### Proposal 1 — Security bypass tests belong in contracts, not grill logs

**What:** Any security bypass-resistance test identified during a grill session MUST be copied verbatim into the milestone `_contract/04-test-matrix.md` (or equivalent test matrix) before planning ends. Grill logs are not reliably consulted during implementation.

**Where:** `.chief/_rules/_verification/security-tests.md` (new file)

**Why:** The symlink escape test was explicitly called out in the grill session (`0001-reeve-design.md`) but was never promoted to the test matrix contract. The autopilot implemented what was in the contract; the grill log was not checked.

**Suggestion:** Adopt.

---

### Proposal 2 — Security mechanisms must be proven in the production path, not only in test helpers

**What:** When a security fix (e.g. env filtering, path validation) is implemented, the verification test MUST exercise the production code path — not a test-only wrapper. A test that calls `run_exec_with_passthrough` does not verify that `run_exec_audited` filters env. The test must run through the same call site that production scripts use.

**Where:** `.chief/_rules/_verification/security-tests.md` (same file as Proposal 1)

**Why:** SF-2 was "fixed" in the first TDD round but the production path `run_exec_audited` still passed `None`. Only the second fix round (prompted by user) closed it. The test gave false confidence.

**Suggestion:** Adopt.

---

### Proposal 3 — Builder scope bleed must be flagged in the batch report

**What:** When a builder-agent implements items from a future task's scope, it MUST note this explicitly in its completion summary. Chief-agent must mark those items as done in the relevant task before delegating that task, to avoid redundant work or missed wiring.

**Where:** `.chief/_rules/_standard/builder-scope.md` (new file)

**Why:** task-2's builder implemented `build_engine_with_args` signature change and `log_*` audit wiring (both task-4 scope). Chief-agent noticed but didn't update task-4's spec. task-4 ran lighter than expected — benign this time, but the pattern can mask missed wiring in more complex cases.

**Suggestion:** Adopt.

---

### Proposal 4 — Contract test matrix items have explicit owners

**What:** Each item in `_contract/XX-test-matrix.md` must be traceable to a specific task in `_plan/_todo.md`. Items with no owner task are at risk of being silently skipped.

**Where:** `.chief/_rules/_verification/test-matrix.md` (new file)

**Why:** H15 (invalid YAML → build failure) was in the contract test matrix but owned by no task. Neither autopilot nor fix cycles addressed it. It remains unimplemented.

**Suggestion:** Adopt — but keep lightweight. Add a single line in `_todo.md` tasks referencing which test-matrix rows they cover; the planner checks coverage before autopilot runs.

---

## User Action Needed

1. **H15 still open** — "Invalid `security.yaml` → `cargo build` fails" is in the contract but unimplemented. Decide: add to milestone-3 backlog, or fix now with a small build-script or const-eval test?

2. **Rule proposals** — see above. Which proposals should be applied to `.chief/_rules/`?

3. **Push branch** — milestone-2 is 9 commits ahead of `origin/milestone-2`. Ready to push when you are.
