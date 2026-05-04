# Backlog

Cross-milestone parking lot. Items here are **not yet planned** into a
milestone — they are signals captured so they don't get lost. Promote
to a milestone TODO via `/chief-plan` when ready to pick one up.

Lightweight format: one bullet per item. Add reason + rough scope only
if the trigger isn't obvious from the title. Don't add verification
steps here — those belong in the task spec when the item is planned in.

## Engineering

- **GitHub Actions cross-platform CI** — matrix
  `{ubuntu-latest, macos-latest}` × stable Rust running build, test,
  clippy. Closes the gap between milestone-1's "Linux AND macOS" goal
  and what was actually verified locally (macOS only). Source: retro
  R5.

- **Migrate off archived `serde_yaml` 0.9** — the original dtolnay
  crate is archived. Move to the actively-maintained successor (the
  YAML org's `serde_yaml` 0.10, or `serde_yml`/`serde_yaml_ng` —
  confirm exact crate name before planning). Touch points: the custom
  `Deserialize` for `BinaryBody` in `warden-pact::parse`, and the
  `parse_yaml` host fn in `warden-core::parse`.

## Done (kept for traceability)

- ~~Rename `linux-readonly` → `unix-readonly`~~ — completed in
  batch 10.1 (commit `62bc7d3`).
