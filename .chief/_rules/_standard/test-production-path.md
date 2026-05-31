# Standard — Tests Must Exercise the Production Call Path

## Rule

A test that verifies behavior X must call the same function (or CLI
entry point) that production code uses to achieve X.

Test-only wrappers, alternative constructors, and helper shims are
permitted for **setup** (creating temp dirs, building a RunContext,
etc.) — but the **assertion** must land on output from the real
production path.

## Why

`run_exec_with_passthrough` proved that env filtering works correctly.
`run_exec_audited` — the function actually called by Rhai scripts —
still passed `None` and leaked secrets. The test was green; production
was broken. The only test that mattered was the CLI integration test
that ran the binary end-to-end.

A test of a wrapper is a test of the wrapper. It says nothing about
what the production caller does.

## How to apply

Before writing a test, ask:
> "Does this call the same code path a real script would use?"

If yes — write it.

If no — either:
1. Fix the test to go through the production path.
2. Document explicitly which production function is being exercised
   and why a lower-level call is equivalent (e.g. "this calls
   `run_exec_with_env` directly; `run_exec_audited` is a one-line
   wrapper that passes the same args — verified by reading the
   source").

For security-critical behavior (sandbox escapes, env filtering, access
control), option 2 is not acceptable. The test must go through the
production entry point, or through the CLI binary end-to-end.

## Good pattern

```rust
// Calls the actual CLI binary — same path a real user takes
Command::cargo_bin("reeve")
    .env("REEVE_SECRET", "should-not-leak")
    .arg("run").arg("script.rhai")
    .assert().stdout(predicate::str::contains("should-not-leak").not());
```

## Bad pattern

```rust
// Calls a test-only wrapper — does NOT verify the production path
run_exec_with_passthrough(pact, "printenv", &[], false, None, &["PATH"]);
```

## Origin

Milestone 2 — SF-2 (child env leak). Fixed in test helper; production
path left broken. Required a second fix round after user caught it.
