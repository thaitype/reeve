# Milestone 2 — Binary Measurement Report

Date: 2026-05-13
Platform: macOS Darwin 25.2.0, arm64
Build: `cargo build --release`

## Binary size

| Metric | Value | Target | Pass? |
|--------|-------|--------|-------|
| `target/release/reeve` | 5.0 MB | < 10 MB | YES |

Command used:
```
ls -la target/release/reeve
.rwxr-xr-x@ 5.0M thada 13 May 08:02 target/release/reeve
```

## Cold start (examples/noop.rhai)

Command: `time ./target/release/reeve run examples/noop.rhai`

| Run | Wall time |
|-----|-----------|
| 1   | 19 ms     |
| 2   | 6 ms      |
| 3   | 6 ms      |

Median: **6 ms** (Target: < 50 ms — PASS)

Note: Run 1 is slightly higher due to OS disk cache warm-up. Subsequent runs
reflect the true steady-state cold-start after the binary is in disk cache.

## Summary

Both targets met. No action required.
