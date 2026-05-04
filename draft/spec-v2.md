# Warden — Design Spec (v2)

> Supersedes `draft/spec.md`. This revision folds in 15 grill decisions from
> `.chief/_grill/closed/0001-warden-design.md`. Phase plan deferred — captured
> separately after this spec is ratified.

## Overview

Warden is an allowlist-first shell automation runtime. It runs scripts written
in Rhai (an embedded scripting language), where every external command and
filesystem reference must pass an allowlist policy declared in a pact (YAML)
plus a security boundary declared in `security.yaml`.

Designed as a generic infrastructure tool: usable in CI/CD, runbooks,
automation pipelines, or as an execution layer for AI agents — all four
treated as co-equal first-class users.

> Warden is part of the chief-tribe ecosystem as a generic execution layer —
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
- **Embeddable** — callable from subprocess, MCP server, or CI runner directly
  via the `warden-flex` binary.

## Non-goals

- Replacing bash for interactive use.
- Full POSIX shell compatibility.
- Kernel-level sandbox (network/syscall isolation) — use bwrap/firejail on top
  if needed.
- Remote pact loading.
- Multi-language support — Rhai only.

## Why Rhai

Rhai chosen over Lua, Starlark, and JavaScript after weighing:

**Rhai wins:**
- **Pure Rust** — no C dependency. Smaller static binary, easier
  cross-compilation, no C runtime concerns. (mlua/Lua require linking the C
  Lua runtime; QuickJS adds a sizeable C engine.)
- **Sandbox by default** — no stdlib to strip. Lua and JS require manual
  removal of `os`/`io`/`debug`/`require`/`load`/etc.
- **First-class resource limits** — `set_max_operations`,
  `set_max_call_stack_depth`, `set_max_string_size`, `set_max_array_size`.
- **`disable_symbol()`** — surgical disabling of `eval`, etc.
- **Type-safe Rust↔script FFI** — `engine.register_fn` is checked at compile
  time on both sides.

**Trade-offs accepted:**
- Smaller LLM training corpus than Lua/Python/JS. Mitigated by shipping
  `SKILL.md` + host-fn documentation (custom host fns dominate corpus
  considerations anyway).
- Less engineer familiarity than Lua. Mitigated by JavaScript-like syntax.

**Considered alternatives:**
- **Lua (mlua):** rejected — C dependency, manual sandbox setup.
- **Starlark (starlark-rust):** viable; would gain deterministic
  review-and-replay properties; loses `while`/recursion/exception ergonomics.
  Reconsider if determinism becomes a goal.
- **JavaScript (QuickJS/boa):** rejected — heavier engine, larger attack
  surface, larger binary.

## Distribution: two binaries

Two CLI binaries ship from the same workspace, with different trust models.

### `warden` — for AI direct use

- Trust boundary: the binary itself.
- Pacts, `security.yaml`, runtime config, all compile-time embedded
  (`include_str!`).
- **No** `--pact`, `--pact-stdin`, `--config`, `--allow-preset` flags. AI
  agents control argv; runtime flags would be bypass.
- All embedded presets active for every run.
- Customize → fork repo → edit YAML → `cargo build --release`.

### `warden-flex` — for trusted callers (MCP servers, CI orchestrators, multi-tenant platforms)

- Trust boundary: the caller that invokes `warden-flex`.
- Pacts and `security.yaml` compile-time embedded (security-critical).
- Runtime config provided by caller via `--config`.
- Caller may pass `--pact <file>`, `--pact-stdin`, `--allow-preset <name>` to
  scope each run.

### Why two binaries (not one with flags)

A single binary with `--pact` exposes runtime-pact override. AI agents
calling that binary can pass `--pact /tmp/evil.yaml` and bypass the
allowlist. "Agent doesn't know about the flag" = security through
obscurity. Capability separation moves the trust boundary into the artifact
itself.

| | `warden` | `warden-flex` |
|---|---|---|
| `run script.rhai` | ✅ | ✅ |
| `check script.rhai` | ✅ | ✅ |
| `preset list`, `preset show <name>` | ✅ | ✅ |
| `version` | ✅ | ✅ |
| `init` (scaffold project) | ✅ | ❌ |
| `--allow-preset` | ❌ | ✅ |
| `--pact <file>` | ❌ | ✅ |
| `--pact-stdin` | ❌ | ✅ |
| `--config <file>` | ❌ | ✅ |
| Trust boundary | Binary | Caller |
| Default consumer | AI direct | MCP/CI/platform |

## Architecture

```
┌──────────────────────────────────────────────────┐
│  warden / warden-flex (CLI binaries)              │
│                                                   │
│  ┌─────────────────────────────────────────────┐ │
│  │ CLI layer (clap)                             │ │
│  │  run / check / preset list / preset show /   │ │
│  │  init (warden only) / version                │ │
│  └────────────────┬────────────────────────────┘ │
│                   ▼                               │
│  ┌─────────────────────────────────────────────┐ │
│  │ Config loader                                │ │
│  │  - Compiled-in security.yaml (both)          │ │
│  │  - Compiled-in pacts                         │ │
│  │  - Runtime config: compiled-in (warden) /    │ │
│  │    --config (warden-flex)                    │ │
│  └────────────────┬────────────────────────────┘ │
│                   ▼                               │
│  ┌─────────────────────────────────────────────┐ │
│  │ Pact engine (warden-pact)                    │ │
│  │  - Parse YAML → policy struct                │ │
│  │  - Generic allowlist validator               │ │
│  │  - Named-kinds dispatch                      │ │
│  │  - Custom Rust validator escape hatch        │ │
│  └────────────────┬────────────────────────────┘ │
│                   ▼                               │
│  ┌─────────────────────────────────────────────┐ │
│  │ Rhai engine (warden-core)                    │ │
│  │  - Resource limits                           │ │
│  │  - Disabled symbols (eval, modules)          │ │
│  │  - Registered host functions                 │ │
│  └────────────────┬────────────────────────────┘ │
│                   ▼                               │
│  ┌─────────────────────────────────────────────┐ │
│  │ Executor                                     │ │
│  │  - Validate exec() against pact + kinds      │ │
│  │  - Spawn (shell:false, argv array)           │ │
│  │  - Apply timeout (per-exec + script-total)   │ │
│  │  - Apply output cap                          │ │
│  │  - Stream stdout_to into .warden/<run-id>/   │ │
│  │  - Emit audit JSONL events                   │ │
│  │  - Throw on failure (default)                │ │
│  └─────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────┘
```

## Pact (YAML)

A pact is an allowlist policy file. **Pure allowlist semantics — no
`forbidden_*` keys.** Every flag, subcommand, and positional argument must be
explicitly allowed. Anything not declared is rejected.

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
      KUBECONFIG: /etc/warden/readonly-kube/config
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
      logs:
        allowed_flags: [-n, --namespace, --tail, -f, --since, --container]
        flag_values:
          "-n": { kind: k8s_namespace }
          "--namespace": { kind: k8s_namespace }
          "--tail": { kind: number }
          "--since": { kind: duration }
        positional:
          - { kind: k8s_name }
      describe:
        allowed_flags: [-n, --namespace]
        positional:
          - { kind: enum, values: [pods, deployments, services, nodes] }
          - { kind: k8s_name }
```

### Built-in named kinds

Implemented in Rust once, reused across pacts.

- `enum` — `{ kind: enum, values: [...] }`. Exact-match against value list.
- `number` — non-negative integer.
- `duration` — Go-style duration string (`30s`, `5m`, `1h`).
- `filepath` — path argument validated against Layer 2 scope (working_dir +
  allowed_roots). Symlinks resolved; traversal rejected.
- `filepath_existing` — like `filepath` but the path must exist.
- `directory` — like `filepath` but must be a directory.
- `glob_pattern` — shell glob pattern; expanded against Layer 2 scope.
- `k8s_name` — RFC 1123 label/subdomain; max 253 chars.
- `k8s_namespace` — RFC 1123 label; max 63 chars.
- `k8s_selector` — label-selector expression (no shell metacharacters).
- *(extensible — new kinds added in `warden-pact/src/kinds/`)*

### Custom validator escape hatch

For DSL-shaped flag values that the schema can't express:

```yaml
flag_values:
  "-o":
    kind: custom
    name: kubectl_output_flag    # → fn validate_kubectl_output_flag(s: &str) -> Result<()>
```

`name` resolves to a Rust function in `warden-pact/src/custom/`. Adding one
requires a code PR (intentional friction).

### Risk categories (documentation, not enforcement)

Each shipped preset declares a category in its README:

- **PureRead** — no side effects (e.g., `kubectl get`, `cat`, `ls`).
- **Standard** — read-side-effecty but bounded (`kubectl logs`).
- **RequiresAudit** — has dangerous flags omitted from allowlist
  (e.g., `find` without `-exec`/`-delete`).
- **Forbidden** — never shipped (`kubectl apply`, `rm`).

## `security.yaml`

Compile-time embedded for both binaries. Changes require recompile —
overriding security config = breaking the security boundary.

```yaml
# security.yaml
working_dir: "$CWD"             # base of run; default = current dir at invocation
allowed_roots: []               # extra paths exec() filepath args may reference
deny_traversal: true            # reject "../" in any path

env_passthrough: [PATH, HOME, LANG]

max_script_seconds: 300         # script-total timeout

audit:
  capture_command: true         # log binary + argv + exit_code + duration
  capture_stdout: false         # log child stdout content (PII risk)
  capture_stderr: false         # log child stderr content (PII risk)
  sink_path: ".warden/<run-id>/audit.jsonl"   # default; override here for centralized logging

workspace_mode: per_run         # per_run | shared
```

## Runtime config

Operational tuning. For `warden`: compiled-in. For `warden-flex`: provided by
caller via `--config <file>`. Overriding these can degrade operations but
cannot break the security boundary.

```yaml
# runtime.yaml (warden-flex) or compiled-in defaults (warden)
workspace:
  max_workspace_bytes: 1073741824     # 1 GiB cap on total .warden/ size
  auto_cleanup:
    enabled: false                    # default off
    target_percent: 80                # cleanup until ≤ 80% of cap
                                      # strategy: oldest_first (hardcoded)
                                      # never touches current run or .warden/inputs/
                                      # cleanups logged to audit
```

## Filesystem model

Two non-overlapping FS layers.

### Layout

```
$CWD/                          ← working_dir
└── .warden/
    ├── inputs/                ← operator stages input files here (read-only to scripts)
    │   └── data.json
    └── <run-id>/              ← per-run dir (script writes go here)
        ├── audit.jsonl
        └── output.json
```

`<run-id>` is a UUID or timestamp+sha generated per `warden run`. Old runs
persist for forensics; new runs never collide.

### Layer 1 — Built-in FS host functions

Scoped to `.warden/` only. Append-only semantics for all writes.

```rhai
// Read (.warden/inputs/ + .warden/<run-id>/)
read_file(path) -> string
read_lines(path) -> array<string>
exists(path) -> bool
glob(pattern) -> array<string>

// Write (.warden/<run-id>/ only)
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

**Read collision** — same filename in `.warden/inputs/` and
`.warden/<run-id>/` → throw `AmbiguousPath` (operator/AI must rename).

### Layer 2 — Exec filepath arguments

Paths flowing into `exec()` via `kind: filepath` (or its variants) are
validated against `working_dir + allowed_roots`. Symlinks resolved.

This is how scripts reach files **outside** `.warden/` — by running an
allowlisted binary (`cat`, `tee`, etc.) whose pact accepts a `filepath`
argument.

### Why two layers

- Layer 1 is for **script's own state** — small, sandboxed, append-only.
- Layer 2 is for **invoking external binaries** that legitimately need
  filesystem access — mediated by the pact, validated against operator-set
  roots.

### Bypass resistance

- Rhai has no FS by default.
- Warden does NOT register `rhai-fs` (community FS module).
- Only the host fns above are registered.
- `engine.set_max_modules(0)` disables `import`/`require`.
- `engine.disable_symbol("eval")` disables `eval`.
- Direct Rhai I/O attempts (`file_open`, `fs::read`) → function not found.

### Cleanup policy

Warden never deletes files. Operator manages `.warden/` lifecycle:

- Manual: `rm -rf .warden/<old-run-id>/` between runs.
- Automatic: enable `workspace.auto_cleanup` in runtime config.

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
//   stdout_to: <relative path>   // stream stdout to .warden/<run-id>/<path>
//                                // file must not pre-exist (write_file rules)
//                                // exits in-memory cap mode
//   timeout_seconds: <int>       // per-exec override (must be ≤ pact default)

exec_allow_fail(binary: string, args: array, opts?: map) -> map
// never throws on non-zero exit; caller inspects exit_code
// Timeout / OutputLimitExceeded still throw

// Data parsing
parse_json(s: string) -> dynamic
parse_yaml(s: string) -> dynamic
parse_toml(s: string) -> dynamic
to_json(v: dynamic) -> string

// Filesystem (Layer 1 only — see Filesystem model)
read_file(path: string) -> string
read_lines(path: string) -> array<string>
exists(path: string) -> bool
glob(pattern: string) -> array<string>
write_file(path: string, content: string)
append_file(path: string, content: string)

// Environment & arguments
env(key: string) -> string
// allowed keys (security.yaml.env_passthrough): returns value
// denied keys: emits stderr warning + returns ""
script_args() -> array
// raw CLI args after script path; validated by pact when passed to exec()

// Output
print(...)
log_info(msg: string)
log_warn(msg: string)
log_error(msg: string)
```

### Rhai language posture

- Standard: `let`, `for`, `if/else`, `while`, `fn`, array, map, string template.
- Disabled: `eval`, module loading (`import`/`require`).
- Resource limits (Rhai engine API names):
  - `set_max_operations(1_000_000)`
  - `set_max_call_stack_depth(32)`
  - `set_max_string_size(102_400)` — 100 KiB
  - `set_max_array_size(10_000)`
  - `set_max_modules(0)`

### Concurrency

Rhai is single-threaded. v0.1 supports sequential `exec()` only. Callers
needing parallelism do orchestrator-level fan-out (multiple `warden`
invocations).

`exec_parallel(jobs: array) -> array<map>` planned for v0.2 (engine fans out
up to `max_parallel` from runtime config).

### Streaming/follow-mode anti-pattern

Binaries like `kubectl logs -f`, `tail -f`, `kubectl get --watch` conflict
with the buffered exec model and the script-total timeout. Don't allowlist
them. If long-tail observation is needed, run an external watcher and
invoke Warden per snapshot.

### Example script

```rhai
// investigate-failing-pods.rhai

// Throws on non-zero exit, on timeout, on output overflow.
let result = exec("kubectl", ["get", "pods", "-n", "prod", "-o", "json"]);

let pods = parse_json(result.stdout);
let failing = [];

for pod in pods.items {
    if pod.status.phase != "Running" {
        failing.push(pod.metadata.name);
    }
}

print(`Found ${failing.len()} failing pods`);

// Persist a structured report (Layer 1 — write to .warden/<run-id>/).
write_file("failing-pods.json", to_json(failing));

for name in failing {
    print(`\n=== ${name} ===`);
    // For pods that may have been deleted between the get and the logs call,
    // tolerate the failure explicitly.
    let logs = exec_allow_fail("kubectl", ["logs", name, "-n", "prod", "--tail=50"]);
    if logs.exit_code == 0 {
        append_file("pod-logs.txt", `\n=== ${name} ===\n${logs.stdout}`);
    } else {
        log_warn(`logs failed for ${name}: ${logs.stderr}`);
    }
}
```

## Executor — `exec()` flow

```
exec(binary, args, opts) flow:
  1. Look up binary in active pacts → not found → throw BinaryNotAllowed
  2. Validate first positional as subcommand (if pact requires) →
     not in allowed_subcommands → throw SubcommandNotAllowed
  3. Validate flags (name in allowed_flags) → throw FlagNotAllowed
  4. Validate flag values (kind dispatch) → throw FlagValueRejected
  5. Validate positionals (kind dispatch) → throw PositionalRejected
  6. If filepath kind appears → resolve symlinks, check Layer 2 scope
  7. Build Command:
     - absolute path from pact
     - argv array (no shell:true)
     - env_clear() then set security.yaml.env_passthrough + pact.env_overrides
  8. Spawn with per-exec timeout (pact default or opts override)
  9. Capture stdout/stderr (size-limited to max_output_bytes) OR stream to
     .warden/<run-id>/<stdout_to> if opts.stdout_to set
 10. On overflow → kill child, throw OutputLimitExceeded
 11. On per-exec timeout → kill child, throw Timeout
 12. On non-zero exit → throw ExecFailed (unless caller used exec_allow_fail)
 13. Emit audit events (exec_start, exec_end, optional stdout/stderr captures)
 14. Return { stdout, stderr, exit_code, duration_ms }
```

Engine also enforces `security.yaml.max_script_seconds` across the whole run
→ on overflow, kill any in-flight child + throw `ScriptTimeout`.

## Audit log

Always-on. JSONL at `.warden/<run-id>/audit.jsonl` (overridable via
`security.yaml.audit.sink_path`).

### Event schema

```jsonl
{"event":"script_start","ts":"2026-05-04T10:30:00Z","run_id":"abc123","script_path":"/path/to/script.rhai","script_sha256":"...","args":["prod"],"presets":["k8s-readonly"]}
{"event":"exec_start","ts":"...","binary":"kubectl","argv":["get","pods","-n","prod","-o","json"]}
{"event":"exec_end","ts":"...","binary":"kubectl","exit_code":0,"duration_ms":234,"stdout_bytes":12044,"stderr_bytes":0}
{"event":"stdout","ts":"...","binary":"kubectl","content":"..."}    // only if audit.capture_stdout
{"event":"stderr","ts":"...","binary":"kubectl","content":"..."}    // only if audit.capture_stderr
{"event":"exec_error","ts":"...","binary":"kubectl","kind":"Timeout","limit_ms":30000}
{"event":"script_end","ts":"...","exit_status":"ok","duration_ms":1234,"exec_count":5}
```

`audit.capture_command` is always true (logs metadata). `capture_stdout` and
`capture_stderr` are off by default; flipping them captures full child output
into the audit log (PII risk — operator decides at compile time).

The audit file is written by Warden, not by the script — it does not count
against `max_workspace_bytes`.

### Forensics bundle

Zip `.warden/<run-id>/` and you have: script artifacts (script outputs
written via Layer 1), audit log, optional captured output streams. Plus the
script itself (referenced via `script_path` and `script_sha256`).

## CLI

### `warden`

```bash
warden run <script.rhai>
warden check <script.rhai>          # static validation; no exec
warden preset list
warden preset show <name>
warden init                          # scaffold .warden/ + example script
warden version
```

Flags:
- `--timeout <seconds>` — per-exec timeout override (cannot exceed pact
  default).
- `--quiet` — suppress logs except errors.
- `--json` — output structured JSON instead of text.
- `--workspace <dir>` — override `working_dir` (default `$CWD`).

### `warden-flex`

Same subcommands as `warden` (except `init`), plus:

```bash
warden-flex run <script> --allow-preset <name>
warden-flex run <script> --pact <file>
warden-flex run --pact-stdin --script-stdin < bundle.json
warden-flex run <script> --config <runtime-config.yaml>
```

### Exit codes

- `0` — success.
- `1` — script error (Rhai runtime, exec failure surfaced as exception).
- `2` — pact violation (binary/flag/value not allowed).
- `3` — configuration error (pact parse error, missing preset, security.yaml
  invalid).
- `4` — timeout (per-exec or script-total).
- `5` — workspace quota exceeded.

## Built-in Presets (v0.1)

- `k8s-readonly` — kubectl get/describe/logs/top/events/version, jq, plus
  Layer 2 helpers (`cat`, `grep`).
- `git-readonly` — git log/show/diff/status/blame/ls-files.

Future: `db-readonly`, `azure-readonly`, `aws-readonly`, `devops-basic`.

## Security model

### Defense layers

| Layer | Mechanism |
|---|---|
| Language | Rhai — no eval, no import, no reflection, no FFI |
| Engine | Resource limits (operations/stack/string/array/modules) |
| Host fn | Validate args, no shell layer, env_clear + allowlist |
| Pact | Pure allowlist of binaries + subcommands + flags + values |
| Filesystem | Two-layer model; Layer 1 sandboxed to .warden/; Layer 2 mediated by pact `kind: filepath` |
| Pact source | Compile-time embedded, immutable at runtime (warden) |
| Audit | Always-on JSONL; metadata always captured |
| External system | (User responsibility) Use scoped credentials |

### Threat model — mitigated

- **Script injection via crafted input** — Rhai parses, no shell layer.
- **Binary substitution** — absolute path required in pact.
- **Flag injection (`--kubeconfig=evil`)** — pure allowlist; flags not
  declared are rejected.
- **DSL injection in flag values (`-o jsonpath=...`)** — `flag_values`
  declares per-flag kinds; custom validator escape hatch handles DSLs.
- **Resource exhaustion (CPU/memory/output)** — engine limits + per-exec +
  per-run timeouts + output size cap + workspace size cap.
- **Pact tampering at runtime (`warden`)** — embedded at compile time; no
  `--pact` flag.
- **AI agent generating malicious scripts** — bound by pact + Layer 1 FS
  sandbox + audit log.
- **Filesystem exfiltration** — Layer 1 writes confined to
  `.warden/<run-id>/`; Layer 2 reads bounded by `allowed_roots`; no
  `read_file` host fn for arbitrary paths.
- **Cross-run tampering** — append-only Layer 1 + per-run dir isolation.

### Threat model — out of scope

- Bugs in host function code (responsibility of contributors / fuzz tests).
- Kernel-level escape — use bwrap/firejail externally.
- Network exfiltration — use OS-level network policy.
- Side channels (timing, memory pressure).
- ReDoS from operator-authored regex in pact `flag_values` — operator
  responsibility.

### Custom pact path (warden-flex only)

```bash
warden-flex run script.rhai --pact ./custom.yaml
# stderr: WARN: using custom pact, not a built-in preset
```

This is **only** in `warden-flex`. The trust assumption is that the caller
spawning `warden-flex` is already trusted (MCP server, CI orchestrator). AI
agents calling `warden` directly cannot reach this path.

## Repository structure

```
warden/
├── Cargo.toml                          # workspace root
├── README.md
├── LICENSE
│
├── crates/
│   ├── warden-core/                    # Rhai engine + executor + audit + FS host fns
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs               # Rhai engine setup + resource limits
│   │       ├── executor.rs             # exec() host fn + flow
│   │       ├── fs.rs                   # Layer 1 FS host fns
│   │       ├── audit.rs                # JSONL emitter
│   │       ├── timeout.rs              # per-exec + script-total timers
│   │       └── workspace.rs            # .warden/ + run-id management
│   │
│   ├── warden-pact/                    # YAML schema + allowlist engine
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schema.rs               # serde structs (pact + security.yaml + runtime)
│   │       ├── engine.rs               # generic allowlist validator
│   │       ├── kinds/                  # built-in named kinds
│   │       │   ├── mod.rs
│   │       │   ├── filepath.rs
│   │       │   ├── enum_kind.rs
│   │       │   ├── number.rs
│   │       │   ├── duration.rs
│   │       │   └── k8s.rs
│   │       ├── custom/                 # custom validator escape hatches
│   │       │   ├── mod.rs
│   │       │   └── kubectl.rs
│   │       └── presets.rs              # include_str! embed
│   │
│   ├── warden/                         # CLI 1: AI direct use
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   │
│   └── warden-flex/                    # CLI 2: trusted callers
│       ├── Cargo.toml
│       └── src/main.rs
│
├── pacts/                              # built-in presets
│   ├── k8s-readonly.yaml
│   └── git-readonly.yaml
│
├── security.yaml                       # default security boundary (embedded)
├── runtime.yaml                        # default runtime config (embedded into warden)
│
├── examples/
│   ├── investigate-pods.rhai
│   └── git-history.rhai
│
└── tests/
    ├── integration/
    │   ├── pact_engine.rs
    │   ├── fs_layer1.rs                # bypass-resistance suite
    │   └── audit.rs
    └── fixtures/
```

## Dependencies

```toml
# warden-core
[dependencies]
rhai = { version = "1.19", features = ["sync"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
anyhow = "1"
thiserror = "1"
regex = "1"
uuid = { version = "1", features = ["v4"] }
chrono = "0.4"

# warden-pact
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
regex = "1"
thiserror = "1"

# warden + warden-flex
[dependencies]
warden-core = { path = "../warden-core" }
warden-pact = { path = "../warden-pact" }
clap = { version = "4", features = ["derive"] }
```

## CI test suite (required before v0.1 ship)

Bypass-resistance suite — every item must be a passing test:

- `read_file("/etc/passwd")` → reject (outside `.warden/`).
- `read_file("../../etc/passwd")` → reject (Layer 1 + traversal).
- `read_file("/workspace/symlink-to-secret")` → reject (symlink resolved).
- `write_file` to existing path → reject `FileAlreadyExists`.
- `append_file` outside `.warden/<run-id>/` → reject.
- `import "fs"` / `require("os")` → reject (modules disabled).
- `eval("...")` → reject (symbol disabled).
- Direct Rhai I/O attempts (`file_open`, `fs::read`) → function not found.
- `exec("rm", ["-rf", "/"])` → reject (binary not in pact).
- `exec("kubectl", ["apply", ...])` → reject (subcommand not allowed).
- `exec("kubectl", ["get", "pods", "--kubeconfig=/tmp/x"])` → reject (flag
  not allowed).
- `exec("kubectl", ["get", "secrets"])` → reject (positional not in enum).
- `exec(...)` exceeding per-exec timeout → throw `Timeout`.
- Script exceeding `max_script_seconds` → throw `ScriptTimeout`.
- `exec(...)` exceeding `max_output_bytes` → throw `OutputLimitExceeded`.
- Workspace exceeding `max_workspace_bytes` → throw `WorkspaceQuotaExceeded`.
- `env("AWS_SECRET_ACCESS_KEY")` → stderr warning + return `""`.
- `--pact` flag on `warden` binary → CLI parse error (unknown flag).

## Success criteria

- v0.1 binaries: `warden` < 10 MB, `warden-flex` < 10 MB, cold start < 50ms
  (verify with measurement task before declaring v0.1 ready).
- All bypass-resistance tests pass.
- Two presets (`k8s-readonly`, `git-readonly`) with PureRead and Standard
  examples.
- README + examples sufficient for adoption within 30 minutes.
- Test coverage > 80% in `warden-core` and `warden-pact`.

## Followups (post-grill)

1. **Phase-cutting** — slice this spec into shippable v0.1 / v0.2 / v0.3
   phases. Prerequisite: this spec ratified + bypass-resistance test list
   final.
2. **Measurement task** — confirm Rhai-built binary actually meets the size /
   cold-start success criteria.
3. **`exec_parallel` design (v0.2)** — error aggregation, cancellation,
   ordering, audit semantics.
4. **MCP integration shape** — concrete protocol for `warden-flex` invoked
   from an MCP server; informs whether sequential-only Q12 holds.
5. **Distribution channels** — Homebrew, cargo install, Docker image,
   pre-built binaries.

## Grill traceability

Decisions in this spec map back to grilled questions:

| Q | Decision | Spec section |
|---|---|---|
| Q1 | Co-equal use cases | Overview |
| Q2 | Operator-only pacts (warden) | Distribution / Security model |
| Q3-revised | YAML allowlist engine + named kinds + custom escape | Pact / Built-in named kinds |
| Q4 | Rhai | Why Rhai |
| Q5-revised + Q15 | Two-layer FS | Filesystem model |
| Q6 | exec() throws default | Built-in host functions |
| Q7 | Two binaries | Distribution / CLI |
| Q8 + Q14-#3,#4 | Always-on JSONL audit | Audit log |
| Q9 + sidebar | env allowlist + raw script_args | Built-in host fns / security.yaml |
| Q10 | Throw on overflow + stdout_to | Executor flow |
| Q11 | Defer phase-cutting | Followups |
| Q12 | Sequential v0.1 | Concurrency |
| Q13 | Per-exec + script-total timeouts | security.yaml / Executor flow |
| Q14-#1 (revised by Q15) | Per-run isolation via .warden/<run-id>/ | Filesystem model |
| Q14-#2 (revised by Q15) | max_workspace_bytes runtime cap | Runtime config |
| Q14-#5 | Env-deny audit rejected | Built-in host fns |
| Q14-#6 | CLI symmetry | CLI |
| Q15 | Config split (security vs runtime) | security.yaml / Runtime config |

Full grill log: `.chief/_grill/closed/0001-warden-design.md`.
