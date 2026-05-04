# Reeve — Design Spec

## Overview

Reeve is an allowlist-first shell automation runtime. It runs scripts written in Rhai (an embedded scripting language), where every external command must be declared in a pact (YAML allowlist) before execution.

Designed as a generic infrastructure tool: usable in CI/CD, runbooks, automation pipelines, or as an execution layer for AI agents.

> Reeve is part of the chief-tribe ecosystem as a generic execution layer — usable standalone or paired with other agents (Sage, Chief, Chieftain, Council).

## Goals

- **Clear safety boundary** — scripts can only do what the pact permits; no shell injection, no arbitrary binary execution
- **Full logic flow** — for, if, let, function, array, map (via Rhai), not just declarative
- **Lightweight** — single binary, no daemon, no runtime dependency
- **Auditable** — both pact and script are text files; reviewable and version-controllable
- **Embeddable** — callable from subprocess, MCP server, or CI runner directly

## Non-goals

- Replacing bash for interactive use
- Full POSIX shell compatibility
- Kernel-level sandbox (network/syscall isolation) — use bwrap/firejail on top if needed
- Remote pact loading in v1
- Multi-language support — Rhai only

## Architecture

```
┌─────────────────────────────────────────────┐
│  reeve CLI (single binary, Rust)            │
│                                              │
│  ┌────────────────────────────────────────┐  │
│  │ CLI layer (clap)                        │  │
│  │  run / check / list-presets / show      │  │
│  └────────────────┬───────────────────────┘  │
│                   ▼                          │
│  ┌────────────────────────────────────────┐  │
│  │ Pact loader                             │  │
│  │  - Load preset (compile-time embed)     │  │
│  │  - Or load custom (--pact flag)         │  │
│  │  - Parse YAML → policy struct           │  │
│  └────────────────┬───────────────────────┘  │
│                   ▼                          │
│  ┌────────────────────────────────────────┐  │
│  │ Rhai engine                             │  │
│  │  - Resource limits                      │  │
│  │  - Disabled symbols (eval, etc.)        │  │
│  │  - Registered host functions            │  │
│  └────────────────┬───────────────────────┘  │
│                   ▼                          │
│  ┌────────────────────────────────────────┐  │
│  │ Executor                                │  │
│  │  - Validate exec() against pact         │  │
│  │  - Spawn (shell:false, argv array)      │  │
│  │  - Apply timeout, output limit          │  │
│  │  - Return structured result             │  │
│  └────────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

## Components

### Component 1: Pact (YAML)

A pact is an allowlist policy file that declares what a script is permitted to run.

**Schema (v1):**

```yaml
version: 1
name: k8s-readonly
description: Read-only Kubernetes investigation

defaults:
  timeout_seconds: 30
  max_output_bytes: 10485760    # 10 MiB
  env_passthrough: [PATH, HOME, LANG]

binaries:
  kubectl:
    path: /usr/bin/kubectl
    allowed_subcommands: [get, describe, logs, top, events, version]
    forbidden_flags:
      exact: ["--kubeconfig", "--token", "--server", "--as", "--raw"]
      patterns: ["^-o(=|$)?jsonpath"]
    env_overrides:
      KUBECONFIG: /etc/reeve/readonly-kube/config
  
  jq:
    path: /usr/bin/jq
    forbidden_flags:
      exact: ["-f", "--from-file"]
  
  git:
    path: /usr/bin/git
    allowed_subcommands: [log, show, diff, status, blame, ls-files]
```

**Validation rules:**
- Binary path must be absolute
- Subcommands match by exact string (case-sensitive)
- Flag check: exact match or regex pattern
- Argument values pass through built-in validators (path traversal, shell metacharacters)

### Component 2: Script (Rhai)

A `.rhai` text file using standard Rhai syntax.

**Built-in functions registered by Reeve:**

```rhai
// Process execution
exec(binary: string, args: array) -> map
// returns: { stdout: string, stderr: string, exit_code: int }

exec_or_fail(binary: string, args: array) -> map
// throws if exit_code != 0

// Data parsing
parse_json(s: string) -> dynamic
parse_yaml(s: string) -> dynamic
parse_toml(s: string) -> dynamic
to_json(v: dynamic) -> string

// Filesystem (allowlisted via pact)
read_file(path: string) -> string
glob(pattern: string) -> array

// Environment & arguments
env(key: string) -> string        // allowlisted vars only
script_args() -> array            // CLI args after script path

// Output
print(...)
log_info(msg: string)
log_warn(msg: string)
log_error(msg: string)
```

**Rhai language features:**
- Standard: let, for, if/else, while, fn, array, map, string template
- Disabled by config: `eval`, module loading
- Resource limits: max_operations=1M, max_call_levels=32, max_string=100KB

**Script example:**

```rhai
// investigate-failing-pods.rhai
let result = exec("kubectl", ["get", "pods", "-n", "prod", "-o", "json"]);
if result.exit_code != 0 {
    log_error(`kubectl failed: ${result.stderr}`);
    return;
}

let pods = parse_json(result.stdout);
let failing = [];

for pod in pods.items {
    if pod.status.phase != "Running" {
        failing.push(pod.metadata.name);
    }
}

print(`Found ${failing.len()} failing pods`);

for name in failing {
    print(`\n=== ${name} ===`);
    let logs = exec("kubectl", ["logs", name, "-n", "prod", "--tail=50"]);
    print(logs.stdout);
}
```

### Component 3: Executor

Rust code that implements the `exec()` host function:

```
exec(binary, args) flow:
  1. Look up binary in pact
     - if not found → throw "binary not allowed"
  2. Validate first arg as subcommand (if pact requires)
     - if not in allowed_subcommands → throw
  3. Validate flags
     - check forbidden_flags.exact and .patterns
  4. Validate argument values
     - check for path traversal, null bytes, shell metacharacters
  5. Build Command
     - absolute path from pact
     - argv array (no shell:true)
     - env_clear() then set allowed env + overrides
  6. Spawn with timeout
     - capture stdout/stderr (size-limited)
  7. Return { stdout, stderr, exit_code }
```

## CLI Design

```bash
reeve run <script.rhai> [--preset <name> | --pact <file>]
reeve check <script.rhai> [--preset <name> | --pact <file>]
reeve list-presets
reeve show-preset <name>
reeve init                     # scaffold project structure
reeve version
```

**Flags:**
- `--preset <name>` — use a built-in preset (one of preset/pact required)
- `--pact <file>` — use a custom pact file (warning printed)
- `--timeout <seconds>` — override default timeout
- `--quiet` — suppress logs except errors
- `--json` — output structured JSON instead of text

**Exit codes:**
- 0: success
- 1: script error (Rhai runtime, exec failure)
- 2: pact violation (binary/flag not allowed)
- 3: configuration error (pact parse error, missing preset)

## Built-in Presets (v0.1)

Ship two presets initially:

**1. `k8s-readonly`** — kubectl get/describe/logs/top + jq

**2. `git-readonly`** — git log/show/diff/status/blame

Future additions: `db-readonly`, `azure-readonly`, `aws-readonly`, `devops-basic`

## Security Model

### Defense layers

| Layer | Mechanism |
|---|---|
| Language | Rhai — no eval, no import, no reflection, no FFI |
| Engine | Resource limits (operations/stack/memory/string/array) |
| Host fn | Validate args, no shell layer, env_clear |
| Pact | Allowlist binaries + subcommands + flags |
| Pact source | Compile-time embed, immutable at runtime |
| External system | (User responsibility) Use scoped credentials |

### Threat model

**Mitigated:**
- Script injection via crafted input — Rhai parses, no shell layer
- Binary substitution — absolute path required in pact
- Flag injection (e.g., `--kubeconfig=evil`) — forbidden_flags check
- Resource exhaustion — engine limits + per-call timeout
- Pact tampering at runtime — embedded at compile time
- AI agent generating malicious scripts — bound by pact

**Out of scope:**
- Bugs in host function code — user/contributor responsibility
- Kernel-level escape — use bwrap/firejail externally
- Network exfiltration — use OS-level network policy
- Side channels (e.g., timing) — not addressed

### Custom pact escape hatch

```bash
reeve run script.rhai --pact ./custom.yaml
# stderr: WARN: using custom pact, not a built-in preset
```

Users who need a new pact should fork the repo, add a preset, and recompile.

This is exposed as a `--pact` flag because:
- Friction signals that the user is leaving the safe path
- Custom pacts share the same trust level — user is responsible

## Repository Structure

```
reeve/
├── Cargo.toml                        # workspace root
├── README.md
├── LICENSE
│
├── crates/
│   ├── reeve-core/                  # Rhai engine + executor
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs             # Rhai engine setup
│   │       ├── executor.rs           # exec() host fn
│   │       ├── validator.rs          # pact validation logic
│   │       └── builtins.rs           # parse_json, glob, etc.
│   │
│   ├── reeve-pact/                  # YAML schema
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schema.rs             # serde structs
│   │       └── presets.rs            # include_str! embed
│   │
│   └── reeve-cli/                   # CLI binary
│       ├── Cargo.toml
│       └── src/
│           └── main.rs               # clap + dispatch
│
├── pacts/                            # built-in presets
│   ├── k8s-readonly.yaml
│   └── git-readonly.yaml
│
├── examples/
│   ├── investigate-pods.rhai
│   └── git-history.rhai
│
└── tests/
    ├── integration/
    └── fixtures/
```

## Dependencies

```toml
# reeve-core
[dependencies]
rhai = { version = "1.19", features = ["sync"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
anyhow = "1"
thiserror = "1"
regex = "1"

# reeve-cli
[dependencies]
reeve-core = { path = "../reeve-core" }
reeve-pact = { path = "../reeve-pact" }
clap = { version = "4", features = ["derive"] }
```

## Implementation Phases

### Phase 1: Core MVP (week 1-2)

- Rhai engine setup with resource limits
- Pact schema + YAML parser
- `exec()` host function (basic validation)
- CLI: `run` command with `--preset`
- One preset: `k8s-readonly`
- Basic integration tests

### Phase 2: Hardening (week 3)

- Forbidden flags (exact + patterns)
- Argument value validation (path traversal, etc.)
- Timeout, output size limit
- Output redaction (basic secret patterns)
- `check` command (dry-run validation)

### Phase 3: Polish (week 4)

- Second preset: `git-readonly`
- Built-in helpers: `parse_json`, `parse_yaml`, `glob`, `read_file`
- `list-presets`, `show-preset` commands
- README with examples
- CI/CD setup, release workflow

### Phase 4: Distribution (post-v0.1)

- Homebrew formula
- Cargo install support
- Docker image
- Pre-built binaries (Linux, macOS, Windows)

## Open Questions

1. **Output redaction policy** — built-in regex set vs configurable per pact?
2. **Concurrent exec** — Rhai is single-threaded; do we need a parallel exec helper?
3. **State across runs** — should scripts persist state (cache/scratch dir) after exit?
4. **Logging format** — text vs JSON? structured logging library?
5. **Preset discovery** — vendor preset (shipped in binary) only, or also support user-level config dir (`~/.reeve/pacts/`)?

## Success Criteria

- v0.1 ship: reeve binary < 10 MB, cold start < 50ms
- All presets pass basic security tests (injection attempts blocked)
- README + examples sufficient for adoption within 30 minutes
- Test coverage > 80% in reeve-core
