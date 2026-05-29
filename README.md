# Reeve

[![Status: experimental](https://img.shields.io/badge/status-experimental-orange.svg)](#status)

**Allowlist-first runtime for shell automation.** Scripts are written in
[Rhai](https://rhai.rs); every external command must be declared in a
YAML pact. Anything not declared is rejected before it runs. No shell,
no globbing, no environment injection.

```js
let host = exec("hostname", ["-s"]);
let kern = exec("uname", ["-a"]);
print(`Running on ${host.stdout.trim()} (${kern.stdout.trim()})`);

// exec("rm", ["-rf", "/"]);  // → BinaryNotAllowed at runtime
```

## Why not bash?

Bash gives you everything by default and asks you to remove it. Reeve
gives you nothing and asks you to declare what's needed. Concretely:

|                                                    | Bash | Reeve       |
| -------------------------------------------------- | ---- | ------------ |
| Run a binary not on the allowlist                  | ✅   | ❌ rejected  |
| Pass a flag the policy didn't approve              | ✅   | ❌ rejected  |
| `eval`, `source`, dynamic command construction | ✅   | ❌ disabled  |
| Network, fs, env access without declaring it       | ✅   | ❌ sandboxed |
| Per-process timeout + output cap by default        | ❌   | ✅           |

Trade-offs: less expressive than bash, slower to author for one-off
work, and the pact file adds review surface. Use Reeve when scripts
run in CI/CD, runbooks, or as the tool surface for an AI agent — places
where "what could this script do?" needs an answer you can read in 30
seconds.

## Install

Install from crates.io:

```bash
cargo install reeve
reeve version
```

Or install from source:

```bash
git clone https://github.com/thaitype/reeve
cd reeve
cargo install --path .
reeve version
```

Requires Rust 1.75+ (`rustup install stable`).

> **Note:** `cargo install` drops the binary into `~/.cargo/bin`. Make
> sure that directory is on your `PATH`, otherwise `reeve version`
> will report "command not found" even after a successful install. If
> Cargo's installer prints a path warning, follow its instructions; on
> most shells, adding `export PATH="$HOME/.cargo/bin:$PATH"` to your
> shell rc file is enough.

## Try it (60 seconds)

```bash
reeve run examples/sysinfo.rhai        # print host/user/kernel/date via exec()
reeve run examples/workspace-demo.rhai # write, append, read a file in the sandbox
```

Output:

```
=== sysinfo ===
user:     thada
host:     macbook
kernel:   Darwin macbook 25.2.0 ... arm64
date:     2026-05-04
```

Each line was gathered by an allowlisted binary (`whoami`, `hostname`,
`uname`, `date`). Anything else throws a typed error and exits non-zero.

## What's in the box

**Pact** — the shipped pact (`pacts/unix-readonly.yaml`) permits:

- `echo`, `date`, `uname`, `whoami`, `hostname` — read-only POSIX info
  commands, with safe flags only.

That's it. The whole point is that the surface is small and visible.

To allow more binaries, fork the repo, edit `pacts/unix-readonly.yaml`,
and rebuild. There is intentionally no `--pact` runtime flag — the
trust boundary is the binary itself, so AI agents calling Reeve cannot
swap policy.

**Host functions** available to scripts:

| Function | Description |
|---|---|
| `exec(bin, args)` | Run an allowlisted binary; throws on non-zero exit |
| `exec_allow_fail(bin, args)` | Same but returns map with `exit_code` instead of throwing |
| `read_file(path)` | Read file from workspace; throws `FileNotFound` if missing |
| `read_lines(path)` | Read file as array of lines |
| `write_file(path, content)` | Create file in workspace; throws `FileAlreadyExists` if present |
| `append_file(path, content)` | Append to file; creates if missing |
| `exists(path)` | Returns bool |
| `env(key)` | Read env var declared in `env_passthrough`; throws `EnvDenied` or `EnvUnset` |
| `parse_json(str)` | Parse JSON string to Rhai map |
| `to_json(value)` | Serialise any Rhai value to JSON string |
| `log_info(msg)` / `log_warn` / `log_error` | Emit to stderr + audit log |

## What's NOT allowed (and why)

- `exec("rm", [...])` → `BinaryNotAllowed`. Not in the pact.
- `eval(...)`, `import`, `require` — disabled in the Rhai engine.
- File I/O outside the sandbox — scripts can only read/write within
  `<reeve_home>/workspace/` via the scoped `read_file`/`write_file` host functions.
- Custom pacts at runtime — see above.
- Long-running tail-style commands (`tail -f`, `kubectl logs -f`) —
  conflict with the per-exec timeout. Run a watcher externally and
  call Reeve per snapshot.

## Status

**Experimental.** v0.2.0 ships with sandboxed `exec`, workspace-scoped
file I/O, a JSONL audit log, env isolation, and a 5 MB binary. Breaking
changes are likely as the design lands; consult `draft/spec-v3.md` for
the target shape.

Delivered so far:

- ✅ Rhai scripting engine + YAML pact allowlist.
- ✅ Sandboxed `exec()` with per-process timeout and output cap.
- ✅ Filesystem host functions (`read_file`, `write_file`, `append_file`, `read_lines`, `exists`) scoped to `<reeve_home>/workspace/`.
- ✅ JSONL audit log — every run, every `exec` call, every log event.
- ✅ `env()` host fn gated by `env_passthrough` allowlist; child processes run with a clean environment.
- ✅ `to_json()` / `parse_json()` for structured data.

Roadmap, in rough order:

- Trusted-caller binary (`reeve-flex`) with runtime pact selection
  for MCP servers and CI orchestrators.
- `pipe()` — chain binaries without temp files.
- Additional presets (`k8s-readonly`, `git-readonly`).

## Development

To hack on Reeve without installing, run from the checkout:

```bash
cargo run --release -- run examples/sysinfo.rhai
cargo run --release -- version
```

Note the `--` separator: anything after it goes to `reeve`, not to
cargo. Drop `--release` for faster rebuilds during iteration; the
`5×` cold-start gain only matters when you're measuring.

Single-crate flat layout at the repo root, with two internal modules:

- `src/pact/` — YAML schema, allowlist engine, named kinds,
  embedded presets. Pure logic, no I/O.
- `src/core/` — Rhai engine, host functions, process
  executor, timeouts, output caps.

The CLI binary lives in `src/bin/reeve.rs`.

Useful loops:

```bash
cargo test                                  # all tests
cargo clippy --all-targets -- -D warnings   # lint gate
cargo build --release                       # ship binary
```

Both `test` and `clippy` must be clean before opening a PR.

## Contributing

Issues and PRs welcome. Notable areas where help is useful right now:

- New named kinds for pact validators (`filepath`, `duration`, ...).
- Cross-platform CI matrix.
- Additional read-only presets backed by real-world tools.

Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` before opening a PR. Both must be
clean.

## License

[Apache License 2.0](LICENSE).
