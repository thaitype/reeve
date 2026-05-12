# resh — Design Spec (v0.1)

> **resh** /rɛʃ/ (rhymes with  *mesh* ,  *fresh* ) is the CLI binary shipped
> by the **@reeve/sh** package. It turns natural language into a Rhai
> script and hands it to `reeve-flex` for execution. Modelled after
> [BuilderIO/ai-shell](https://github.com/BuilderIO/ai-shell), but the
> allowlist is Reeve's pact — not a separate prefix list.
>
> The short name `resh` (CLI) follows the shell-tool naming pattern of
> `bash`, `zsh`, `fish`, `dash`, `ash`. The long name `@reeve/sh`
> (package) makes the relationship to Reeve explicit on the registry.

published under @reeve on npm with package name `@reeve/sh`

resh is a thin TypeScript wrapper that turns natural language into a Rhai
script the user trusts to run. It does four things:

1. Take a natural-language request from the user.
2. Spawn an AI CLI subprocess (e.g. `claude -p "..."`) and read back a
   Rhai script. **The active pact is NOT sent to the AI** — the AI writes
   Rhai from its own knowledge; the pact catches what slips through.
3. Stage the script under `$HOME/.resh/run/<run-id>/script.rhai` and call
   `reeve-flex check`. If it passes, call `reeve-flex run` immediately —
   no human approval prompt.
4. If `check` fails, the user picks: approve once, add to pact
   permanently, retry (let AI fix), or abort.

resh never re-implements pact validation. Reeve's `reeve-flex` is the
single enforcement layer; resh is plumbing.

```
┌──────────┐  NL request   ┌────────┐  prompt   ┌──────────┐
│   user   │ ────────────► │ resh  │ ────────► │  AI CLI  │
└──────────┘               │  (TS)  │ ◄──────── │ (claude) │
                           └───┬────┘  Rhai     └──────────┘
                               │
                               │ script + $HOME/.resh/pact.yaml
                               ▼
                       ┌──────────────────┐
                       │   reeve-flex     │
                       │  check  ──► run  │
                       └──────────────────┘
                               │ violation
                               ▼
                       ┌──────────────────┐
                       │  user picks:     │
                       │  approve once /  │
                       │  add to pact /   │
                       │  retry / abort   │
                       └──────────────────┘
```

## Persona & trust model

resh sits in a different trust axis from `reeve` and `reeve-flex`:

| Binary              | Invoker                            | Trust boundary      | Pact source                           |
| ------------------- | ---------------------------------- | ------------------- | ------------------------------------- |
| `reeve`           | AI agent (autonomous)              | binary itself       | compile-time embed                    |
| `reeve-flex`      | Developer / orchestrator (MCP, CI) | the caller          | runtime `--pact`                    |
| **`resh`** | **Human user at terminal**   | **the human** | `$HOME/.resh/pact.yaml` (editable) |

The human is the trust boundary. The human edits their own pact, the
human sees violations as they happen, the human decides whether to widen
the pact or abort. AI generation is convenience; the pact is the
guarantee.

## Goals

- **Zero allowlist logic in resh.** Pact handling lives in `reeve-flex`.
  resh reads the pact only when the user explicitly extends it on a
  violation.
- **No pact in the system prompt.** The AI writes Rhai from its own
  knowledge of Reeve's host functions and common CLI tools. The pact
  catches what slips through. Lower tokens, cleaner separation.
- **AI-CLI agnostic.** The AI provider is invoked as a subprocess command
  the user configures. Default: `claude -p`. Anything that takes a prompt
  on stdin/argv and prints a response on stdout works.
- **Trust the pact, not the human review.** A passing `reeve-flex check`
  is sufficient to proceed — resh does NOT prompt for y/N after a clean
  check. The pact is one-time, up-front approval; per-script approval
  would be redundant noise.
- **Stay thin.** No daemon, no audit log, no history feature in v0.1.

## Non-goals (v0.1)

- Implementing pact validation. That's `reeve-flex check`.
- Audit logging. The natural artifact (`$HOME/.resh/run/<run-id>/`)
  serves as informal history; no JSONL audit stream.
- Owning credentials for any AI provider. The configured AI CLI handles
  its own auth.
- Interactive REPL or multi-turn refinement. Each `resh "..."` is
  one-shot.
- Editing the generated script before run. v0.1 trusts the pact + retry
  loop.
- Auto-cleanup of `$HOME/.resh/run/`. User cleans up manually; a
  `resh clean` command may land later.
- Multi-pact stacking. v0.1 = exactly one `pact.yaml`.

## Prerequisites

One change to `reeve-flex` must land before resh ships:

- **`reeve-flex run --no-check <script>`** — a flag that runs the script
  without re-validating against the pact. Used by resh's "approve once"
  path. Without this, "approve once" would require resh to synthesize a
  temporary permissive pact, which is more invasive.

`--script-stdin` and `--pact-stdin` already in the Reeve spec are not
strictly required — resh works with file paths.

## Filesystem layout

```
$HOME/.resh/
├── config.json                     # ai_command, reeve_flex_path
├── pact.yaml                       # the active pact (single file)
├── pact.yaml.bak.<timestamp>       # written before any "add to pact"
└── run/
    └── <run-id>/                   # YYYYMMDD-HHMMSS-<4charsuffix>
        └── script.rhai             # AI-generated, last attempt of this run
```

Notes:

- **Single pact file** — no `pacts/` directory. Multi-pact stacking is a
  future feature; one file keeps the mental model simple and round-trip
  YAML editing trivial.
- **`run/<run-id>/` mirrors `reeve`'s audit directory layout** — same
  shape, easier to read both projects' code side by side. The directory
  is not removed after a run; it accumulates and the user prunes
  manually.
- **Run-id format:** `YYYYMMDD-HHMMSS-<4 random chars>`. Sortable with
  `ls -lt`, human-readable, collision-safe for sub-second invocations.
- **Retry overwrites `script.rhai`** in the same run directory. v0.1
  keeps only the final attempt; future versions may keep `attempt-1.rhai`,
  `attempt-2.rhai`, etc.

## Configuration

Single config file: `$HOME/.resh/config.json`.

```json
{
  "ai_command": ["claude", "-p"],
  "reeve_flex_path": "reeve-flex"
}
```

- `ai_command` — argv prefix. resh appends the assembled prompt as the
  final positional arg.
- `reeve_flex_path` — binary lookup. Default `reeve-flex` (PATH).

The pact path is **not** configurable — it is hardcoded to
`$HOME/.resh/pact.yaml`. There is one pact; making the path tunable
invites confusion ("which pact am I editing?").

Env var overrides: `resh_AI_COMMAND`, `resh_REEVE_FLEX`.

resh has no API keys, no secrets, no provider config of its own. If the
configured AI CLI needs auth, the user sets it up there.

## CLI

```
resh "<natural language request>"

  Reserved flags (no-op in v0.1, accepted for forward compat):
    -y, --yes               accepted; current flow has no default prompt
    --dry-run               accepted; current flow has no preview-only mode

  Real flags:
    --help, --version

resh pact show             cat $HOME/.resh/pact.yaml
resh pact edit             open the pact in $EDITOR
resh config                show effective config
```

There are no flags to override the pact, the AI command, or the
reeve-flex path at invocation time in v0.1. Edit `config.json` or set the
env vars.

## Flow

```
resh "list pods in prod that aren't Running"
  │
  ├─► Load config. Mint a run-id. Create $HOME/.resh/run/<run-id>/.
  │
  ├─► Build system prompt:
  │     - Static Rhai/Reeve primer (bundled, ~200 lines)
  │     - "User request:" <the NL string>
  │     - "Output ONLY the Rhai script. No prose, no fences."
  │
  ├─► Spawn AI subprocess: claude -p "<system prompt>"
  │     Capture stdout. Strip stray ```rhai / ```rust / ``` fences.
  │     Empty/garbage output → exit 11.
  │
  ├─► Write script to $HOME/.resh/run/<run-id>/script.rhai
  │     Print the script to the terminal (syntax-highlighted if TTY).
  │
  ├─► spawn: reeve-flex check <script-path> --pact $HOME/.resh/pact.yaml
  │     ✓ pass → fall through to run
  │     ✗ fail → enter the violation loop (below)
  │
  ├─► spawn: reeve-flex run <script-path> --pact $HOME/.resh/pact.yaml
  │     Stream stdout/stderr to terminal as-is.
  │     Exit with reeve-flex's exit code.
  │
  └─► (run directory remains on disk; no cleanup)
```

### Violation loop

When `reeve-flex check` fails, resh shows the violation and asks the
user what to do. The prompt is a regular interactive arrow-key picker
(via the `prompts` library), not letter shortcuts:

```
⚠  pact violation in $HOME/.resh/pact.yaml

   AI wants to run:
     kubectl get pods -A -o json

   Violated rule:
     flag `-A` (--all-namespaces) not allowed on `kubectl get`

   ? What would you like to do? (Use arrow keys)
   ❯ Approve once
     Add to pact permanently
     Retry — ask AI to fix
     Abort
```

Action handlers:

| Action                            | Behavior                                                                                                                                                                                                                                                                                          |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Approve once**            | `reeve-flex run --no-check <script> --pact $HOME/.resh/pact.yaml`. Script runs without re-validation. No log; the script file in `run/<run-id>/` is the only artifact.                                                                                                                       |
| **Add to pact permanently** | Round-trip-parse `pact.yaml` (preserving comments + key order), apply the minimum edit that would let the violating script pass, write `pact.yaml.bak.<ISO timestamp>`, show a unified diff, ask the user to confirm the diff, then write the new `pact.yaml`. Re-run `check` → `run`. |
| **Retry**                   | Re-prompt the AI with the previous script + the `reeve-flex check` stderr appended. Up to 2 retries; afterward, abort with exit 3.                                                                                                                                                              |
| **Abort**                   | Exit 0. Run directory remains on disk.                                                                                                                                                                                                                                                            |

### Auto-add scope

When the user picks "Add to pact permanently," resh attempts to expand
the pact for any level of violation:

- ✅ **Add allowed flag** (e.g. `--all-namespaces`, `-A`).
- ✅ **Infer flag_values kind** (best-effort: `bool` for valueless flags,
  fall back to a permissive kind for valued flags; the user can tighten
  via `resh pact edit` afterward).
- ✅ **Expand a positional enum** (e.g. add `secrets` to the kinds list
  for `kubectl get`).
- ✅ **Add a subcommand** (e.g. add `apply` under `kubectl`).
- ✅ **Add a binary** (e.g. add `helm`).

Every level is supported in v0.1; the user is the trust boundary, and
the confirm-diff step makes the change explicit. No
typed-`yes`/double-confirm gating beyond the standard diff confirmation.

### YAML mutation strategy

resh must edit `pact.yaml` without losing comments or reordering keys
unrelated to the change. Two requirements:

1. Use a YAML library that supports comment-preserving round-trip. In
   TypeScript: the `yaml` package's `parseDocument` / `Document.toString`
   APIs.
2. Write `pact.yaml.bak.<ISO timestamp>` before any mutation. Keep all
   backups in v0.1 — no rotation/pruning yet.

Concurrent mutation is not guarded against in v0.1 (no lockfile).
Concurrent resh invocations editing the same pact at the same time is
considered out of scope until it becomes a real problem.

## System prompt construction

The prompt sent to the AI CLI has three sections, concatenated:

1. **Static primer** (~200 lines, bundled with resh): Rhai language
   quickref + Reeve's host function signatures (`exec`, `pipe`,
   `parse_json`, `read_file`, etc.) + 2–3 worked examples that exercise
   the common shapes (one-shot exec, pipe stage, parse JSON, write
   workspace file).
2. **User request**: the raw NL string passed to resh.
3. **Closing**: `Output ONLY the Rhai script. No markdown fences, no commentary.`

The pact is NOT included. The AI relies on its training-data knowledge
of common CLIs (`kubectl`, `git`, `jq`, etc.) and the bundled primer for
Reeve's host fn surface. Violations are caught by `reeve-flex check` and
surfaced to the user.

When the AI returns the script with stray markdown fences anyway (it
will), resh strips a leading ``rhai / ``rust / ``and trailing``
before writing `script.rhai`.

## Retry behavior

On `reeve-flex check` failure → "Retry" selection, resh re-prompts the
AI with the original system prompt plus an appended block:

```
# PREVIOUS ATTEMPT FAILED VALIDATION
<the script that failed>

# ERROR FROM reeve-flex check
<stderr from check>

Produce a corrected Rhai script that addresses the error above.
Output ONLY the script.
```

Maximum 2 retries per user request. After that, abort with exit 3 and
print: "Retry limit reached. Use 'Approve once' or 'Add to pact' if you
trust this script, or 'Abort' to give up."

## Error handling

| Condition                                                            | Exit code     | Behavior                          |
| -------------------------------------------------------------------- | ------------- | --------------------------------- |
| AI subprocess fails (non-zero exit)                                  | 10            | Print AI stderr, abort.           |
| AI returned empty / non-Rhai output                                  | 11            | Print received bytes, abort.      |
| Retry limit reached                                                  | 3             | Print last error, abort.          |
| User picks "Abort" at violation prompt                               | 0             | Clean exit.                       |
| `reeve-flex check` errors structurally (parse error, missing pact) | 4             | Print error, abort.               |
| `reeve-flex run` exits non-zero                                    | (passthrough) | Forward reeve-flex's exit code.   |
| Config missing / malformed                                           | 4             | Print path + parse error.         |
| `pact.yaml` unreadable or malformed                                | 4             | Print path + parse error.         |
| YAML mutation fails (e.g. unparseable change)                        | 5             | Restore from backup, print error. |

resh's own exit codes are deliberately offset from `reeve-flex`'s
(0–5) so users can distinguish "resh failed" from "the script failed."

## Repository structure

```
resh/
├── package.json
├── tsconfig.json
├── README.md
├── LICENSE
│
├── src/
│   ├── cli.ts                 # arg parsing, top-level orchestration
│   ├── config.ts              # config.json + env merge
│   ├── runid.ts               # YYYYMMDD-HHMMSS-xxxx generation + run dir mgmt
│   ├── prompt.ts              # system prompt assembly
│   ├── ai.ts                  # AI subprocess spawn, stdout capture, fence strip
│   ├── reeve.ts               # reeve-flex check/run/--no-check spawn wrappers
│   ├── violation.ts           # interactive picker + action handlers
│   ├── pact.ts                # YAML round-trip parse + minimum-edit synthesis
│   └── errors.ts              # exit-code enum + formatted printer
│
├── prompts/
│   └── primer.md              # static Rhai/Reeve primer (bundled at build)
│
└── test/
    ├── prompt.test.ts         # system prompt assembly snapshot
    ├── ai.test.ts             # fence-stripping, empty-output handling
    ├── reeve.test.ts          # exit-code passthrough (mock spawn)
    ├── pact.test.ts           # YAML round-trip + comment preservation
    ├── violation.test.ts      # auto-add edit synthesis for each violation kind
    └── e2e.test.ts            # full flow against a stub reeve-flex
```

## Dependencies

Minimal. Target Bun runtime, but stay portable to Node 20+.

```json
{
  "dependencies": {
    "execa": "^9",
    "kleur": "^4",
    "prompts": "^2",
    "yaml": "^2"
  },
  "devDependencies": {
    "typescript": "^5",
    "@types/node": "^20",
    "vitest": "^2"
  }
}
```

- `execa` — subprocess spawn for the AI CLI and `reeve-flex`.
- `kleur` — minimal ANSI coloring for script preview, prompts, diffs.
- `prompts` — interactive arrow-key picker for the violation menu;
  degrades cleanly to non-TTY (defaults to abort).
- `yaml` — comment-preserving round-trip for `pact.yaml` mutation.

No AI SDK. No diff library — built-in unified diff is fine for the
violation confirm step (short edits only).

## Success criteria

- One-line install: `bun install -g resh` (or `npm i -g resh`).
- End-to-end happy path under 3 seconds when the AI CLI is fast.
- Generated script always goes through `reeve-flex check` before run.
- YAML round-trip preserves all comments and key order on every test
  pact in the suite.
- Test coverage > 70% on `src/`, with snapshot tests for prompt assembly
  and mocked-spawn tests for the orchestration flow.
- README walks a new user from `bun install` to first successful run in
  under 5 minutes.

## Future (post-v0.1)

1. **Rust port into the Reeve workspace** — once prompt + UX converge,
   port resh to a Rust crate alongside `reeve` and `reeve-flex`,
   calling the engine in-process. Trade-off accepted today: subprocess
   overhead + informal CLI contract, in exchange for fast iteration
   while the design is still moving.
2. **`resh clean`** — prune old `run/<run-id>/` directories by age or
   count.
3. **`-y` activation** — currently a no-op; will become meaningful if
   any default prompt is added (e.g. confirm-before-run mode).
4. **`--dry-run` activation** — currently a no-op; will print the script
   and skip both check and run.
5. **`--edit`** — open generated script in `$EDITOR` before check.
6. **Multi-attempt run dirs** — keep `attempt-1.rhai`, `attempt-2.rhai`,
   `violation-1.txt`, etc. alongside the final `script.rhai`.
7. **`resh explain <script.rhai>`** — reverse direction: ask the AI to
   explain an existing Rhai script in plain language.
8. **Multi-pact stacking** — accept multiple pacts; merge for the
   `reeve-flex` call.
9. **Optional pact summary in prompt** — when first-try success rate is
   measurably bad, ship a "binaries-and-subcommands-only" summary in the
   system prompt as an opt-in (`config.pact_hint = "summary"`). Not in
   v0.1 because the cost/benefit isn't measured yet.
