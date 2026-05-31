# Contract — JSONL Audit Log

## File location

```
<reeve_home>/runs/<run-id>/audit.jsonl
```

`run-id` is a UUID v7 generated at process startup (`uuid::Uuid::now_v7()`).
The `runs/<run-id>/` directory is created by `AuditWriter::open()` before the
first event is written.

## `AuditWriter` (`src/core/audit.rs`)

```rust
pub struct AuditWriter {
    file:   BufWriter<File>,
    run_id: String,
}

impl AuditWriter {
    pub fn open(runs_dir: &Path, run_id: &str) -> Result<Self, AuditError>
    pub fn emit(&mut self, event: &AuditEvent) -> Result<(), AuditError>
}
```

`emit()` serialises `event` to a single JSON line + newline, writes it,
then calls `self.file.flush()`. Flush-per-event is mandatory (decision D2).

`AuditError` wraps `std::io::Error`. Audit write failures are logged to
stderr but do NOT abort the script run — audit is best-effort in v0.1.

## Event schema

Every event is a JSON object on its own line. All events carry:

```json
{ "event": "<name>", "ts": "<RFC 3339 millis>", "run_id": "<uuid>" }
```

`ts` format: `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)`
→ e.g. `"2026-05-12T10:23:45.123Z"`.

### `script_start`

Emitted immediately after home init and before script evaluation.

```jsonl
{"event":"script_start","ts":"...","run_id":"...","script_path":"...","args":[...]}
```

- `script_path` — absolute path (`fs::canonicalize`; raw path on error).
- `args` — raw CLI args after the script path (may be empty array).
- No `script_sha256` this milestone.

### `exec_start`

Emitted just before spawning the child process.

```jsonl
{"event":"exec_start","ts":"...","run_id":"...","binary":"kubectl","argv":["get","pods"]}
```

When `audit.capture_command` is `false` in `security.yaml`, the `argv` field is
emitted as an empty array (`[]`) to avoid logging potentially sensitive arguments.
The `binary` field is always present. The default is `true` (full argv logged).

```jsonl
{"event":"exec_start","ts":"...","run_id":"...","binary":"kubectl","argv":[]}
```

### `exec_end`

Emitted after the child process exits (success or non-zero).

```jsonl
{"event":"exec_end","ts":"...","run_id":"...","binary":"kubectl","exit_code":0,"duration_ms":234,"stdout_bytes":1024,"stderr_bytes":0}
```

### `script_log`

Emitted by `log_info`, `log_warn`, `log_error`.

```jsonl
{"event":"script_log","ts":"...","run_id":"...","level":"info","msg":"Found 3 failing pods"}
```

`level`: `"info"` | `"warn"` | `"error"`.

### `script_end`

Emitted as the last event before process exit.

```jsonl
{"event":"script_end","ts":"...","run_id":"...","exit_status":"ok","duration_ms":1234,"exec_count":5}
```

`exit_status`: `"ok"` | `"error"`.

## Capture flags

`audit.capture_stdout` and `audit.capture_stderr` in `security.yaml` are
parsed into `SecurityConfig.audit` but have no effect on emitted events
this milestone (no stdout/stderr capture events). The fields are wired
through so the feature can be enabled by flipping flags in a later
increment without a schema change.

## Audit write failures

If `AuditWriter::emit()` returns an error:
1. Print a single `WARN: audit write failed: <msg>` line to stderr.
2. Continue script execution — do not abort.

## Module

`src/core/audit.rs` owns `AuditWriter`, `AuditEvent` (enum or tagged
struct), and `AuditError`.
