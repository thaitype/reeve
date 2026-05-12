# Contract — Host Functions (Milestone 2 additions)

Extends `milestone-1/_contract/02-host-fns.md`. All milestone-1 host fns
(`exec`, `exec_allow_fail`, `parse_json`, `parse_yaml`, `script_args`,
`print`, `log_*`) remain unchanged in signature. Only additions and
modifications are documented here.

---

## New host functions

### Layer 1 FS (`src/core/fs.rs`)

All paths are relative to `<reeve_home>/workspace/`. Absolute paths and
`..` components throw `PathDenied` before any filesystem access.

Path validation rule:
1. Join `workspace_root.join(path)`.
2. Call `fs::canonicalize` on the parent dir (not the full path, since the
   file may not exist yet for writes). If the resolved path does not start
   with `workspace_root` → throw `PathDenied`.
3. For reads: check file exists → throw `FileNotFound` if not.
4. For `write_file`: check file does NOT exist → throw `FileAlreadyExists`
   if it does.

```
read_file(path: string) -> string
```
Throws: `PathDenied`, `FileNotFound`, `IoError`.

```
read_lines(path: string) -> array<string>
```
Returns lines without trailing newlines. Throws: same as `read_file`.

```
exists(path: string) -> bool
```
Returns `true` if file exists within `workspace/`. Returns `false` (does
not throw) for paths outside `workspace/` that would otherwise be
`PathDenied` — existence is safe to probe. Throws `IoError` only on
unexpected OS errors.

```
write_file(path: string, content: string)
```
Creates the file (and any intermediate dirs within `workspace/`). Throws
`FileAlreadyExists` if path is occupied. Throws `PathDenied`, `IoError`.

```
append_file(path: string, content: string)
```
Creates the file if absent; appends if present. Throws `PathDenied`,
`IoError`.

---

### `env()` (`src/core/engine.rs`)

```
env(key: string) -> string
```
Checks `key` against `SecurityConfig.env_passthrough`.
- Key in passthrough + set → returns value.
- Key in passthrough + unset → throws `EnvUnset { key }`.
- Key NOT in passthrough → throws `EnvDenied { key }`.

---

### `to_json()` (`src/core/engine.rs`)

```
to_json(v: Dynamic) -> string
```
Serialises any Rhai `Dynamic` to a JSON string using `serde_json` (Rhai's
`serde` feature is already enabled). Throws `SerializeError { msg }` on
failure.

---

## Modified host functions

### `log_info`, `log_warn`, `log_error`

Signatures unchanged. Now also emit a `script_log` audit event via
`RunContext.audit` in addition to writing to stderr.

### `exec` / `exec_allow_fail`

Signatures unchanged. `env_passthrough` now sourced from
`SecurityConfig.env_passthrough` (via `RunContext.security`) instead of
the hardcoded `ENV_PASSTHROUGH` constant. Runtime behaviour is identical
for the default `security.yaml` which ships `[PATH, HOME, LANG]`.

---

## Error kind registry (additions)

All new error kinds follow the `#{ kind: "...", ...payload }` shape from
`_rules/_contract/error-maps.md`.

| Kind | Thrown by | Payload fields | Exit code |
|---|---|---|---|
| `PathDenied` | FS fns | `path: string` | 1 |
| `FileNotFound` | `read_file`, `read_lines` | `path: string` | 1 |
| `FileAlreadyExists` | `write_file` | `path: string` | 1 |
| `IoError` | FS fns | `path: string`, `msg: string` | 1 |
| `EnvDenied` | `env` | `key: string` | 1 |
| `EnvUnset` | `env` | `key: string` | 1 |
| `SerializeError` | `to_json` | `msg: string` | 1 |

`classify_error` in `src/bin/reeve.rs` does not need updating — all new
kinds map to exit code 1 (script error).

---

## `build_engine_with_args` signature change

```rust
// milestone-1
pub fn build_engine_with_args(args: Vec<String>) -> Engine

// milestone-2
pub fn build_engine_with_args(args: Vec<String>, ctx: Arc<RunContext>) -> Engine
```

All callers (currently only `src/bin/reeve.rs`) must be updated.
