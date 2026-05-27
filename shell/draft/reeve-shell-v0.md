# resh — v0 Increment (slimmed from v0.1 spec)

> Slimmed from `reeve-shell.md`. The spine: NL → AI → Rhai → `reeve-flex check`/`run`.
> Everything interactive or pact-mutating is deferred.

## Core

**Take NL on the CLI, ask an AI subprocess for a Rhai script, run it through
`reeve-flex check` then `run`. On failure, exit with a clear message and let
the human fix the pact by hand.**

## Ground state

- `shell/` is a `tsx`/`tsup` starter; `src/main.ts` is placeholder.
- `@reeve/sh` package name reserved; CLI binary will be `resh`.
- No reeve-flex prerequisite needed for v0 — `--no-check` is deferred along with "Approve once".

## Filesystem (v0)

```
$HOME/.resh/
├── config.json          # ai_command, reeve_flex_path
├── pact.yaml            # user-maintained; resh never writes it in v0
└── run/
    └── <run-id>/
        └── script.rhai
```

Run-id: `YYYYMMDD-HHMMSS-<4 random chars>`. Directory persists; no cleanup.

## Config

`$HOME/.resh/config.json`:

```json
{
  "ai_command": ["claude", "-p"],
  "reeve_flex_path": "reeve-flex"
}
```

No env-var overrides in v0. Pact path is hardcoded to `$HOME/.resh/pact.yaml`.

## CLI

```
resh "<natural language request>"
resh --help
resh --version
```

No subcommands, no reserved flags.

## Flow

```
resh "list pods in prod that aren't Running"
  │
  ├─► Load config. Mint run-id. Create $HOME/.resh/run/<run-id>/.
  ├─► Build prompt: bundled primer + NL + "Output ONLY the Rhai script."
  ├─► Spawn AI subprocess. Capture stdout. Strip ```rhai / ```rust / ``` fences.
  │     Empty output → exit 11.
  │     AI non-zero exit → print stderr → exit 10.
  ├─► Write script.rhai. Print it to the terminal (plain, no color).
  ├─► reeve-flex check <script> --pact $HOME/.resh/pact.yaml
  │     ✗ → print stderr → exit with check's code (offset to 4 if structural).
  ├─► reeve-flex run <script> --pact $HOME/.resh/pact.yaml
  │     Stream stdio. Passthrough exit code.
  └─► Done.
```

No menu, no retry, no pact mutation. If `check` fails, the user reads the
error, edits `pact.yaml` by hand, and re-runs.

## Exit codes (v0)

| Condition                                | Exit |
| ---------------------------------------- | ---- |
| AI subprocess non-zero                   | 10   |
| AI empty / non-Rhai output               | 11   |
| Config / pact unreadable / `check` errors structurally | 4 |
| `reeve-flex run` exits non-zero          | passthrough |
| Success                                  | 0    |

## Repository structure

```
shell/
├── package.json
├── tsconfig.json
├── src/
│   ├── cli.ts          # arg parsing + top-level orchestration
│   ├── config.ts       # read config.json, apply defaults
│   ├── runid.ts        # YYYYMMDD-HHMMSS-xxxx + run dir
│   ├── prompt.ts       # primer + NL + closing instruction
│   ├── ai.ts           # spawn AI subprocess, capture, fence-strip
│   ├── reeve.ts        # reeve-flex check / run spawn
│   └── errors.ts       # exit-code enum + printer
├── prompts/
│   └── primer.md       # bundled Rhai/Reeve primer (~200 lines)
└── test/
    ├── prompt.test.ts  # snapshot
    ├── ai.test.ts      # fence-strip, empty output
    └── reeve.test.ts   # mocked-spawn exit-code passthrough
```

`violation.ts` and `pact.ts` are intentionally absent — they land with the
deferred items.

## Dependencies

```json
{
  "dependencies": { "execa": "^9" }
}
```

Dropped from v0.1: `kleur`, `prompts`, `yaml`. Each comes back with the
feature that needs it.

## Done when

- `resh "list files in cwd"` with a permissive `pact.yaml` produces a script,
  passes `check`, runs, prints output, exits 0.
- With a restrictive `pact.yaml`, resh prints reeve-flex's stderr and exits
  non-zero — no interactive menu.
- Unit tests green (`prompt.test.ts`, `ai.test.ts`, `reeve.test.ts`).
- `tsup` build emits ESM + CJS + types.
- `bun install -g .` (or `npm i -g .`) puts `resh` on PATH.

## Deferred (room left in v0 layout)

| Feature | How it lands later |
| --- | --- |
| Interactive violation loop (Approve once / Add to pact / Retry / Abort) | New `violation.ts`; `cli.ts` already branches on `check` exit code |
| Add to pact (YAML round-trip + backups + unified-diff confirm) | New `pact.ts`; pulls in `yaml` dep |
| AI retry loop (max 2 attempts, appended stderr) | Extend `prompt.ts` + new path in `cli.ts` |
| `reeve-flex run --no-check` prerequisite | Only needed when "Approve once" lands |
| `resh pact show` / `resh pact edit` / `resh config` | New arg branches in `cli.ts` |
| Env-var overrides (`resh_AI_COMMAND`, `resh_REEVE_FLEX`) | Extend `config.ts` |
| Interactive picker (`prompts`) + colored output (`kleur`) | Re-add deps with violation loop |

## Out (v0 and v0.1 both)

- Reserved `-y` / `--dry-run` no-op flags — re-add when they mean something.
- Audit log, daemon, REPL, multi-pact stacking, auto-cleanup.
- Rust port (post-spec future item).
