# Grill: Warden Design

## Open Questions

## Resolved

Q1: Primary user for v0.1 → **all four use cases co-equal** (CI/CD, runbooks, pipelines, AI agents). Why: user prefers generic framing; no single use case drives trade-offs.

Q2: Custom pact trust boundary → **operator-only (Option A)**. Why: `--pact <file>` removed at runtime; pacts compile-time embedded only. Matches threat model row "Pact tampering at runtime — embedded at compile time." SREs iterating on pacts must use a dev build / recompile.

Q3 (revised): Pact schema → **YAML-driven generic allowlist engine in Rust**. Why: one engine, many YAML pacts; reviewer reads YAML; contribution = YAML PR (lower bar than Rust). Constraints:
- **Pure allowlist only** — no `forbidden_*` keys; every flag/subcommand/positional must be explicitly allowed. Fail-closed.
- **Built-in named kinds** — `k8s_name`, `k8s_namespace`, `filepath`, `number`, `enum`, etc. implemented in Rust once, reused across pacts.
- **Regex supported** — user can put regex in YAML; ReDoS or bad regex is the pact author's responsibility.
- **Custom validator escape hatch** — `{ kind: custom, name: kubectl_output_flag }` dispatches to a named Rust fn for DSL-shaped values the schema can't express.
- **Compile-time embed preserved** — pacts still `include_str!`'d (Q2 holds). "YAML-driven" reduces contributor effort, not recompile requirement.

Risk mitigations: engine bugs are systemic → fuzz tests + strict schema (`deny_unknown_fields`); pact-author misjudgment → ship reviewed presets + document binary risk categories (PureRead / Standard / RequiresAudit / Forbidden).

Initial Q3 decision (hand-rolled Rust validators per binary) is superseded.

Q4: Script language → **(A) Rhai**. Why: pure Rust (no C dep — better for static cross-compile + binary-size goal), sandbox-by-default (no stdlib to strip), first-class resource limits API (`set_max_operations`, `set_max_call_levels`, `set_max_string_size`), `disable_symbol()` for surgical hardening, type-safe Rust↔script FFI. Trade-offs accepted: smaller LLM corpus than Lua/Python/JS; less engineer familiarity than Lua. Mitigation: ship SKILL.md + host-fn docs (custom host fns matter more than base-language corpus anyway). Starlark noted as viable alternative if "deterministic review-and-replay" becomes a goal later.

Spec gap: add a "Why Rhai" section with the trade-off table; current spec assumes the choice without justifying it.

Q5 (revised): Filesystem access → **built-in Rhai FS host fns, workspace-scoped, append-only semantics**. Reverses the initial Q5 (drop FS) decision.

API:
- Read: `read_file(path)`, `read_lines(path)`, `exists(path)`, `glob(pattern)`.
- Write: `write_file(path, content)` — throws `FileAlreadyExists` if file exists; `append_file(path, content)` — creates if missing, appends if present.
- **Not provided:** edit, delete, rename, move, overwrite. No way to mutate prior content from any host fn.

Workspace scope: `security.yaml.workspace` declares `allowed_roots: ["$CWD"]`, `deny_traversal: true`, symlinks always resolved. Pure allowlist.

Why append-only:
1. Audit trail — no silent overwrite.
2. Idempotency — re-run fails loudly on existing file (AI sees the error and adapts).
3. No read-modify-write race.
4. Tamper resistance — earlier scripts in same workspace can't be rewritten by later ones.
5. Schema simplicity — fewer edge cases.

Why built-in vs exec("cat",…) (the original Q5 pick): better ergonomics, reuses the same workspace-validation backbone (`kind: filepath` infra), no need to ship `cat`/`tee`/`mkdir` presets, AI script clearer. Trade-off accepted: not "one FS validation path" — but the validation logic is shared between built-ins and pact's `kind: filepath`.

Why AI can't bypass:
- Rhai has no FS by default — must be host-registered.
- Warden does NOT register `rhai-fs` (community FS package).
- Only workspace-scoped fns above are registered.
- `engine.set_max_modules(0)` disables import/require.
- `engine.disable_symbol("eval")` disables eval.
- AI sees only the registered surface; no backdoor.

CI tests required:
- `read_file("/etc/passwd")` → reject (outside workspace).
- `read_file("../../etc/passwd")` → reject (traversal).
- `read_file("/workspace/symlink-to-secret")` → reject (symlink resolved).
- `write_file` on existing path → reject.
- import/require/eval attempts → reject.
- Direct Rhai I/O attempts (`file_open`, `fs::read`) → function not found.

Subdecisions (defer to spec rewrite or follow-up Q):
- **Per-script disk-write cap** in `security.yaml` (e.g., `max_workspace_write_bytes: 100_000_000`) — prevent runaway `append_file` loops.
- **Workspace cleanup story** — `write_file` failing on stale files from prior failed runs is real friction; need either a `--clean-workspace` flag or operator instruction to wipe before re-run.

Q10: Output overflow behavior → **(A) throw on overflow** for in-memory exec, plus a **`stdout_to: <workspace_path>` exec parameter** as escape hatch.
- Default: when child stdout+stderr exceeds `max_output_bytes`, kill child and throw `OutputLimitExceeded { binary, bytes_seen, limit }`. Consistent with Q6 (exec throws by default).
- Escape hatch: `exec("kubectl", [...], { stdout_to: "pods.json" })` streams stdout to file (resolves to `.warden/<run-id>/pods.json` per Q15 Layer 1 write scope), bypassing in-memory cap. File subject to same Layer 1 rules: must not pre-exist (treated like `write_file`); writes outside `.warden/<run-id>/` rejected. Subject to `max_workspace_bytes` (Q15 runtime config).
- Per-binary `max_output_bytes` overrides in pact (e.g., `kubectl: max_output_bytes: 50_000_000`) recommended for known-noisy binaries.

Spec gaps to add: `security.yaml` schema (workspace, env_passthrough, audit_*, max_workspace_write_bytes); `--workspace`/`--clean-workspace` CLI flags; the FS host fns (read/write/append + glob/exists/read_lines); `stdout_to` exec parameter; the eight built-in path kinds (`filepath`, `filepath_existing`, `directory`, `glob_pattern`, plus k8s_*, etc.).

Q11: v0.1 scope vs grilled-spec scope → **let the spec grow; defer phase-cutting to a separate exercise after grill closes**. Capture the full design now (everything in this log), then run a separate planning step to slice into shippable phases. Risk: "later" never happens — mitigate by scheduling phase-cutting as the immediate next action after this grill closes. Within whatever phases emerge, the "secure spine" (engine + warden binary + one preset + exec-throws + output-cap-throws) is the natural first checkpoint.

Phase-cutting prerequisites (from verifier): (1) rewritten `spec.md` capturing all grill decisions; (2) CI test list from Q5-revised.

Q12: Concurrency in scripts → **(A) sequential only in v0.1**. Document `exec_parallel(jobs)` as planned v0.2 host fn (one new fn, no Rhai threading; engine fans out up to `max_parallel` from `security.yaml`). v0.1 callers needing parallelism do orchestrator-level fan-out (run N `warden` invocations in parallel from MCP/shell/CI). Risk: bets on orchestrator-level parallelism being good enough — if MCP integration treats Warden as the execution unit, revisit before v0.2.

Q13-sidebar resolved: `--timeout` CLI flag overrides **per-exec only**. Script-total cap lives in `security.yaml` (compile-time embedded, no runtime override). Operator who needs a different script-total recompiles.

Q15 (final FS + workspace + config-split design — supersedes parts of Q5-revised and Q14 #1/#2/#3):

**Concepts:**
- `working_dir` — base of run (default `$CWD`).
- `allowed_roots` — paths *outside* working_dir that exec() filepath args may reference (security guard for `kind: filepath`).
- Built-in FS API — scoped to `.warden/` only.

**Two non-overlapping FS layers:**
- **Layer 1 — Built-in API** (`read_file`, `write_file`, `append_file`, `read_lines`, `exists`, `glob`):
  - Read scope: `.warden/inputs/` (operator-staged) + `.warden/<run-id>/` (current run's outputs).
  - Write scope: `.warden/<run-id>/` only.
  - Read collision (same name in inputs + run-dir) → throw `AmbiguousPath`.
  - Used for script's own state.
- **Layer 2 — Exec filepath argument** (`kind: filepath` in pact):
  - Scope: `working_dir` + `allowed_roots`.
  - Security guard for binaries called via `exec()`.

**Layout:**
```
$CWD/                          ← working_dir
└── .warden/
    ├── inputs/                ← operator stages input files here
    │   └── data.json
    └── <run-id>/              ← per-run dir (script writes here)
        ├── audit.jsonl
        └── output.json
```

**Write rules:** `write_file` create-only (throws if exists), `append_file` append-or-create. No edit/delete/rename/move. Writes outside run-dir rejected.

**Config split (security boundary):**
- **`security.yaml`** (compile-time embedded — AI override = break security):
  - `working_dir`, `allowed_roots`, `deny_traversal`
  - `env_passthrough`, `max_script_seconds`
  - `audit.capture_command/stdout/stderr/sink_path`
- **runtime config** (AI override = degrade operations only):
  - `workspace.max_workspace_bytes` (default 1 GiB)
  - `workspace.auto_cleanup.{enabled,target_percent}` (default off; if on, removes oldest run-ids until ≤ target_percent of cap; never touches current run or `.warden/inputs/`; cleanups logged to audit; strategy = oldest_first, hardcoded)

**Per-binary application:**
- **`warden`** (AI direct): security.yaml + runtime config + pacts ALL compile-time embedded. AI overrides nothing. Customize = rebuild from source.
- **`warden-flex`** (trusted caller): security.yaml + pacts compile-time embedded. Runtime config provided by caller via `--config`.

**Out of scope for built-in FS:** anything outside `.warden/` requires going through `exec()` with an allowlisted binary (`cat`, `tee`, etc.) and `kind: filepath` validation against `allowed_roots`.

**Replaces from Q5-revised:** the "single FS root via `allowed_roots: [$CWD]`" model. Now two layers, clearly separated.
**Replaces from Q14:** #1 (per-run isolation now built into Layer 1's write scope), #3 (audit path is explicit at `.warden/<run-id>/audit.jsonl`).
**Tweaks Q14:** #2's `max_workspace_write_bytes` → `max_workspace_bytes` in runtime config (covers total `.warden/` not just per-script writes); auto-cleanup added.

---

Q14 (subdecision sweep, in progress):
- **#2 Per-script disk-write cap** → SUPERSEDED BY Q15. Q15 renames to `max_workspace_bytes` (default 1 GiB), moves it to *runtime config* (not security.yaml), and reframes from per-script-write cap to total-`.warden/`-size cap with optional auto-cleanup. WorkspaceQuotaExceeded throw still applies on overflow.
- **#4 Script lifecycle audit records** → emit `script_start { ts, script_path, script_sha256, args, preset }` and `script_end { ts, exit_status, duration_ms, exec_count }`.
- **#5 Env-deny audit JSONL** → REJECTED (overengineering). Stderr warning from Q9 is sufficient.
- **#6 CLI symmetry** → both binaries get `run`, `check`, `preset list`, `preset show`, `version`. Only `warden-flex` gets `--pact`, `--pact-stdin`, `--config`, `--allow-preset` flags. Only `warden` gets `init` (scaffold for direct use).
- **#1 Workspace cleanup** → **per-run isolated workspace dir**. Each `warden run` uses `$CWD/.warden/<run-id>/` as workspace root. `security.yaml.workspace_mode: per_run` (default) | `shared` (opt-in if scripts need to see prior-run output). Warden never deletes; operator manages `.warden/` lifecycle.
- **#3 Audit sink** → **single file at `$CWD/.warden/<run-id>/audit.jsonl`** (lives inside the per-run workspace dir from #1). Typed JSONL events: `script_start`, `exec_start`, `exec_end`, `stdout` (if `audit_capture_stdout`), `stderr` (if `audit_capture_stderr`), `script_end`. Forensics bundle = zip the `<run-id>/` dir → script artifacts + audit in one place. No stream mixing with script's own stderr. `security.yaml.audit_sink_path` overrides the default location if operator wants centralized logging.

Q13: Timeout model → **(A) both timeouts, both throw**.
- **Per-exec timeout** in pact (overridable per binary; default 30s). Throws `Timeout { binary, elapsed_ms, limit_ms }` on expiry; child killed.
- **Script-total timeout** in `security.yaml` (`max_script_seconds: 300` default). Engine kills script + throws `ScriptTimeout` on expiry.
- Symmetric with Q6 (throw default), Q10 (`OutputLimitExceeded`).
- `try { } catch` for tolerance.
- Note in spec: streaming/follow-mode binaries (`kubectl -f`, `tail -f`) are anti-patterns under buffered exec; document this so users don't fight the model.

Q6: `exec()` error model → **(A) `exec` throws by default; `exec_allow_fail` returns map**. Why: throw-by-default makes the happy path the safe path; forgetting error-handling fails loudly (LLM-friendly failure mode); halves host-fn count; `try { } catch { }` available for structured handling. Spec example at line 152 must be rewritten.

Q7: Distribution architecture → **ship two binaries: `warden` and `warden-flex`** (partially revises Q2).
- **`warden`** — for AI direct use. Trust boundary = the binary itself. Pact + security config compile-time embedded. NO `--pact`, `--pact-stdin`, `--config`, `--allow-preset` flags (AI controls argv, so flags = bypass). All embedded presets active. Customize = build from source.
- **`warden-flex`** — for trusted callers (MCP server, CI orchestrator, multi-tenant platform). Trust boundary = the caller. Supports `--pact <file>`, `--pact-stdin`, `--config`, `--allow-preset` to scope the run. Caller hardcodes argv; AI never touches it.

Why two binaries: a single binary with `--pact` flag means AI bypass = "agent doesn't know about the flag" (security through obscurity). Capability separation moves the boundary into the artifact itself.

Crate layout: `warden-core` (engine + executor, shared) + `warden-pact` (schema, shared) + `warden` (CLI 1) + `warden-flex` (CLI 2). Distribute via `cargo install warden` or `cargo install warden-flex`.

Q2 status: still holds *for the `warden` binary*. `warden-flex` is the trusted-caller escape hatch.

Q8: Audit model → **always-on JSONL audit, three independent capture flags in `security.yaml`**:
- `audit_capture_command: true` (default) — log binary + argv + exit_code + duration + ts on every exec().
- `audit_capture_stdout: false` (default) — opt-in for stdout content (PII risk).
- `audit_capture_stderr: false` (default) — opt-in for stderr content (also potential PII).

Audit is on for command metadata by default, off for output content. Operator can enable content capture per-deployment via `security.yaml` (compile-time embedded, same as pact).

Subdecisions resolved by Q14/Q15:
- **Sink** → file at `.warden/<run-id>/audit.jsonl` (Q14-#3, Q15). Path overridable via `security.yaml.audit.sink_path`.
- **Format** → JSONL, typed events (Q14-#3).
- **Script lifecycle records** → yes, emit `script_start` + `script_end` (Q14-#4).

Q9: Untrusted inputs (env + CLI args) → **(B) strict env allowlist + raw args (validated at exec)**, with refinement on env-deny behavior.
- **Env**: `security.yaml` declares `env_passthrough: [PATH, HOME, LANG]`. `env(key)` returns value if allowed; if denied, **emit stderr warning + return `""` (do not throw)**. Reason: optional-env pattern (`env("DEBUG") || ""`) is common and legitimate; throwing breaks scripts. Warning still surfaces probing attempts and can be audit-logged.
- **Args**: `script_args()` returns raw strings. Validation happens when args flow into `exec()` (engine matches positional/flag against pact's `kind:` schema). Args used purely in control flow (`if script_args()[0] == "fast"`) are unvalidated — acceptable, can't escape sandbox.
- **Defer**: per-script `args_schema` declaration (Option A) — only add post-v0.1 if real users hit the ergonomics gap.

Subdecision: should env-deny also emit an audit JSONL record (not just stderr warn)? Recommended yes — operator can detect probing patterns. Defer to Q8 audit-schema work.

Q9-sidebar resolved: `env_passthrough` → `security.yaml` (process-level). `env_overrides` → **stays per-binary in pact** (binary-specific; e.g., `kubectl` gets `KUBECONFIG`, `jq` doesn't).

## Final Review

**Verdict (verifier):** conflict — five items flagged.

**Resolved at close (in-log bookkeeping):**
- Q14-#2 explicitly marked superseded by Q15 (field renamed to `max_workspace_bytes`, moved to runtime config, reframed as total-`.warden/`-size cap).
- Q8 "open subdecisions" closed: sink → `.warden/<run-id>/audit.jsonl` (Q14-#3, Q15); format → JSONL typed events; lifecycle records → yes.

**Carried forward as known followup (not blocking):**
- Spec.md rewrite required across at least: pact schema (Q3-revised — drop `forbidden_*`), exec error model (Q6 — invert exec/exec_or_fail), distribution (Q7 — single-binary diagram + CLI need split into warden/warden-flex), CLI flag list (lines 200–213 — split per binary), example script (lines 147–172 — use throwing exec), crate layout (line 298 — split warden-cli into warden + warden-flex), open-questions list (#1, #2, #4, #5 are now answered), success criteria (resource-limit names + measurement task), plus net-new sections: "Why Rhai", security.yaml schema, runtime-config schema, FS host fns, audit JSONL event schema, two-layer FS model with `.warden/` layout, named-kinds catalog.
- Phase-cutting exercise still owed (Q11). Prerequisite: rewritten spec + Q5 CI-test list.

**Closing posture:** all foundational architectural decisions are resolved and internally consistent post-bookkeeping fix. Spec rewrite is mechanical; phase-cutting is the next session.
