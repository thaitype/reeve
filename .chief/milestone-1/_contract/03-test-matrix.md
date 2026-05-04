# Contract — Bypass-Resistance Test Matrix (milestone 1)

Every row MUST be a passing test before milestone 1 is "done". Tests live in
`tests/integration/` (where they need the full CLI) or per-crate `#[cfg(test)]`
modules (where unit-level is sufficient). Each row names its preferred home.

## Policy violations (use `linux-readonly` preset)

| # | Script call                                                | Expected error          | Home                     |
|---|------------------------------------------------------------|-------------------------|--------------------------|
| 1 | `exec("rm", ["-rf", "/"])`                                 | `BinaryNotAllowed`      | warden-pact integration  |
| 2 | `exec("uname", ["-X"])`                                    | `FlagNotAllowed`        | warden-pact integration  |
| 3 | `exec("echo", ["hello; rm -rf /"])`                        | `PositionalRejected`    | warden-pact integration  |
| 4 | `exec("echo", ["a$b"])`                                    | `PositionalRejected`    | warden-pact integration  |
| 5 | `exec("echo", ["a\nb"])`                                   | `PositionalRejected`    | warden-pact integration  |
| 6 | `exec("whoami", ["root"])`                                 | `PositionalRejected`    | warden-pact integration  |

## Engine sandbox

| #  | Script                                            | Expected                       | Home              |
|----|---------------------------------------------------|--------------------------------|-------------------|
| 7  | `import "fs"; ...`                                | Rhai parse / engine error      | warden-core unit  |
| 8  | `eval("1 + 1")`                                   | Rhai engine error              | warden-core unit  |
| 9  | Loop body that exceeds `max_operations`           | engine throws                  | warden-core unit  |

## Executor safety rails (use test-only `test-fixtures` pact)

| #  | Script                                            | Expected                       | Home                    |
|----|---------------------------------------------------|--------------------------------|-------------------------|
| 10 | `exec("sleep", ["5"])` with timeout=1             | `Timeout`                      | warden-core integration |
| 11 | `exec("yes", [])`                                 | `OutputLimitExceeded`          | warden-core integration |

## CLI

| #  | Invocation                                        | Expected                       | Home              |
|----|---------------------------------------------------|--------------------------------|-------------------|
| 12 | `warden run script.rhai --pact x.yaml`            | clap parse error, exit ≠ 0     | warden CLI test   |
| 13 | `warden run nonexistent.rhai`                     | exit ≠ 0, stderr names file    | warden CLI test   |

## Happy path

| #  | Scenario                                           | Expected                       | Home              |
|----|----------------------------------------------------|--------------------------------|-------------------|
| 14 | `examples/sysinfo.rhai` runs `whoami`, `hostname -s`, `uname -a`, `date -I`; verifies non-empty stdout for each on Linux + macOS | exit 0, captured output non-empty | warden CLI test   |

## Dropped from increment-1's original list

- "Missing pact preset → exit code 3" — N/A in milestone 1 (single embedded
  preset, no selection mechanism). See `_goal/02-decisions.md` D3.

## Measurement (record, don't gate)

After `cargo build --release`:

- `ls -la target/release/warden` → record byte size; **fail milestone if > 10 MB**.
- `time target/release/warden run examples/noop.rhai` (3 runs, take min) →
  record cold-start; **fail milestone if > 50 ms**.
