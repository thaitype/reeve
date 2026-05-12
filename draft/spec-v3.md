# Reeve — Design Spec (v3)

> Supersedes `draft/spec-v2.md`. This revision shifts Reeve's primary persona
> from "co-equal CI/runbook/AI runtime" to **"AI agent on a bare VM"**, and
> folds in 17 grill decisions captured in this draft's grill table at the
> bottom. Phase-cutting (what actually lands in v0.1 vs v0.2) remains
> deferred — captured separately after this spec is ratified.

## Overview

Reeve is an allowlist-first shell automation runtime for AI agents operating
close to the OS on a VM or container. It runs scripts written in Rhai (an
embedded scripting language), where every external command and filesystem
reference must pass an allowlist policy declared in a pact (YAML) plus a
security boundary declared in `security.yaml`.

**Primary persona:** an AI agent invoked from a shell on a bare VM, with its
own per-user state at `$HOME/.reeve/`. CI/CD, runbooks, MCP servers, and
multi-tenant platforms remain supported through the `reeve-flex` binary, but
they are no longer co-equal — they reuse the AI-agent runtime via the
trusted-caller variant.

> Reeve is part of the chief-tribe ecosystem as a generic execution layer —
> usable standalone or paired with other agents (Sage, Chief, Chieftain,
> Council).

## Goals

- **Clear safety boundary** — scripts can only do what the pact + security
  config permit; no shell injection, no arbitrary binary execution, no
  filesystem escape.
- **Full logic flow** — for, if, let, function, array, map (via Rhai), not
  just declarative.
- **Lightweight** — no daemon, no runtime dependency. (Two CLI binaries; see
  Distribution.)
- **Auditable** — both pacts and scripts are text files (reviewable,
  version-controllable), and every run produces a structured JSONL audit log.
- **Embeddable** — callable from subprocess, MCP server, or CI runner
  directly via the `reeve-flex` binary.

## Non-goals

- Replacing bash for interactive use.
- Full POSIX shell compatibility.
- Kernel-level sandbox (network/syscall isolation) — use bwrap/firejail on
  top if needed.
- Remote pact loading.
- Multi-language support — Rhai only.
- Total-run wall-clock timeout enforced by the engine (operators wrap with
  external `timeout` if they need it; see Concurrency & timeouts).

## Why Rhai

Unchanged from v2. Rhai chosen over Lua, Starlark, and JavaScript:

**Rhai wins:** pure Rust (no C dep), sandbox by default (no stdlib to strip),
first-class resource limits (`set_max_operations`, etc.), `disable_symbol()`,
type-safe Rust↔script FFI.

**Trade-offs accepted:** smaller LLM training corpus than Lua/Python/JS
(mitigated by `SKILL.md` + host-fn docs); less engineer familiarity than Lua
(mitigated by JS-like syntax).

**Considered alternatives:**
- Lua (mlua): rejected — C dep, manual sandbox setup.
- Starlark: viable; loses `while`/recursion/exception ergonomics. Reconsider
  if determinism becomes a goal.
- JavaScript (QuickJS/boa): rejected — heavier engine, larger attack
  surface, larger binary.

## Distribution: two binaries

Two CLI binaries ship from the same single `reeve` crate (declared as two
`[[bin]]` entries in `Cargo.toml`), with different trust models. Both are
installed by `cargo install reeve`.

### `reeve` — for AI direct use

- Trust boundary: the binary itself.
- Pacts, `security.yaml` (including `reeve_home`), all compile-time embedded
  (`include_str!`).
- **No** `--pact`, `--pact-stdin`, `--config`, `--allow-preset`,
  `--reeve-home` flags. AI agents control argv; runtime flags would be
  bypass.
- **Does not read `REEVE_HOME` from env.** `reeve_home` value comes only
  from compile-time `security.yaml`.
- All embedded presets active for every run.
- Customize → fork repo → edit YAML → `cargo build --release`.

### `reeve-flex` — for trusted callers (MCP servers, CI orchestrators, multi-tenant platforms)

- Trust boundary: the caller that invokes `reeve-flex`.
- Pacts and `security.yaml` compile-time embedded (security-critical).
- Runtime config (`config.json`) provided by caller via `--config`.
- `REEVE_HOME` env var honored — caller (trusted) controls workspace
  location.
- Caller may pass `--pact <file>`, `--pact-stdin`, `--allow-preset <name>`
  to scope each run.

### Why two binaries (not one with flags)

A single binary with `--pact` exposes runtime-pact override. AI agents
calling that binary can pass `--pact /tmp/evil.yaml` and bypass the
allowlist. "Agent doesn't know about the flag" = security through
obscurity. Capability separation moves the trust boundary into the artifact
itself. The same logic applies to `REEVE_HOME` — env vars are flags by
another name when the AI controls its own process environment.

| | `reeve` | `reeve-flex` |
|---|---|---|
| `run script.rhai` | ✅ | ✅ |
| `check script.rhai` | ✅ | ✅ |
| `preset list`, `preset show <name>` | ✅ | ✅ |
| `version` | ✅ | ✅ |
| `--allow-preset` | ❌ | ✅ |
| `--pact <file>` | ❌ | ✅ |
| `--pact-stdin` | ❌ | ✅ |
| `--config <file>` | ❌ | ✅ |
| `REEVE_HOME` env var | ❌ | ✅ |
| Trust boundary | Binary | Caller |
| Default consumer | AI direct | MCP/CI/platform |

## Filesystem model

Reeve maintains per-user state at **`$HOME/.reeve/`** (default; the path is
controlled by `reeve_home` in `security.yaml` — see Distribution).

### Layout

```
$REEVE_HOME/                          # default $HOME/.reeve/
├── config.json                       # runtime config (engine-managed JSON)
├── .reeve-managed                    # sentinel; engine writes on first init
├── workspace/                        # script's playground (Layer 1 r/w)
│   └── ...arbitrary script artifacts...
└── runs/
    └── <run-id>/
        └── audit.jsonl               # engine writes only; scripts cannot reach
```

- **`workspace/`** is shared across all runs. Append-only semantics for
  writes (see Layer 1). The script and operator-staged files coexist here;
  no separate `inputs/` directory.
- **`runs/<run-id>/`** holds only `audit.jsonl` in v0.1. This directory is
  the reserved mount point for the future `--isolated` flag (see Future);
  scripts have no way to reach it through Layer 1 or Layer 2.
- **`config.json`** is engine-managed. Operators may tune values, but
  comments do not survive — the engine owns the file format.
- **`.reeve-managed`** is a sentinel file written by the engine on first
  init. Its presence does not enforce anything in v0.1; future versions may
  use it to detect "Reeve home pointed at an existing populated directory"
  shadowing attempts.

### File-format split

| File | Format | Why |
|---|---|---|
| `config.json` | JSON | Machine-managed; aligns with AI-tool ecosystem (Claude Desktop, MCP, npm-style configs). |
| `security.yaml` | YAML | Compile-time embedded; hand-edited by trusted operator; needs comments. |
| `pacts/*.yaml` | YAML | Heavily comment-driven ("# omitted: --exec is RCE"); JSON would be hostile to authoring. |

Rule: **machine-managed → JSON. Human-authored → YAML.**

### Layer 1 — Built-in FS host functions

Scoped to `<reeve_home>/workspace/` only — hardcoded, **not** widened by
`allowed_roots`. Append-only semantics for all writes.

```rhai
// Read (workspace/ only)
read_file(path) -> string
read_lines(path) -> array<string>
exists(path) -> bool
glob(pattern) -> array<string>

// Write (workspace/ only)
write_file(path, content)        // create only — throws FileAlreadyExists if exists
append_file(path, content)       // append, or create if missing
```

**No** `edit`, `delete`, `rename`, `move`, or overwrite host fns. Append-only
gives:

1. Audit trail — no silent overwrite.
2. Idempotency — re-run fails loudly on existing file.
3. No read-modify-write race.
4. Tamper resistance — earlier scripts can't be rewritten by later ones.
5. Schema simplicity — fewer edge cases.

**Cross-run contamination is a feature, not a bug.** Since `workspace/` is
shared globally per user, a script that re-runs and hits a name collision
gets `FileAlreadyExists` — telling the operator "you already have output
from this script; clean up or change the filename." This is the same loud
failure as v2's per-run model, just applied across sessions.

### Layer 2 — Exec filepath arguments

Paths flowing into `exec()` via `kind: filepath` (or its variants) are
validated against `allowed_roots`. Symlinks resolved.

This is the **only** way scripts reach files outside `workspace/` — by
running an allowlisted binary (`cat`, `grep`, `jq`, etc.) whose pact accepts
a `filepath` argument.

`allowed_roots` defaults:

```yaml
allowed_roots:
  - "$CWD"                            # operator-controlled scope
  - "$HOME/.reeve/workspace"          # also reachable via exec
```

`$CWD` is the invoker's working directory at the moment Reeve is spawned —
this is how scripts inspect arbitrary project trees, checked-out repos, etc.
Caller responsibility: don't spawn Reeve from `/`, `/etc`, or other
system-root paths.

### No `denied_roots`

v3 preserves v2's pure-allowlist invariant: **anything not declared is
denied.** A `denied_roots` key would have been redundant (`<reeve_home>/runs`
and `<reeve_home>/config.json` are unreachable because their parent isn't in
`allowed_roots`) and would have introduced precedence ambiguity. Dropped.

### Two-gate exec path check

`exec("cat", ["/etc/passwd"])` goes through two independent gates:

1. **Pact gate:** is `cat` allowed? Is `kind: filepath` declared for its
   positional? If not → reject.
2. **`allowed_roots` gate:** does `/etc/passwd` resolve under any allowed
   root? If not → reject.

Both gates must pass. Symlink traversal is resolved before the
`allowed_roots` check.

### Bypass resistance

- Rhai has no FS by default.
- Reeve does NOT register `rhai-fs` (community FS module).
- Only the host fns above are registered.
- `engine.set_max_modules(0)` disables `import`/`require`.
- `engine.disable_symbol("eval")` disables `eval`.
- Direct Rhai I/O attempts (`file_open`, `fs::read`) → function not found.

### Audit log protection

Audit log at `<reeve_home>/runs/<run-id>/audit.jsonl` is "protected" at two
levels:

1. **Layer 1 scope:** Script's FS API cannot read it — Layer 1 is hardcoded
   to `workspace/`.
2. **Layer 2 scope:** Script's `exec()` cannot reach it — `runs/` is not in
   `allowed_roots`.

**Not protected at OS level.** Reeve writes the audit log with default
perms; a same-user process outside Reeve can read or tamper with it. The
threat model is **the script**, not "another process running as the same
user." Document this boundary honestly — anyone who has same-user shell
access on the VM can already do everything Reeve can.

### Future: `--isolated` (deferred)

A future CLI flag (`reeve run --isolated script.rhai` or
`reeve-flex run --isolated ...`) will rebind Layer 1 scope:

| | Default (v0.1) | `--isolated` (future) |
|---|---|---|
| `write_file("report.json")` lands at | `workspace/report.json` | `runs/<run-id>/report.json` |
| `read_file("foo")` reads from | `workspace/` | `runs/<run-id>/` |
| `audit.jsonl` location | `runs/<run-id>/audit.jsonl` | same |
| Cross-run visibility | Yes (shared workspace) | No |
| Forensic bundle | audit.jsonl only | `zip runs/<run-id>/` → audit + all script artifacts |

The script API is identical between modes — `--isolated` is a pure
mount-point swap. Deferred because shipping both day-one would require
designing run-cleanup, run-listing, and run-resume semantics now.

## Pact (YAML)

Unchanged from v2. A pact is an allowlist policy file. Pure allowlist
semantics — no `forbidden_*` keys. Every flag, subcommand, and positional
argument must be explicitly allowed.

### Schema (v1)

```yaml
version: 1
name: k8s-readonly
description: Read-only Kubernetes investigation

defaults:
  timeout_seconds: 30
  max_output_bytes: 10485760    # 10 MiB

binaries:
  kubectl:
    path: /usr/bin/kubectl
    env_overrides:
      KUBECONFIG: /etc/reeve/readonly-kube/config
    subcommands:
      get:
        allowed_flags: [-n, --namespace, -o, -l, --selector, --all-namespaces]
        flag_values:
          "-o": { kind: enum, values: [json, yaml, wide, name] }
          "-n": { kind: k8s_namespace }
          "--namespace": { kind: k8s_namespace }
          "-l": { kind: k8s_selector }
          "--selector": { kind: k8s_selector }
        positional:
          - { kind: enum, values: [pods, deployments, services, nodes, configmaps] }
          - { kind: k8s_name, optional: true }
      # ... logs, describe, etc.
```

### Built-in named kinds

Implemented in Rust once, reused across pacts:

- `enum` — exact-match against value list.
- `number` — non-negative integer.
- `duration` — Go-style duration string (`30s`, `5m`, `1h`).
- `filepath` — path argument validated against Layer 2 scope
  (`allowed_roots`). Symlinks resolved; traversal rejected.
- `filepath_existing` — like `filepath` but the path must exist.
- `directory` — like `filepath` but must be a directory.
- `glob_pattern` — shell glob; expanded against Layer 2 scope.
- `k8s_name`, `k8s_namespace`, `k8s_selector` — Kubernetes-specific.
- *(extensible — new kinds added in `src/pact/kinds/`)*

### Custom validator escape hatch

For DSL-shaped flag values:

```yaml
flag_values:
  "-o":
    kind: custom
    name: kubectl_output_flag    # → fn validate_kubectl_output_flag(s: &str)
```

Adding one requires a code PR (intentional friction).

### Risk categories (documentation, not enforcement)

Each shipped preset declares a category in its README:

- **PureRead** — no side effects (e.g., `kubectl get`, `cat`, `ls`).
- **Standard** — read-side-effecty or recursive-read but bounded
  (`kubectl logs`, `find` without `-exec`/`-delete`, `grep -r`).
- **RequiresAudit** — has dangerous flags omitted from allowlist.
- **Forbidden** — never shipped (`kubectl apply`, `rm`).

## `security.yaml`

Compile-time embedded for both binaries. Changes require recompile —
overriding security config = breaking the security boundary.

```yaml
# security.yaml
reeve_home: "$HOME/.reeve"      # root of Reeve's per-user state
                                # default ships as $HOME/.reeve;
                                # AI-facing production builds should rebuild
                                # with a literal path (e.g. /var/lib/reeve)
                                # if $HOME is AI-controllable.

allowed_roots:                  # exec() filepath args must resolve under one of these
  - "$CWD"
  - "$HOME/.reeve/workspace"
deny_traversal: true            # reject "../" in any path

env_passthrough: [PATH, HOME, LANG]

audit:
  capture_command: true         # log binary + argv + exit_code + duration
  capture_stdout: false         # log child stdout content (PII risk)
  capture_stderr: false         # log child stderr content (PII risk)
```

Removed since v2: `working_dir` (implicit `$CWD`), `workspace_mode` (single
mode in v3), `audit.sink_path` (hardcoded under `reeve_home/runs/<run-id>/`),
`max_script_seconds` (see Concurrency & timeouts).

## Runtime config

Operational tuning. For `reeve`: compiled-in. For `reeve-flex`: provided by
caller via `--config <file>`. Overriding these can degrade operations but
cannot break the security boundary.

```json
{
  "max_workspace_bytes": 1073741824,
  "auto_cleanup": {
    "enabled": false,
    "target_percent": 80
  }
}
```

- `max_workspace_bytes` — total cap on `<reeve_home>/` (workspace + runs
  combined). Measured **once at startup** by walking the tree; if over,
  engine exits with `WorkspaceQuotaExceeded` (exit code 5) before loading
  the script. If `auto_cleanup.enabled`, cleanup runs first; re-walk; exit
  only if still over.
- `auto_cleanup` — when enabled, engine deletes oldest entries in `runs/`
  until total size ≤ `target_percent` of `max_workspace_bytes`. Never
  touches the current run. Strategy is `oldest_first` (hardcoded).
  Cleanup events are logged to the new run's audit.

The cap is a **next-run gate**, not in-flight enforcement: a runaway script
that writes infinitely during one run can exceed the cap; the bill comes
due next invocation. In-flight protection comes from per-exec output caps
(`max_output_bytes`) and Rhai op limits.

## Script (Rhai)

Standard Rhai syntax with restricted environment.

### Built-in host functions

```rhai
// Process execution
exec(binary: string, args: array, opts?: map) -> map
// throws { binary, exit_code, stdout, stderr } on non-zero exit
// throws Timeout on per-exec timeout
// throws OutputLimitExceeded on stdout+stderr > max_output_bytes
// returns: { stdout: string, stderr: string, exit_code: int, duration_ms: int }
//
// opts:
//   stdin: <string>              // feed string as child stdin
//   stdout_to: <relative path>   // stream stdout to workspace/<path>
//                                // file must not pre-exist (write_file rules)
//                                // exits in-memory cap mode
//   timeout_seconds: <int>       // per-exec override (must be ≤ pact default)

exec_allow_fail(binary, args, opts?) -> map
// never throws on non-zero exit; caller inspects exit_code
// Timeout / OutputLimitExceeded still throw

// Pipelined exec — chain N commands with OS-level pipes between stages
pipe(stages: array, opts?: map) -> map
// stages: array of [binary, ...args] arrays; each stage independently
//   pact-validated (same gates as exec()).
// Pipefail semantics: throws on the FIRST non-zero stage exit, carrying
//   { stage_index, binary, exit_code, stdout, stderr }.
// Throws Timeout / OutputLimitExceeded same as exec().
// returns: final stage's { stdout, stderr, exit_code, duration_ms }
// opts.stdin feeds the first stage; opts.stdout_to streams the final stage.

pipe_allow_fail(stages, opts?) -> array<map>
// per-stage results; never throws on non-zero exit at any stage.
// Timeout / OutputLimitExceeded still throw.

// Data parsing
parse_json(s) -> dynamic
parse_yaml(s) -> dynamic
parse_toml(s) -> dynamic
to_json(v) -> string

// Filesystem (Layer 1 only — workspace/ scope)
read_file(path) -> string
read_lines(path) -> array<string>
exists(path) -> bool
glob(pattern) -> array<string>
write_file(path, content)
append_file(path, content)

// Environment & arguments
env(key) -> string
// allowed keys (security.yaml.env_passthrough): returns value
// denied keys: throws EnvDenied
// unset (but allowed) keys: throws EnvUnset
// callers wanting probe behavior must guard explicitly
script_args() -> array
// raw CLI args after script path; validated by pact when passed to exec()

// Output / logging
print(...)
log_info(msg)
log_warn(msg)
log_error(msg)
// log_* emits both to stderr and as audit events
// (event: "script_log", level, msg)
```

### Rhai language posture

- Standard: `let`, `for`, `if/else`, `while`, `fn`, array, map, string template.
- Disabled: `eval`, module loading (`import`/`require`).
- Resource limits:
  - `set_max_operations(1_000_000)`
  - `set_max_call_stack_depth(32)`
  - `set_max_string_size(102_400)` — 100 KiB
  - `set_max_array_size(10_000)`
  - `set_max_modules(0)`

### Concurrency & timeouts

Rhai is single-threaded. v0.1 supports sequential `exec()` plus pipelined
`pipe()` (single chain at a time). Parallel fan-out (`exec_parallel`)
planned for v0.2.

**Timeouts:**
- **Per-exec timeout** — pact default + `opts.timeout_seconds` override. On
  fire, child killed, `Timeout` thrown.
- **Per-pipe timeout** — same per-stage; total pipe duration is the sum.
- **No script-total timeout enforced by Reeve.** Operators wrap with
  external `timeout 300 reeve run script.rhai` if they need a wall-clock
  bound. Trade-off accepted: external `timeout` kills the process with
  SIGTERM, which may truncate `audit.jsonl` mid-write.

### Streaming/follow-mode anti-pattern

Unchanged from v2. Don't allowlist `kubectl logs -f`, `tail -f`,
`kubectl get --watch` — they conflict with the buffered exec model.

### Example script

```rhai
// investigate-failing-pods.rhai

let result = exec("kubectl", ["get", "pods", "-n", "prod", "-o", "json"]);
let pods = parse_json(result.stdout);

let failing = [];
for pod in pods.items {
    if pod.status.phase != "Running" {
        failing.push(pod.metadata.name);
    }
}

log_info(`Found ${failing.len()} failing pods`);

// Persist a structured report. Throws if a previous run already wrote it.
write_file("failing-pods.json", to_json(failing));

for name in failing {
    print(`\n=== ${name} ===`);
    let logs = exec_allow_fail("kubectl", ["logs", name, "-n", "prod", "--tail=50"]);
    if logs.exit_code == 0 {
        append_file("pod-logs.txt", `\n=== ${name} ===\n${logs.stdout}`);
    } else {
        log_warn(`logs failed for ${name}: ${logs.stderr}`);
    }
}

// Pipelined data extraction — single audit event, OS pipes between stages.
let names = pipe([
    ["kubectl", "get", "pods", "-n", "prod", "-o", "json"],
    ["jq", "-r", ".items[].metadata.name"]
]);
log_info(`pipeline produced ${names.stdout.split("\n").len()} names`);
```

## Executor — `exec()` flow

```
exec(binary, args, opts) flow:
  1. Look up binary in active pacts → not found → throw BinaryNotAllowed
  2. Validate first positional as subcommand → throw SubcommandNotAllowed
  3. Validate flags (name in allowed_flags) → throw FlagNotAllowed
  4. Validate flag values (kind dispatch) → throw FlagValueRejected
  5. Validate positionals (kind dispatch) → throw PositionalRejected
  6. If filepath kind appears → resolve symlinks, check Layer 2 scope
  7. Build Command:
     - absolute path from pact
     - argv array (no shell:true)
     - env_clear() then set security.yaml.env_passthrough + pact.env_overrides
  8. Spawn with per-exec timeout (pact default or opts override)
  9. If opts.stdin provided → feed to child stdin
 10. Capture stdout/stderr (size-limited to max_output_bytes) OR stream to
     workspace/<stdout_to> if opts.stdout_to set
 11. On overflow → kill child, throw OutputLimitExceeded
 12. On per-exec timeout → kill child, throw Timeout
 13. On non-zero exit → throw ExecFailed (unless caller used exec_allow_fail)
 14. Emit audit events (exec_start, exec_end, optional stdout/stderr captures)
 15. Return { stdout, stderr, exit_code, duration_ms }
```

### `pipe()` flow

```
pipe(stages, opts) flow:
  1. For each stage, run steps 1-7 of exec validation; if ANY fails, throw
     before spawning any child.
  2. Spawn all stages with OS pipes connecting them (stage_i.stdout →
     stage_{i+1}.stdin). First stage stdin = opts.stdin or null.
  3. Apply per-stage timeout; on fire, kill all stages, throw Timeout
     carrying { stage_index }.
  4. Apply max_output_bytes to the FINAL stage's stdout only (intermediate
     stages stream through OS pipes — bounded by kernel pipe buffers).
  5. Emit audit events: pipe_start (all stages), exec_start/exec_end per
     stage, pipe_end with final status.
  6. On first non-zero exit (any stage):
       - pipe(): kill remaining stages, throw PipeStageFailed carrying
         { stage_index, binary, exit_code, stdout, stderr }.
       - pipe_allow_fail(): collect all per-stage results, return array.
  7. Return final stage's { stdout, stderr, exit_code, duration_ms } (pipe)
     or array of all stage results (pipe_allow_fail).
```

## Audit log

Always-on. JSONL at `<reeve_home>/runs/<run-id>/audit.jsonl`. Sink path is
hardcoded — operators cannot redirect.

### Event schema

```jsonl
{"event":"script_start","ts":"...","run_id":"abc123","script_path":"...","script_sha256":"...","args":[...],"presets":[...]}
{"event":"exec_start","ts":"...","binary":"kubectl","argv":[...]}
{"event":"exec_end","ts":"...","binary":"kubectl","exit_code":0,"duration_ms":234,"stdout_bytes":12044,"stderr_bytes":0}
{"event":"pipe_start","ts":"...","stages":[["kubectl","get",...],["jq","-r",...]]}
{"event":"pipe_end","ts":"...","exit_code":0,"duration_ms":312,"stage_count":2}
{"event":"stdout","ts":"...","binary":"kubectl","content":"..."}    // only if audit.capture_stdout
{"event":"stderr","ts":"...","binary":"kubectl","content":"..."}    // only if audit.capture_stderr
{"event":"exec_error","ts":"...","binary":"kubectl","kind":"Timeout","limit_ms":30000}
{"event":"script_log","ts":"...","level":"info","msg":"..."}
{"event":"cleanup","ts":"...","removed_run_ids":[...],"bytes_freed":...}
{"event":"script_end","ts":"...","exit_status":"ok","duration_ms":1234,"exec_count":5}
```

`audit.capture_command` is always true. `capture_stdout`/`capture_stderr`
are off by default; flipping them captures full child output into the audit
log (PII risk — operator decides at compile time).

The audit file is written by Reeve, not by the script.

### Forensics bundle

In v0.1, the forensic artifact is the single `audit.jsonl` — script
artifacts live in shared `workspace/` and are not run-tagged on disk
(though every write is recorded in audit, so artifacts can be reconstructed
post-hoc by replaying the log).

Once `--isolated` ships, `zip runs/<run-id>/` will produce a self-contained
bundle (audit + all script artifacts for that run).

## CLI

### `reeve`

```bash
reeve run <script.rhai>
reeve check <script.rhai>          # static validation; no exec
reeve preset list
reeve preset show <name>
reeve version
```

Flags:
- `--timeout <seconds>` — per-exec timeout override (cannot exceed pact default).
- `--quiet` — suppress logs except errors.
- `--json` — output structured JSON instead of text.

Removed since v2: `init` (engine lazy-creates `<reeve_home>/{workspace,runs}`
and writes default `config.json` on first run, with a one-line stderr
notice), `--workspace <dir>` (no per-project root in v3).

### `reeve-flex`

Same subcommands as `reeve`, plus:

```bash
reeve-flex run <script> --allow-preset <name>
reeve-flex run <script> --pact <file>
reeve-flex run --pact-stdin --script-stdin < bundle.json
reeve-flex run <script> --config <runtime-config.json>
REEVE_HOME=/var/lib/reeve-agent reeve-flex run <script>
```

### Exit codes

- `0` — success.
- `1` — script error (Rhai runtime, exec failure surfaced as exception).
- `2` — pact violation (binary/flag/value not allowed).
- `3` — configuration error (pact parse error, missing preset, security.yaml
  invalid).
- `4` — **per-exec** timeout (no script-total timeout exists in v3; external
  `timeout` produces SIGTERM, which the OS reports separately).
- `5` — workspace quota exceeded (at startup walk).

## Built-in Presets (v0.1)

Three presets ship in `reeve` (all compile-time-active) and are available
to `reeve-flex` via `--allow-preset`:

- **`core-tools`** — shared FS/data utilities: `cat`, `grep` (read-only
  flags), `jq`, `yq`, `head`, `tail`, `wc`, `sort`, `uniq`, `find`
  (without `-exec`/`-delete`). Category: **Standard** (recursive read).
- **`k8s-readonly`** — `kubectl get/describe/logs/top/events/version`.
  Does NOT bundle jq/cat/grep (those come from `core-tools`).
  Category: **Standard**.
- **`git-readonly`** — `git log/show/diff/status/blame/ls-files`.
  Category: **PureRead**.

Presets are stackable: a script using `kubectl get | jq` needs both
`k8s-readonly` and `core-tools` active.

Future: `db-readonly`, `azure-readonly`, `aws-readonly`, `devops-basic`.

## Security model

### Defense layers

| Layer | Mechanism |
|---|---|
| Language | Rhai — no eval, no import, no reflection, no FFI |
| Engine | Resource limits (operations/stack/string/array/modules) |
| Host fn | Validate args, no shell layer, env_clear + allowlist |
| Pact | Pure allowlist of binaries + subcommands + flags + values |
| Filesystem | Two-layer model; Layer 1 hardcoded to `<reeve_home>/workspace/`; Layer 2 mediated by pact `kind: filepath` + `allowed_roots` |
| Pact source | Compile-time embedded, immutable at runtime (reeve) |
| Audit | Always-on JSONL; metadata always captured; script-unreachable |
| External system | (User responsibility) Use scoped credentials |

### Threat model — mitigated

- **Script injection via crafted input** — Rhai parses, no shell layer.
- **Binary substitution** — absolute path required in pact.
- **Flag injection (`--kubeconfig=evil`)** — pure allowlist; flags not
  declared are rejected.
- **DSL injection in flag values (`-o jsonpath=...`)** — `flag_values`
  declares per-flag kinds; custom validator escape hatch handles DSLs.
- **Resource exhaustion (CPU/memory/output)** — Rhai op limit, per-exec +
  output size caps, workspace size cap (startup gate).
- **Pact tampering at runtime (`reeve`)** — embedded at compile time; no
  `--pact` flag.
- **`reeve_home` redirection by AI** — `reeve` does not read `REEVE_HOME`
  from env; value comes only from compile-time `security.yaml`.
- **AI agent generating malicious scripts** — bound by pact + Layer 1 FS
  scope + audit log.
- **Filesystem exfiltration** — Layer 1 confined to `workspace/`; Layer 2
  bounded by `allowed_roots`; pure allowlist for paths.

### Threat model — out of scope

- Bugs in host function code (contributor / fuzz-test responsibility).
- Kernel-level escape — use bwrap/firejail externally.
- Network exfiltration — use OS-level network policy.
- Side channels (timing, memory pressure).
- ReDoS from operator-authored regex in pact `flag_values` — operator
  responsibility.
- **Same-user tampering with `audit.jsonl`** — audit is protected against
  the script, not against other processes running as the same user. If the
  AI has shell access outside Reeve, it can edit audit log files directly
  via normal file I/O. Mitigation requires OS-level immutable attrs or a
  separate audit user (out of v0.1 scope).
- **Script-total wall-clock runaway** — no engine-enforced cap. Wrap
  invocations with external `timeout` if needed.

### Custom pact path (reeve-flex only)

```bash
reeve-flex run script.rhai --pact ./custom.yaml
# stderr: WARN: using custom pact, not a built-in preset
```

This is **only** in `reeve-flex`. The trust assumption is that the caller
spawning `reeve-flex` is already trusted (MCP server, CI orchestrator). AI
agents calling `reeve` directly cannot reach this path.

## Repository structure

Single `reeve` crate at the repo root. Two binaries are declared as `[[bin]]`
entries in the one `Cargo.toml`.

```
reeve/
├── Cargo.toml                          # single-crate manifest, two [[bin]] entries
├── README.md
├── LICENSE
│
├── src/
│   ├── lib.rs
│   │
│   ├── bin/
│   │   ├── reeve.rs                    # CLI 1: AI direct use
│   │   └── reeve_flex.rs               # CLI 2: trusted callers
│   │
│   ├── core/                           # Rhai engine + executor + audit + FS host fns
│   │   ├── mod.rs
│   │   ├── engine.rs
│   │   ├── executor.rs                 # exec() + pipe() host fns + flow
│   │   ├── fs.rs                       # Layer 1 FS host fns
│   │   ├── audit.rs
│   │   ├── timeout.rs                  # per-exec timer
│   │   └── home.rs                     # reeve_home init + sentinel
│   │
│   └── pact/
│       ├── mod.rs
│       ├── schema.rs
│       ├── engine.rs
│       ├── kinds/
│       │   ├── mod.rs
│       │   ├── filepath.rs
│       │   ├── enum_kind.rs
│       │   ├── number.rs
│       │   ├── duration.rs
│       │   └── k8s.rs
│       ├── custom/
│       │   ├── mod.rs
│       │   └── kubectl.rs
│       └── presets.rs                  # include_str! embed
│
├── pacts/
│   ├── core-tools.yaml
│   ├── k8s-readonly.yaml
│   └── git-readonly.yaml
│
├── security.yaml                       # default security boundary (embedded)
├── config.json.default                 # default runtime config (embedded into reeve)
│
├── examples/
│   ├── investigate-pods.rhai
│   └── git-history.rhai
│
└── tests/
    ├── integration/
    │   ├── pact_engine.rs
    │   ├── fs_layer1.rs                # bypass-resistance suite
    │   ├── pipe.rs
    │   └── audit.rs
    └── fixtures/
```

## Dependencies

```toml
[dependencies]
rhai = { version = "1.19", features = ["sync", "serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
anyhow = "1"
thiserror = "1"
regex = "1"
uuid = { version = "1", features = ["v4"] }
chrono = "0.4"
wait-timeout = "0.2"
clap = { version = "4", features = ["derive"] }
```

## CI test suite (required before v0.1 ship)

Bypass-resistance suite — every item must be a passing test:

- `read_file("/etc/passwd")` → reject (outside `workspace/`).
- `read_file("../../etc/passwd")` → reject (Layer 1 + traversal).
- `read_file("/workspace/symlink-to-secret")` → reject (symlink resolved).
- `read_file("../runs/<run-id>/audit.jsonl")` → reject (Layer 1 scope).
- `write_file` to existing path → reject `FileAlreadyExists`.
- `append_file` outside `workspace/` → reject.
- `import "fs"` / `require("os")` → reject (modules disabled).
- `eval("...")` → reject (symbol disabled).
- Direct Rhai I/O attempts (`file_open`, `fs::read`) → function not found.
- `exec("rm", ["-rf", "/"])` → reject (binary not in pact).
- `exec("kubectl", ["apply", ...])` → reject (subcommand not allowed).
- `exec("kubectl", ["get", "pods", "--kubeconfig=/tmp/x"])` → reject.
- `exec("kubectl", ["get", "secrets"])` → reject (positional not in enum).
- `exec("cat", ["~/.reeve/runs/<id>/audit.jsonl"])` → reject (Layer 2
  scope; `runs/` not in `allowed_roots`).
- `exec("cat", ["~/.reeve/config.json"])` → reject (same).
- `exec(...)` exceeding per-exec timeout → throw `Timeout`.
- `exec(...)` exceeding `max_output_bytes` → throw `OutputLimitExceeded`.
- `pipe([...])` with a non-zero middle stage → throw `PipeStageFailed`
  carrying the right `stage_index`.
- `pipe_allow_fail([...])` with a non-zero middle stage → returns full
  per-stage results array.
- Startup with workspace exceeding `max_workspace_bytes` →
  `WorkspaceQuotaExceeded` exit code 5 before script loads.
- `env("AWS_SECRET_ACCESS_KEY")` → throws `EnvDenied`.
- `env("UNSET_BUT_ALLOWED_VAR")` → throws `EnvUnset`.
- `--pact` flag on `reeve` binary → CLI parse error (unknown flag).
- `REEVE_HOME=/tmp/x reeve run ...` → env var ignored; `reeve_home` from
  compile-time `security.yaml`.
- `REEVE_HOME=/tmp/x reeve-flex run ...` → env var honored.

## Success criteria

- v0.1 binaries: `reeve` < 10 MB, `reeve-flex` < 10 MB, cold start < 50ms
  (verify with measurement task before declaring v0.1 ready).
- All bypass-resistance tests pass.
- Three presets (`core-tools`, `k8s-readonly`, `git-readonly`) with
  PureRead and Standard examples.
- README + examples sufficient for adoption within 30 minutes.
- Test coverage > 80% in `core::*` and `pact::*` modules.

## Followups (post-grill)

1. **Phase-cutting** — slice this spec into shippable v0.1 / v0.2 / v0.3
   phases. Prerequisite: this spec ratified + bypass-resistance test list
   final.
2. **Measurement task** — confirm Rhai-built binary actually meets the size
   / cold-start success criteria.
3. **`exec_parallel` design (v0.2)** — error aggregation, cancellation,
   ordering, audit semantics.
4. **`--isolated` design (post-v0.1)** — run-cleanup, run-listing,
   run-resume semantics that come with per-run script isolation.
5. **MCP integration shape** — concrete protocol for `reeve-flex` invoked
   from an MCP server.
6. **Distribution channels** — Homebrew, cargo install, Docker image,
   pre-built binaries.

## Grill traceability (v3)

Decisions in this spec map back to grilled questions in this draft session:

| Q | Decision | Spec section |
|---|---|---|
| Q1 | `$HOME/.reeve/` (global per-user). Persona shift to "AI agent on bare VM." | Overview / Filesystem model |
| Q2 | Shared workspace + append-only; FileAlreadyExists is a feature. | Layer 1 |
| Q3 | Drop `inputs/`. | Filesystem layout |
| Q4 | JSON for `config.json`, YAML for `security.yaml` + pacts. | File-format split |
| Q5 | Scripts r/w `workspace/` only; `runs/<run-id>/` holds audit only. | Layout / `--isolated` future |
| Q6 | Drop `denied_roots`. | Filesystem model |
| Q7 | Keep `$CWD` in `allowed_roots`. | Layer 2 |
| Q8 | Full v2 host-fn set + new `opts.stdin` + `pipe()`. | Built-in host functions |
| Q8.2 | `pipe()` pipefail by default; `pipe_allow_fail()` for tolerant mode. | pipe() flow |
| Q9 | `env()` throws on deny (`EnvDenied` vs `EnvUnset`). | Built-in host functions |
| Q10 | `security.yaml` carry v2 forward minus dead fields; add `reeve_home`. | security.yaml |
| Q10.1 | Drop `max_script_seconds`. External `timeout` for total-run bound. | Concurrency & timeouts |
| Q11 | Drop `reeve init`. Lazy auto-init on first run. | CLI |
| Q12 | `config.json` keeps `max_workspace_bytes` + `auto_cleanup`. | Runtime config |
| Q13 | `reeve` ignores env; `reeve-flex` reads `REEVE_HOME` env. No CLI flag. | Distribution |
| Q14 | Keep `check` subcommand. | CLI |
| Q15 | Audit protection = Layer 1 + Layer 2 scope only (not OS-level). | Audit log protection |
| Q16 | Ship three presets: `core-tools` + `k8s-readonly` + `git-readonly`. Stackable. | Built-in Presets |
| Q17 | Workspace cap measured once at startup against total `<reeve_home>/`. | Runtime config |

Full grill log: this conversation thread (no separate file written).
