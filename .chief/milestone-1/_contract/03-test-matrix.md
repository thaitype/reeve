# Contract — Bypass-Resistance Test Matrix (milestone 1)

Every row MUST be a passing test before milestone 1 is "done". Tests live in
`tests/integration/` (where they need the full CLI) or per-crate `#[cfg(test)]`
modules (where unit-level is sufficient). Each row names its preferred home.

## Policy violations (use `linux-readonly` preset)

| # | Script call                                                | Expected error          | Home                     |
|---|------------------------------------------------------------|-------------------------|--------------------------|
| 1 | `exec("rm", ["-rf", "/"])`                                 | `BinaryNotAllowed`      | reeve-pact integration  |
| 2 | `exec("uname", ["-X"])`                                    | `FlagNotAllowed`        | reeve-pact integration  |
| 3 | `exec("echo", ["hello; rm -rf /"])`                        | `PositionalRejected`    | reeve-pact integration  |
| 4 | `exec("echo", ["a$b"])`                                    | `PositionalRejected`    | reeve-pact integration  |
| 5 | `exec("echo", ["a\nb"])`                                   | `PositionalRejected`    | reeve-pact integration  |
| 6 | `exec("whoami", ["root"])`                                 | `PositionalRejected`    | reeve-pact integration  |

## Engine sandbox

| #  | Script                                            | Expected                       | Home              |
|----|---------------------------------------------------|--------------------------------|-------------------|
| 7  | `import "fs"; ...`                                | Rhai parse / engine error      | reeve-core unit  |
| 8  | `eval("1 + 1")`                                   | Rhai engine error              | reeve-core unit  |
| 9  | Loop body that exceeds `max_operations`           | engine throws                  | reeve-core unit  |

## Executor safety rails (use test-only `test-fixtures` pact)

| #  | Script                                            | Expected                       | Home                    |
|----|---------------------------------------------------|--------------------------------|-------------------------|
| 10 | `exec("sleep", ["5"])` with timeout=1             | `Timeout`                      | reeve-core integration |
| 11 | `exec("yes", [])`                                 | `OutputLimitExceeded`          | reeve-core integration |

## CLI

| #  | Invocation                                        | Expected                       | Home              |
|----|---------------------------------------------------|--------------------------------|-------------------|
| 12 | `reeve run script.rhai --pact x.yaml`            | clap parse error, exit ≠ 0     | reeve CLI test   |
| 13 | `reeve run nonexistent.rhai`                     | exit ≠ 0, stderr names file    | reeve CLI test   |

## Happy path

| #  | Scenario                                           | Expected                       | Home              |
|----|----------------------------------------------------|--------------------------------|-------------------|
| 14 | `examples/sysinfo.rhai` runs `whoami`, `hostname -s`, `uname -a`, `date -I`; verifies non-empty stdout for each on Linux + macOS | exit 0, captured output non-empty | reeve CLI test   |

## Dropped from increment-1's original list

- "Missing pact preset → exit code 3" — N/A in milestone 1 (single embedded
  preset, no selection mechanism). See `_goal/02-decisions.md` D3.

## Measurement (record, don't gate)

After `cargo build --release`:

- `ls -la target/release/reeve` → record byte size; **fail milestone if > 10 MB**.
- `time target/release/reeve run examples/noop.rhai` (3 runs, take min) →
  record cold-start; **fail milestone if > 50 ms**.
