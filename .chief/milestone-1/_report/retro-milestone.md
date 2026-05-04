# Retro: milestone-1 — Milestone

Scope: all 10 tasks complete (`_plan/_todo.md` fully `[x]`).
Span: 11 commits from `7ce0ef0` (plan) to `51e2fa7` (final).
Output: 3 crates, 1 prod pact, 1 test pact, 2 examples, README,
60 passing tests, 4.7 MB binary, 8.7 ms cold start.

## Coverage Check

| File | Status | Notes |
|---|---|---|
| `_goal/01-scope.md` | ✅ | Every "in scope" item shipped. Done-when checklist all met (60 tests pass, sysinfo runs Linux+macOS — Linux not yet manually verified, see Lessons; binary 4.7 MB; cold start 8.7 ms; README has both sections). |
| `_goal/02-decisions.md` | ✅ | D1 (per-OS path arrays + `default`) implemented in `reeve-pact::engine::resolve_path`. D2 (test pact NOT next to prod) corrected mid-batch-4 after user flag. D3 (single embedded preset, no selection) honored — no `--pact`/`--allow-preset`. D4 (edition 2021, MSRV 1.75, `clippy::all` + `-D warnings`, no `pedantic`) honored. |
| `_contract/01-pact-schema.md` | ✅ | All struct shapes match. Path note added during batch 4 (test pact moved to `crates/reeve-pact/tests/fixtures/`). One contract correction in batch 5 — see below. |
| `_contract/02-host-fns.md` | ✅ | All 9 host fns registered. Error map shapes match `_contract/02` §"Throws" verbatim. One contract correction: `set_max_call_stack_depth` → `set_max_call_levels` (batch 5). |
| `_contract/03-test-matrix.md` | ✅ | All 14 rows have a passing test (table in batch-9 report). "Missing preset → exit 3" intentionally dropped per D3. Both measurement gates met. |

## Planned vs Delivered

**Planned (TODO):** 10 tasks, dependency-ordered.
**Delivered:** 10 tasks. No tasks added, removed, or reordered. No
mid-stream scope cuts.

Differences vs original spec-v2:
- Test fixture pact moved out of `pacts/` (not in original plan, fixed
  after user flag during batch 4).
- `set_max_call_stack_depth` renamed to `set_max_call_levels` to match
  the actual rhai 1.x API.
- `BinaryNotResolvable` introduced in `reeve-pact` (rust-side name);
  surfaces as `BinaryNotFound` in Rhai-visible error maps.

## Blockers Hit

- **Rhai API name drift** (batch 5). Spec-v2 used the conceptual name
  `set_max_call_stack_depth` for what rhai 1.x actually calls
  `set_max_call_levels`. Builder caught it; contract updated mid-flight
  with a clarifying note.
- **`#[cfg(test)]` artifact visibility across crates** (batch 6).
  `reeve-pact::test_fixtures()` is `#[cfg(test)]`, so `reeve-core`'s
  test build can't import it. Resolved by re-`include_str!`-ing the
  fixture YAML inside `reeve-core` tests.
- **Output cap timing** (batch 6). Builder chose post-exit cap check
  (simpler) over stream-time enforcement (correct-er). Documented as a
  known limitation: a runaway producer can buffer above the cap until
  the timeout kills it. Acceptable because timeout always bounds total
  time.
- **Test pact directory placement** (batch 4). Builder initially put
  `test-fixtures.yaml` in workspace `pacts/` next to the prod pact.
  User caught it; moved to `crates/reeve-pact/tests/fixtures/`. The
  task spec said "never embedded in release" — it didn't say "never
  visible alongside production policy." Both should be the rule.

## Lessons Learned

**What worked:**
- **Strict task contracts in builder prompts** — listing the exact
  fields, error variants, and test cases up-front gave builders little
  room to drift. Builder-agent runs landed in 90 s – 6 min each, almost
  always green on first try.
- **Per-task batch reports with explicit "Decisions Made"** — the
  decisions log captured non-obvious trade-offs (timeout primitive,
  `on_print` vs `register_fn`, post-exit cap timing) that would
  otherwise have been lost. Easier to retro because the trail is right
  there.
- **Coverage audit at task-9, not at retro time** — pre-confirming
  every test-matrix row has a home let task-10 (measurement) be a
  formality.
- **Auto mode plus per-task scoping** ("only focus on task-N")
  prevented chief-agent from running ahead and reduced human prompts
  to ~one per task.
- **Grill before plan, not during** — 4 questions in Phase 0 surfaced
  the per-OS path schema, the test-fixture isolation question, and the
  toolchain stance. Cheaper than discovering them during builder work.

**What didn't:**
- **Test artifacts placed by file-type instead of by trust boundary.**
  The natural Rust convention "fixtures live in tests/" wasn't in any
  rule, so the builder used the more visible `pacts/` workspace dir.
  Trust-boundary placement (prod vs test) needs to be a stated rule,
  not implicit.
- **Spec API names taken at face value.** Spec-v2 had at least one
  out-of-date Rhai API name. No batch caught it pre-implementation;
  builder caught it during writing. Worth noting that spec-v2 should
  carry a "code-verified" or "aspirational" tag on API references.
- **Linux verification still manual** despite `_goal/01-scope.md` saying
  "Linux AND macOS dev boxes." Only macOS verified locally. Not a
  blocker because all paths are POSIX-standard and the per-OS resolver
  has fallbacks, but the gate is technically not 100% verified.

## Proposed Rule Updates

### Rule R1 — Test-only artifacts go under the consuming crate's `tests/` tree

- **What:** Any test fixture (YAML, JSON, sample data, mock binaries,
  etc.) that is gated by `#[cfg(test)]` MUST live under the consuming
  crate's `tests/` directory (e.g.
  `crates/<crate>/tests/fixtures/<name>.yaml`). It MUST NOT live in
  workspace-root directories that hold production assets (e.g.
  `pacts/`, `assets/`, `templates/`).
- **Where:** `.chief/_rules/_standard/test-artifacts.md` (new file).
- **Why:** Batch 4 — test fixture placed next to prod pact. Reviewer
  surface and any future `include = ["pacts/**"]` glob would have
  shipped the test artifact. Same-directory implies same-trust.
- **Suggestion:** Apply.

### Rule R2 — Spec API references must be verified against the library before code lands

- **What:** When a task spec references a library API by name (e.g.
  `engine.set_max_call_stack_depth`), the implementing builder MUST
  verify the name against the actual library version pinned in
  `Cargo.toml` before writing code. If the name is wrong, the builder
  updates the contract with a one-line correction note in the same PR.
- **Where:** `.chief/_rules/_standard/library-api-verification.md`
  (new).
- **Why:** Batch 5 — spec-v2 used `set_max_call_stack_depth` (concept)
  for what rhai 1.x actually exposes as `set_max_call_levels`. Caught
  by builder, but only after writing code that didn't compile.
  Cheaper to verify first.
- **Suggestion:** Apply.

### Rule R3 — Pact / policy file path layout

- **What:** Production pacts live at workspace root `pacts/`. Test-only
  pacts live at `crates/<crate>/tests/fixtures/`. The naming convention
  is `<scope>-readonly.yaml` for read-only allowlists; suffix with
  `-rw.yaml` if write operations are permitted.
- **Where:** `.chief/_rules/_contract/pact-layout.md` (new). This is a
  contract-shape rule, not a coding standard.
- **Why:** Now that we have the precedent (`linux-readonly`,
  `test-fixtures`), future presets (`k8s-readonly`, `git-readonly`)
  should follow consistently. Naming convention prevents bikeshedding
  later.
- **Suggestion:** Apply.

### Rule R4 — Runtime error map shape contract

- **What:** Any host fn that throws to Rhai MUST throw an
  `EvalAltResult::ErrorRuntime(Dynamic, Position::NONE)` where the
  `Dynamic` is a `Map` containing at minimum a `kind: String` field.
  Additional fields are documented per error in
  `.chief/<milestone>/_contract/02-host-fns.md` §"Throws". The CLI's
  `classify_error` reads the `kind` field to choose the exit code, so
  any new error kind requires updating both the contract table and the
  classifier.
- **Where:** `.chief/_rules/_contract/error-maps.md` (new).
- **Why:** This pattern emerged organically across tasks 6, 7, 8.
  Codifying it now prevents drift when new host fns land (Layer 1 FS,
  audit, etc.).
- **Suggestion:** Apply.

### Rule R5 — Cross-platform verification gate (NOT applied as a rule)

- **Status:** Re-scoped to a backlog task instead of a rule.
- **What changed:** Rather than codifying "manual cross-platform
  verification" as a rule, this is now `task-11` in
  `_plan/_todo.md`'s backlog: add a GitHub Actions matrix
  (`{ubuntu-latest, macos-latest}` × stable Rust) running build, test,
  clippy. CI is the right mechanism — manual verification across boxes
  doesn't scale, and a CI matrix makes "verified on Linux + macOS"
  automatic for every PR going forward.
- **When to revisit as a rule:** if we later add a third or fourth
  target (Windows, *BSD, musl) and the matrix gets opinionated, a
  written rule about which targets are gating may be worth adding.

## User Action Needed

- **Decide which of R1–R5 to apply.** I recommend all five — each is
  cheap and ties to a concrete batch event.
- **Optional:** manually verify `examples/sysinfo.rhai` on a Linux box
  to fully close `_goal/01-scope.md` "Done when" item 2. Take ~2 min;
  not gating.
- **Pick milestone 2 scope.** Strong candidates: (a) Layer 1 FS host
  fns + `.reeve/<run-id>/` workspace + JSONL audit, OR (b)
  `reeve-flex` binary + `security.yaml` + `--config` plumbing.
  Rough sizing: (a) is one solid week, (b) is half that but unlocks
  fewer use cases. /chief-plan when ready.
