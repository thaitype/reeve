# Release Notes — v0.2.0

**Released:** 2026-05-29

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
- `exec_start` / `exec_end` / `exec_error` — binary, argv, exit code, duration per call
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
