# Reeve — Increment 1 (slim of spec-v2)

> Smallest useful slice that proves the central thesis: Rhai + pact allowlist
> + exec. Everything else from `spec-v2.md` is **deferred, not dropped** —
> the architecture below leaves room for each deferred item to be added
> additively.

## Goal

A `reeve run script.rhai` that executes a Rhai script which can call basic
Linux commands (`echo`, `date`, `uname`, `whoami`, `hostname`) with
allowlisted arguments, throwing on any policy violation. No external infra
needed to run the test suite — works on any Linux/macOS dev box.

## Delivers

### Layout

Single `reeve` crate at the repo root, organized by module:

```
src/
├── lib.rs
├── bin/
│   └── reeve.rs     # CLI: `reeve run <script>`, `reeve version`
├── core/            # Rhai engine + executor + tracing hook
│   ├── mod.rs
│   ├── engine.rs
│   ├── executor.rs
│   ├── logging.rs
│   └── parse.rs
└── pact/            # YAML schema + pure-allowlist validator + named kinds
    ├── mod.rs
    ├── schema.rs
    ├── parse.rs
    ├── engine.rs
    ├── kinds.rs
    ├── error.rs
    └── presets.rs
```

(`reeve-flex` is *not* created in this increment. When it appears, it slots
in as a second `[[bin]]` entry in the same `Cargo.toml` reusing `core` and
`pact` modules from the library.)

### Engine (`core`)

- Rhai engine with:
  - `set_max_operations(1_000_000)`
  - `set_max_call_stack_depth(32)`
  - `set_max_string_size(102_400)`
  - `set_max_array_size(10_000)`
  - `set_max_modules(0)`
  - `disable_symbol("eval")`
- No `rhai-fs` registration. No FS host fns.
- Resource-limit overflow → throws.

### Host functions (Rhai)

```rhai
exec(binary, args) -> map        // throws on non-zero, timeout, output cap
exec_allow_fail(binary, args) -> map  // throws only on timeout / output cap
parse_json(s) -> dynamic
parse_yaml(s) -> dynamic
script_args() -> array           // raw CLI args after script path
print(...)
log_info(msg)
log_warn(msg)
log_error(msg)
```

`exec` return / throw shape:
- Returns `{ stdout, stderr, exit_code, duration_ms }` on success.
- Throws `ExecFailed { binary, exit_code, stdout, stderr }` on non-zero.
- Throws `Timeout { binary, elapsed_ms, limit_ms }` on per-exec timeout.
- Throws `OutputLimitExceeded { binary, bytes_seen, limit }` on overflow.
- Throws `BinaryNotAllowed`, `SubcommandNotAllowed`, `FlagNotAllowed`,
  `FlagValueRejected`, `PositionalRejected` on policy violation.

### Pact (`pact`)

YAML schema (subset). Each binary may declare **either** `subcommands:`
**or** top-level `allowed_flags`/`positional`/`flag_values` (basic Linux
commands have no subcommands; engine dispatches based on which is present):

```yaml
version: 1
name: linux-readonly
description: Basic POSIX info commands (echo/date/uname/whoami/hostname) — no side effects

defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576    # 1 MiB

binaries:
  echo:
    path: /bin/echo
    allowed_flags: [-n, -e]
    positional:
      - { kind: string, repeated: true }

  date:
    path: /bin/date
    allowed_flags: [-u, -I, -R]
    positional:
      - { kind: string, optional: true, repeated: true }   # +FORMAT strings

  uname:
    path: /usr/bin/uname
    allowed_flags: [-a, -s, -r, -m, -n, -p]

  whoami:
    path: /usr/bin/whoami

  hostname:
    path: /bin/hostname
    allowed_flags: [-s, -f]
```

Engine: pure allowlist, fail-closed. **No `forbidden_*` keys.**

> **Path note:** paths above target Linux. macOS dev boxes may need
> `/usr/bin/hostname` instead of `/bin/hostname`. Ship a small `pacts/` dir
> with platform-specific overrides, or document the swap in README.

### Named kinds (only these for now)

- `enum` — `{ kind: enum, values: [...] }`. Exact-match.
- `number` — non-negative integer.
- `string` — literal pass-through. Rejects null bytes and shell
  metacharacters (`; & | $ \` < >` `\n \r`). Used for arbitrary
  user-supplied text like `echo` args or `date +FORMAT`.

(`k8s_namespace`, `k8s_name`, `filepath`, `duration`, custom validators —
all deferred until a preset needs them.)

### Built-in preset

- `linux-readonly.yaml` (the one above), embedded via `include_str!`.

### CLI (`reeve`)

```bash
reeve run <script.rhai>
reeve version
```

That's it. No flags. No `check`, no `preset list`, no `init`.

### Executor flow

```
exec(binary, args):
  1. Look up binary in embedded pact → not found → throw BinaryNotAllowed
  2. Validate first positional as subcommand → throw if not allowed
  3. Validate flag names against allowed_flags → throw
  4. Validate flag values against flag_values kinds → throw
  5. Validate positionals against positional kinds → throw
  6. Build Command (absolute path, argv array, no shell)
  7. Spawn with per-exec timeout
  8. Buffer stdout/stderr up to max_output_bytes; overflow → kill + throw
  9. Per-exec timeout → kill + throw
 10. Non-zero exit → throw (unless exec_allow_fail)
 11. Emit single-line stderr trace via `executor::trace!` macro
 12. Return { stdout, stderr, exit_code, duration_ms }
```

The `executor::trace!` macro currently writes one line per exec call to
stderr. **It's the same call site that JSONL audit will hook into later** —
swap the macro impl, no changes to executor logic.

### Hardcoded config (no security.yaml yet)

In a small `config` module of the `reeve` crate:

```rust
pub const ENV_PASSTHROUGH: &[&str] = &["PATH", "HOME", "LANG"];
// (no env() host fn yet — Command inherits these via std::env)
```

When `security.yaml` lands, this constant turns into a YAML loader.

### Tests (must pass before declaring increment done)

Bypass-resistance subset, one test per assertion (zero external infra):

- `exec("rm", ["-rf", "/"])` → `BinaryNotAllowed`.
- `exec("uname", ["-X"])` → `FlagNotAllowed` (`-X` not in allowed list).
- `exec("echo", ["hello; rm -rf /"])` → `PositionalRejected`
  (`string` kind blocks `;`).
- `exec("whoami", ["root"])` → `PositionalRejected` (whoami declares no
  positionals).
- `exec` exceeding per-exec timeout → `Timeout`. (Use a long-running
  command via the *real* allowed binary — e.g., a sleep added to the
  preset, OR run a unit test with a stub binary that ignores SIGTERM long
  enough to trigger.)
- `exec` exceeding `max_output_bytes` → `OutputLimitExceeded`. (Use
  `echo` with a giant arg, or `yes` if added to the preset.)
- `import "fs"` / `require("os")` → engine error.
- `eval("...")` → engine error.
- Missing pact preset → exit code 3.
- `--pact` flag → CLI parse error (unknown flag).

Plus one happy-path integration test (no infra needed):

- `examples/sysinfo.rhai` runs `whoami`, `hostname -s`, `uname -a`,
  `date -I`, prints them as a small report. Verified against captured
  stdout in the test.

### Measurement

After build:
- `ls -la target/release/reeve` → record size (target < 10 MB).
- `time target/release/reeve run examples/noop.rhai` → record cold start
  (target < 50 ms).

## Deferred (from spec-v2.md, not forgotten)

Each item has a clear additive path forward — no rewrite required.

| Deferred | When to add | Additive change |
|---|---|---|
| `reeve-flex` binary | When MCP/CI integrator appears | New `[[bin]]` in the same crate; reuses `core` + `pact` modules |
| `security.yaml` | When `reeve-flex` ships (caller needs to override) | Replace `config` module constants with YAML loader |
| `runtime.yaml` + `--config` | With `reeve-flex` | New module, new flag |
| Layer 1 FS host fns (`read_file`/`write_file`/`append_file`/`glob`/`exists`/`read_lines`) | When first script needs file I/O | Register new host fns + add `.reeve/<run-id>/` workspace dir |
| Per-run workspace `.reeve/<run-id>/` | With Layer 1 | Runtime-only; doesn't touch engine |
| `stdout_to` exec opt | With Layer 1 | New field in exec opts map |
| JSONL audit log | When forensics demand appears | Swap `executor::trace!` impl |
| `audit.capture_stdout/stderr` flags | With JSONL audit | New flags in security.yaml |
| Script-total timeout | When hung scripts become real | New timer + new throw type |
| `env()` host fn + allowlist | When a script needs to read env | New host fn + uses `ENV_PASSTHROUGH` |
| `env_overrides` per binary | When two binaries need conflicting envs | Already in pact schema; wire through executor |
| Custom validator escape hatch (`kind: custom`) | When a DSL needs it | Add `Custom` variant to kind enum |
| Additional named kinds (`filepath`, `duration`, `k8s_selector`, …) | As pacts demand | New module per kind |
| `k8s-readonly` preset | After dogfooding linux-readonly | New YAML; needs `k8s_namespace` + `k8s_name` kinds |
| `git-readonly` preset | After k8s-readonly | New YAML, embedded |
| `check`, `preset list`, `preset show`, `init` | When users hit ergonomics walls | New clap subcommands |
| `to_json`, `parse_toml` | When scripts need them | New host fns |
| CLI flags (`--workspace`, `--quiet`, `--json`, `--timeout`) | One at a time, as needs arise | Additive in clap |
| `exec_parallel` | v0.2 (per Q12) | New host fn; engine fans out |

## Out (dropped — re-add only if needed)

- **Risk-category vocabulary** (PureRead/Standard/RequiresAudit/Forbidden) —
  documentation polish. Re-add when there are 5+ presets and reviewers need
  a shared vocabulary.
- **Distribution polish** (Homebrew formula, Docker image, pre-built
  binaries for all platforms) — `cargo install --path .` works for
  v0.1-min.

## Done when

- [ ] All bypass-resistance tests pass.
- [ ] `reeve run examples/sysinfo.rhai` prints whoami/hostname/uname/date
      output on any Linux/macOS dev box (zero infra setup).
- [ ] Binary size < 10 MB.
- [ ] Cold start < 50 ms.
- [ ] README has a 5-line "what is this" + 10-line "try it" section, where
      "try it" runs end-to-end without installing anything beyond Rust +
      this repo.

## Scope sanity check

Roughly: 1 week solo if focused. Compared to spec-v2.md (6-8+ weeks),
this is the smallest useful step that doesn't paint into a corner.

> Trace back to: `draft/spec-v2.md` for the full design;
> `.chief/_grill/closed/0001-reeve-design.md` for the decision log.
