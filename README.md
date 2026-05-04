# Warden

Allowlist-first runtime for Rhai scripts. Every external command must be
declared in a YAML pact; anything else is rejected at parse time. No
shell, no globbing, no env injection.

## Why

Warden exists for contexts where automation must be auditable: CI/CD
pipelines, runbooks, and AI agent tool-use. Scripts are written in Rhai
(a safe embedded language); the pact file is the single source of truth
for what binaries and flags are permitted. If it is not in the pact, it
does not run. See `draft/spec-v2.md` for the full design.

## Try it (60 seconds)

Prereq: Rust 1.75+ (`rustup install stable`).

```bash
git clone https://github.com/thaitype/warden
cd warden
cargo build --release -p warden
./target/release/warden version
./target/release/warden run examples/sysinfo.rhai
```

Expected output of the last command: a small report listing your user,
hostname, kernel, and the date — all gathered through allowlisted
binaries.

## What's allowed (milestone 1)

The shipped pact `pacts/unix-readonly.yaml` permits:

- `echo`, `date`, `uname`, `whoami`, `hostname`

with safe flags only. Write a script under `examples/` and run it with
`./target/release/warden run examples/your-script.rhai`.

## What's NOT allowed (and why)

- `exec("rm", [...])` → `BinaryNotAllowed`
- `eval(...)`, `import "fs"`, file I/O — disabled in the engine
- Custom pacts at runtime — milestone 1 ships a single embedded preset.
  Forking the repo and recompiling is the only way to change policy.

## Project status

Milestone 1 = smallest useful slice (Rhai + pact + exec). See
`draft/spec-v2.md` for the full design and `.chief/milestone-1/`
for the milestone plan and reports.
