# Publishing a Release

How to cut a new Reeve release to crates.io and GitHub Releases.

The pipeline is **manually triggered** (`workflow_dispatch`). It does not run on
push or tag. Defined in [`.github/workflows/publish.yml`](../.github/workflows/publish.yml).

## What the pipeline does for you (automatic)

When run with `dry_run: false`:

1. **gate** — `cargo test` + `cargo clippy --all-targets -- -D warnings` on Linux and macOS.
2. **publish** — verifies `Cargo.toml`'s version matches the `version` input, then `cargo publish` to crates.io.
3. **release-binaries** — builds release binaries for `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`, then **creates the git tag `v<version>` and the GitHub Release** with the binaries attached (via `softprops/action-gh-release`).

You do **not** tag or create a GitHub Release by hand — the pipeline does both. The
tag is created at whatever commit you dispatch the workflow from, so dispatch from
the `main` commit you want released.

## What you do by hand (before triggering)

The pipeline does not bump the version. You must do this first, on a branch, and
merge it to `main`:

1. Bump the version in `Cargo.toml`.
2. **Sync `Cargo.lock` in the same commit.** Editing `Cargo.toml` alone leaves the
   lock stale; the next cargo run rewrites it and the dirty tree fails
   `cargo publish --dry-run` (exit 101). Either:
   ```sh
   cargo update -p reeve --precise <version>   # surgical: updates just the reeve entry
   # or
   cargo set-version <version>                 # needs cargo-edit; bumps Cargo.toml + lock together
   ```
   Then commit **both** files:
   ```sh
   git add Cargo.toml Cargo.lock && git commit -m "chore: release v<version>"
   ```
3. Open a PR and merge to `main`.

Verify locally that a build leaves the tree clean before merging:
```sh
cargo build && git diff --exit-code Cargo.lock   # must print nothing
```

## Triggering the pipeline

GitHub → **Actions** → **publish** → **Run workflow**:

- **Branch:** `main` (the merged release commit).
- **version:** e.g. `0.2.0` — must equal `Cargo.toml`'s version or the job fails.
- **dry_run:**
  - `true` (default) — runs gate + `cargo publish --dry-run` only. No crates.io upload, no tag, no release. Use this to validate first.
  - `false` — full release: publishes to crates.io, tags `v<version>`, creates the GitHub Release.

Recommended flow: run once with `dry_run: true` to confirm it's green, then run
again with `dry_run: false` to cut the release.

## Prerequisites

- `CARGO_REGISTRY_TOKEN` secret must be set in the repo (crates.io publish token).
- `GITHUB_TOKEN` is provided automatically by Actions (used for the release upload).

## Troubleshooting

- **`cargo publish --dry-run` fails with "files in the working directory contain
  changes ... Cargo.lock" (exit 101):** the committed `Cargo.lock` is out of sync
  with `Cargo.toml`. Run `cargo update -p reeve --precise <version>`, commit the
  lock, and re-run. Do **not** add `--allow-dirty` — that only hides a stale lock.
- **"Cargo.toml version X != input Y":** the `version` input doesn't match
  `Cargo.toml`. Fix one to match the other.
