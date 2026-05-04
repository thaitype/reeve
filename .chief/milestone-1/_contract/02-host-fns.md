# Contract — Rhai Host Functions (milestone 1)

Functions registered on the Rhai engine. Anything not listed here is **not**
exposed to scripts. Standard Rhai language features (`let`, `for`, `if/else`,
`while`, `fn`, arrays, maps, string templates) remain available subject to
engine resource limits.

## `exec(binary: string, args: array) -> map`

Validates `binary` + `args` against the active pact, then spawns the process.

- Always uses `argv` array form — never `shell:true`.
- Inherits env from the host process for milestone 1 (no env scrubbing yet;
  `env_passthrough` enforcement deferred with `security.yaml`).
- Per-exec timeout = pact `defaults.timeout_seconds` (no per-call override
  in milestone 1).
- Captures stdout + stderr in memory up to pact `defaults.max_output_bytes`.
  On overflow → kill child and throw `OutputLimitExceeded`.

### Returns (success, exit code 0)

```rhai
#{
  stdout:      "...",   // string
  stderr:      "...",   // string
  exit_code:   0,       // i64
  duration_ms: 234,     // i64
}
```

### Throws

| Error                | When                                              |
|----------------------|---------------------------------------------------|
| `BinaryNotAllowed`   | `binary` not in active pact                       |
| `SubcommandNotAllowed` | first positional not in `subcommands` (when pact uses subcommands) |
| `FlagNotAllowed`     | a flag in `args` not in `allowed_flags`           |
| `FlagValueRejected`  | flag value fails kind validation                  |
| `PositionalRejected` | positional fails kind validation, or extra positional given |
| `Timeout`            | per-exec timer expired                            |
| `OutputLimitExceeded`| stdout+stderr exceeded `max_output_bytes`         |
| `ExecFailed`         | child exited non-zero                             |

Error payloads are Rhai maps so scripts can inspect them. Minimum fields:

```rhai
ExecFailed         #{ kind: "ExecFailed",         binary, exit_code, stdout, stderr }
Timeout            #{ kind: "Timeout",            binary, elapsed_ms, limit_ms }
OutputLimitExceeded#{ kind: "OutputLimitExceeded",binary, bytes_seen, limit }
BinaryNotAllowed   #{ kind: "BinaryNotAllowed",   binary }
FlagNotAllowed     #{ kind: "FlagNotAllowed",     binary, flag }
FlagValueRejected  #{ kind: "FlagValueRejected",  binary, flag, value, reason }
PositionalRejected #{ kind: "PositionalRejected", binary, index, value, reason }
SubcommandNotAllowed #{ kind: "SubcommandNotAllowed", binary, subcommand }
```

## `exec_allow_fail(binary, args) -> map`

Identical to `exec`, except a non-zero exit returns the result map (with
`exit_code` set) instead of throwing. `Timeout`, `OutputLimitExceeded`, and
all policy-violation errors still throw.

## Data parsing

```rhai
parse_json(s: string) -> dynamic    // throws on parse error
parse_yaml(s: string) -> dynamic    // throws on parse error
```

`to_json` and `parse_toml` are **deferred**.

## Arguments

```rhai
script_args() -> array<string>      // raw CLI args after script path
```

When passed to `exec(...)` they go through pact validation like any other
argv element.

## Output / logging

```rhai
print(...)                          // unstructured stdout
log_info(msg: string)               // single-line stderr, level=INFO
log_warn(msg: string)               // single-line stderr, level=WARN
log_error(msg: string)              // single-line stderr, level=ERROR
```

Log format (stable for milestone 1 — JSONL audit will parse this later):

```
<ISO-8601 timestamp> <LEVEL> <message>
```

## Engine configuration (fixed in milestone 1)

```rust
engine.set_max_operations(1_000_000);
engine.set_max_call_stack_depth(32);
engine.set_max_string_size(102_400);
engine.set_max_array_size(10_000);
engine.set_max_modules(0);
engine.disable_symbol("eval");
// rhai-fs NOT registered. No FS host fns at all.
```
