# Reeve — Increment 2: Filesystem Model + Audit + `security.yaml`

> Builds on `increment-1.md`. Adds the persistent state that makes `reeve`
> useful as an AI agent runtime: a typed home directory, workspace-scoped FS
> host fns, JSONL audit, `security.yaml`, and the `env()` host fn.
> `reeve-flex`, `pipe()`, Layer 2, and the canonical presets are all deferred.

## Goal

A `reeve run script.rhai` that can read and write files inside
`$HOME/.reeve/workspace/`, read allowed env vars, produce a JSONL audit trail
in `$HOME/.reeve/runs/<run-id>/`, and have its security boundary declared in a
compile-time-embedded `security.yaml`.

## Delivers

### `security.yaml` — compile-time embedded

New file at repo root, embedded via `include_str!` into the `reeve` binary:

```yaml
# security.yaml
reeve_home: "$HOME/.reeve"
allowed_roots:
  - "$CWD"
  - "$HOME/.reeve/workspace"
deny_traversal: true
env_passthrough: [PATH, HOME, LANG]
audit:
  capture_command: true
  capture_stdout: false
  capture_stderr: false
```

Loaded once at startup into a `SecurityConfig` struct. `$HOME` and `$CWD` are
expanded at runtime (not in YAML). No `--config` flag; values are fixed per
build.

New module: `src/security.rs`. Replaces the `config::ENV_PASSTHROUGH` constant
from increment-1.

### `$HOME/.reeve/` — lazy init

On first run, engine creates:

```
$HOME/.reeve/
├── .reeve-managed        # sentinel
├── workspace/
└── runs/
```

Fails loudly (`HomeInitError`) if the target path exists but is not a
directory, or if `.reeve-managed` is absent on an existing populated tree.

Module: `src/core/home.rs`.

### Layer 1 FS host functions

Scoped to `<reeve_home>/workspace/` — hardcoded, not configurable at runtime.
All paths are resolved relative to `workspace/`; absolute paths and `..`
traversal throw `PathDenied`.

```rhai
read_file(path) -> string          // throws PathDenied, FileNotFound
read_lines(path) -> array<string>  // same
exists(path) -> bool
glob(pattern) -> array<string>     // workspace-rooted glob
write_file(path, content)          // throws FileAlreadyExists, PathDenied
append_file(path, content)         // creates if missing; throws PathDenied
```

No `delete`, `rename`, `move`, or overwrite. `write_file` throws
`FileAlreadyExists` if the path is occupied — append-only by design.

Registered in `src/core/fs.rs`; wired into `engine.rs` alongside exec.

### JSONL audit log

Every run writes to `$HOME/.reeve/runs/<run-id>/audit.jsonl`. `run-id` is a
UUID v4 generated at startup.

Events emitted:

```jsonl
{"event":"script_start","ts":"...","run_id":"...","script_path":"...","script_sha256":"...","args":[...]}
{"event":"exec_start","ts":"...","binary":"whoami","argv":[]}
{"event":"exec_end","ts":"...","binary":"whoami","exit_code":0,"duration_ms":12,"stdout_bytes":7,"stderr_bytes":0}
{"event":"exec_error","ts":"...","binary":"...","kind":"Timeout","limit_ms":10000}
{"event":"script_log","ts":"...","level":"info","msg":"..."}
{"event":"script_end","ts":"...","exit_status":"ok","duration_ms":340,"exec_count":2}
```

`audit.capture_stdout/capture_stderr` (from `security.yaml`) gate whether
stdout/stderr content events are emitted. Off by default.

`executor::trace!` macro from increment-1 is replaced by `audit::emit()` calls
at the same call sites — no structural change to executor logic.

Module: `src/core/audit.rs`. The `AuditWriter` is created in `main()` and
passed into the executor context (not a global).

### `env()` host fn

```rhai
env(key) -> string
// allowed (env_passthrough): returns value
// denied: throws EnvDenied
// allowed but unset: throws EnvUnset
```

Keys checked against `security.yaml.env_passthrough`. No probing — callers
must guard explicitly if a key may be absent.

Registered in `engine.rs`. Uses `SecurityConfig` loaded at startup.

### `to_json()` host fn

```rhai
to_json(v) -> string   // serialises any Dynamic to JSON; throws SerializeError
```

Trivial companion to existing `parse_json()`. Registered alongside it in
`engine.rs`.

### Updated `exec()` env behaviour

Increment-1's executor called `env_clear()` and passed hardcoded
`ENV_PASSTHROUGH`. In this increment, `Command` inherits
`security.yaml.env_passthrough` instead. No API change — `exec(binary, args)`
signature is unchanged.

### New exit code

- `3` — configuration error (security.yaml parse error, home init failure).
  Previously only covered missing preset; now also covers init failures.

### CLI — unchanged

`reeve run <script>` and `reeve version`. No new subcommands this increment.

### Tests (must pass)

Bypass-resistance additions:

- `read_file("/etc/passwd")` → `PathDenied`.
- `read_file("../../etc/passwd")` → `PathDenied` (traversal).
- `write_file("report.json", "x"); write_file("report.json", "y")` →
  second call throws `FileAlreadyExists`.
- `append_file("/etc/hosts", "x")` → `PathDenied`.
- `env("AWS_SECRET_ACCESS_KEY")` → `EnvDenied`.
- `env("UNSET_ALLOWED_VAR")` → `EnvUnset` (set up with a key that's in
  `env_passthrough` but absent from the test process env).
- `REEVE_HOME=/tmp/x reeve run ...` → env var ignored; home comes from
  compiled `security.yaml`.

Happy-path additions:

- `write_file("out.txt", "hello")` then `read_file("out.txt")` → `"hello"`.
- `append_file("log.txt", "a")` twice → file contains `"aa"` (no error on
  second append).
- `exists("out.txt")` → `true`; `exists("missing.txt")` → `false`.
- `env("HOME")` → non-empty string.
- `to_json(#{"x": 1})` → `'{"x":1}'` (or equivalent JSON).
- Audit file is written at `$HOME/.reeve/runs/<run-id>/audit.jsonl` and
  contains `script_start` and `script_end` events.

## Deferred (not forgotten)

| Deferred | When | Additive change |
|---|---|---|
| `reeve-flex` binary | Increment 3 | New `[[bin]]`; reuses `core` + `pact` + `security` |
| `pipe()` + `pipe_allow_fail()` | Increment 3 or 4 | New host fns; executor fan-out |
| `exec()` opts (`stdin:`, `stdout_to:`, `timeout_seconds:`) | Increment 3 | Extend exec signature; `stdout_to` streams to `workspace/` |
| Layer 2 / `allowed_roots` filepath validation in `exec()` | Increment 3 | New gate in executor; `filepath` kind in pact |
| `check`, `preset list`, `preset show` subcommands | Later | New clap subcommands |
| Three canonical presets (`core-tools`, `k8s-readonly`, `git-readonly`) | Later | Needs `filepath`, `duration`, `k8s_*` kinds |
| `config.json` + `max_workspace_bytes` / `auto_cleanup` | Later | New module; startup gate |
| `--timeout`, `--quiet`, `--json` CLI flags | Later | Additive in clap |
| `parse_toml()` | Later | New host fn |
| Custom validators (`kind: custom`) | Later | Needs DSL use case |

## Out

- `denied_roots` — pure-allowlist invariant holds; anything not in
  `allowed_roots` is already denied.
- `--isolated` per-run scope — deferred to post-v0.1 per spec-v3.

## Done when

- [ ] All bypass-resistance tests (increment-1 + new ones above) pass.
- [ ] `reeve run examples/sysinfo.rhai` still works (regression check).
- [ ] New example `examples/workspace-demo.rhai` writes a file, appends to
      it, reads it back, and prints contents — runs clean on any dev box.
- [ ] Audit file present and parseable after any run.
- [ ] `REEVE_HOME=/tmp/x reeve run examples/sysinfo.rhai` uses
      `security.yaml`-compiled home (env var ignored).
- [ ] Binary size still < 10 MB.

## Scope

Roughly 1 focused week. Largest chunk is the audit writer + home init;
FS host fns and `env()` are small. No new dependencies needed beyond what's
already in `Cargo.toml`.

> Trace back to: `draft/spec-v3.md` for full design;
> `draft/increment-1.md` for prior increment.
