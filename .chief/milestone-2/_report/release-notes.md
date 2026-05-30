# Release Notes — v0.2.0

**Released:** 2026-05-29

---

## Motivation

v0.1.0 established the core execution model — a Rhai script engine with a YAML pact allowlist that blocks undeclared binaries and arguments before any process is spawned. What it left open was the question of *evidence and isolation*: after a script ran, there was no record of what it did, child processes could see every secret in the host environment, and scripts had no safe place to read or write structured data without going through an external binary.

v0.2.0 closes those gaps. Every run now produces an append-only JSONL audit trail covering each `exec` call and its outcome. Child processes are started with a clean environment — only keys explicitly declared in `env_passthrough` are forwarded, so `AWS_SECRET_ACCESS_KEY`, `GITHUB_TOKEN`, and similar secrets stay invisible to spawned binaries. Scripts gain a sandboxed workspace for file I/O that enforces the same fail-closed philosophy as the pact: absolute paths, traversal, and symlink escapes are rejected at validation time, not discovered after the fact.

---

## Security model

### What Reeve is responsible for

- **Binary allowlist** — only binaries declared in the embedded pact can be called via `exec()`. Any other binary throws `BinaryNotAllowed` before a process is spawned.
- **Argument validation** — every argument is matched against the pact's declared kinds (`enum`, `number`, `string`). Arguments containing shell metacharacters are rejected. Undeclared flags throw `FlagNotAllowed`. (A `filepath` kind with `allowed_roots` scoping is forthcoming in v0.3.0.)
- **Environment isolation** — child processes inherit only the keys listed in `env_passthrough`. The host environment is otherwise cleared before spawn.
- **Workspace sandboxing** — FS host functions (`read_file`, `write_file`, etc.) are strictly scoped to `<reeve_home>/workspace/`. Absolute paths, `..` components, and symlinks pointing outside the workspace are rejected.
- **Audit trail** — every run produces a JSONL log of all `exec` calls, arguments, exit codes, and durations. Command capture is on by default; stdout/stderr capture is opt-in.
- **Pact immutability** — the pact and `security.yaml` are embedded at compile time. A running script or AI agent cannot change the policy it is being enforced by.

### What Reeve does not protect against

- **OS-level isolation** — Reeve runs as the invoking user. A pact that allows `chmod`, `chown`, or a setuid binary gives that binary the same OS privileges the user already has. For kernel-level syscall or network isolation, layer Reeve inside `bwrap` or `firejail`.
- **Audit log integrity** — the audit log is written with standard file permissions. Another process running as the same OS user can read or modify it. Reeve makes no cryptographic integrity guarantees over the log.
- **Pact correctness** — Reeve enforces the pact faithfully, but cannot reason about whether the pact itself is safe. A pact that allows `curl -o /dev/stdout <url>` is syntactically valid. Pact review is the operator's responsibility.
- **Secret exfiltration via allowed binaries** — a pact that permits `echo` or `curl` with a permissive argument kind cannot prevent a script from passing a secret string as an argument. Reeve validates argument *shape*, not *content semantics*.
- **Network access** — Reeve imposes no network restrictions. A binary that opens a socket is not blocked.
- **Resource exhaustion** — `exec()` does not impose a per-call timeout or output cap. A hung binary blocks the run until killed externally, and a binary that floods stdout is read unbounded into memory. This matches bash's default behaviour; bound it with an OS-level wrapper (`timeout`, cgroups, CI job limits) or an external supervisor.

---

## What's new

### Persistent home directory

Reeve now maintains a per-user home at `$HOME/.reeve/` (configurable via `security.yaml`). On first run it creates `workspace/` and `runs/` directories automatically — no setup step required.

### File I/O host functions (Layer 1)

Scripts can now read and write files within the workspace sandbox:

```js
write_file("report.txt", "build passed\n");
append_file("log.txt", `${exec("date", []).stdout.trim()}: done\n`);
let content = read_file("report.txt");
let lines   = read_lines("log.txt");
let exists  = exists("report.txt");   // true
```

All paths are scoped to `<reeve_home>/workspace/`. Absolute paths, `..` traversal, and symlinks pointing outside the workspace are rejected. `write_file` throws `FileAlreadyExists` on collision — re-runs fail loudly rather than silently overwriting prior output.

### JSONL audit log

Every run now writes a tamper-evident audit trail to `$HOME/.reeve/runs/<run-id>/audit.jsonl`. The log captures:

- `script_start` / `script_end` — script path, args, duration, exit status
- `exec_start` / `exec_end` — binary, argv, exit code, duration per call
- `script_log` — `log_info`, `log_warn`, `log_error` calls from the script

Each event is flushed immediately so the log remains readable after a crash or timeout. Run directories sort chronologically by name (UUID v7).

### `env()` host function

```js
let path = env("PATH");   // allowed if PATH is in env_passthrough
let tok  = env("SECRET"); // → throws EnvDenied if not declared
```

Env access is gated by the `env_passthrough` list in `security.yaml`. Keys not on the list throw `EnvDenied`; listed-but-absent keys throw `EnvUnset`. No silent empty-string fallback.

### `to_json()` host function

```js
let result = exec("kubectl", ["get", "pods", "-o", "json"]);
let pods   = parse_json(result.stdout);
let out    = to_json(pods);   // back to JSON string
```

Serialises any Rhai value to a JSON string. Complement to the existing `parse_json()`.

### Child process env isolation

Spawned child processes now run with a clean environment — only keys declared in `env_passthrough` are forwarded. Host secrets (`AWS_SECRET_ACCESS_KEY`, `GITHUB_TOKEN`, etc.) are not visible to any binary called via `exec()`.

---

## Security fixes

Four correctness and security issues fixed before this release:

| Issue | Impact | Fix |
|---|---|---|
| `exec()` with non-string argument (e.g. `exec("echo", [42])`) panicked the process and skipped the audit entry for that call | Audit bypass on script error | `try_cast` with catchable `TypeError` error |
| Reader thread panics were silently swallowed; partial output used with no error signal | Silent data corruption | `thread::join()` errors now propagate as `ExecFailed` |
| `path.contains("..")` rejected legitimate filenames like `v2..1/out.txt` | False `PathDenied` on valid paths | Replaced with path-component check |
| `elapsed_ms` was measured before output buffers were drained | Audit underreported wall time | Measurement moved to after reader thread join |

---

## Q&A

**Q: What is the difference between the workspace directory and `allowed_roots` in `security.yaml`?**

They are two separate filesystem layers with different purposes:

| | Layer 1 — Workspace | Layer 2 — `allowed_roots` |
|---|---|---|
| Applies to | `read_file`, `write_file`, `append_file`, `read_lines`, `exists` | filepath arguments passed to `exec()` |
| Scope | `$HOME/.reeve/workspace/` only | `working_dir` + any paths listed in `allowed_roots` |
| Defined in | Hardcoded in the engine | `security.yaml` (compile-time embedded) |
| Status | ✅ Shipped in v0.2.0 | ⏳ Deferred to v0.3.0 |

**Layer 1** is the script's own sandbox — a place for scripts to read and write state between steps. The path is always `$HOME/.reeve/workspace/` and cannot be changed at runtime.

```js
write_file("output.json", data);  // → $HOME/.reeve/workspace/output.json
read_file("output.json");
```

**Layer 2** is a guard on filepath *arguments* that scripts pass to external binaries via `exec()`. For example, `kubectl apply -f <path>` — the pact would mark that argument as `kind: filepath` (a forthcoming v0.3.0 kind; not valid today), and `allowed_roots` limits which directories on the host filesystem that path may resolve to. This is not yet enforced; `allowed_roots` is parsed and stored but not checked until v0.3.0.

```js
// v0.3.0: Layer 2 will validate that ./manifests/ is inside allowed_roots
exec("kubectl", ["apply", "-f", "./manifests/deploy.yaml"]);
```

In short: Layer 1 is the script's scratchpad; Layer 2 is the fence around what host paths external binaries can reach.

---

**Q: Workspace cleanup — when does it happen, and is there a command for it?**

Reeve does not auto-delete the workspace. `$HOME/.reeve/workspace/` persists across runs by design — scripts can intentionally leave files for the next run to read. To clean up, delete manually:

```bash
rm -rf ~/.reeve/workspace/*   # clear workspace files
rm -rf ~/.reeve/runs/*        # clear audit logs
```

---

**Q: Can a script read files from outside the workspace, e.g. a config file the operator placed somewhere?**

Not directly via the FS host functions — they are strictly scoped to `$HOME/.reeve/workspace/`. To read an external file, use an allowlisted binary:

```js
let cfg  = exec("cat", ["/etc/myapp/config.json"]);
let data = parse_json(cfg.stdout);
```

This requires `cat` to be in your pact with `kind: string` on the argument (which accepts an absolute path, since the `string` kind only forbids shell metacharacters). Note: host-path scoping via `kind: filepath` and `allowed_roots` is deferred to v0.3.0.

---

**Q: How do I use Reeve with an AI agent like Claude or GPT?**

Point the agent at the `reeve` binary. The agent writes a Rhai script and runs it:

```bash
reeve run agent-generated.rhai
```

The pact is the trust boundary — the agent can only call binaries and pass arguments that the pact declares. Anything outside the pact throws a typed error before the process is ever spawned.

---

**Q: Why Rhai instead of Lua, Python, or Bash?**

Three reasons drove the choice:

1. **Pure Rust, no C dependency** — Lua and Python require a C runtime, which complicates static cross-compilation and inflates binary size. Rhai is a Rust crate.
2. **Sandbox by default** — Rhai has no filesystem, network, or process access unless the host explicitly registers those functions. Stripping Lua or Python to the same level requires disabling most of their standard library.
3. **First-class resource limits** — `set_max_operations`, `set_max_call_levels`, and `set_max_string_size` are built into Rhai's engine API, not bolted on.

The main trade-off: Rhai has a smaller corpus in AI training data than Lua or Python. The mitigation is that Reeve's host functions (`exec`, `read_file`, `env`, etc.) matter more than the base language — and those are documented and demonstrated in the examples directory.

---

## Breaking changes

None. v0.2.0 is backwards-compatible with v0.1.0 scripts and pacts.

---

## Binary targets

| Metric | v0.2.0 | Target |
|---|---|---|
| Binary size | 5.0 MB | < 10 MB |
| Cold start | 6 ms | < 50 ms |
| Test suite | 99 tests passing | — |

Platform: Linux/macOS, x86_64/arm64.

---

## What's next (v0.3.0)

- `reeve-flex` binary — trusted-caller variant with runtime `--pact` and `--config` flags
- `pipe()` / `pipe_allow_fail()` — chain binaries without temp files
- Layer 2 filepath validation — `allowed_roots` enforcement for `exec()` filepath arguments
- `glob()` host function
