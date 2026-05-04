# Contract — Pact YAML Schema (milestone 1 subset)

Authoritative schema for pact files in milestone 1. Loaded via `serde_yaml`.
Anything not declared here is **rejected at parse time** (no extra-field
permissiveness).

## Top-level

```yaml
version: 1                          # u32, must equal 1
name: <string>                      # required, [a-z0-9-]+
description: <string>               # required, free text
defaults:                           # required
  timeout_seconds: <u32>            # default per-exec timeout
  max_output_bytes: <u64>           # default stdout+stderr cap
binaries:                           # required, map<name, BinarySpec>
  <name>: <BinarySpec>
```

## BinarySpec

```yaml
path:                               # required, see D1 in _goal/02-decisions.md
  default: [<abs-path>, ...]        # optional
  linux:   [<abs-path>, ...]        # optional
  macos:   [<abs-path>, ...]        # optional
  # at least one of {default, linux, macos} must be present

# EITHER subcommands OR top-level (allowed_flags / flag_values / positional)
# — never both. Validated at parse time.

subcommands:                        # optional
  <subcmd>:
    allowed_flags: [<string>, ...]  # optional, defaults to []
    flag_values:                    # optional
      "<flag>": <KindSpec>
    positional:                     # optional, ordered list
      - <PositionalSpec>

allowed_flags: [<string>, ...]      # optional, top-level (no-subcommand binaries)
flag_values:                        # optional, top-level
  "<flag>": <KindSpec>
positional:                         # optional, top-level
  - <PositionalSpec>
```

Path strings under `path.*` MUST be absolute (start with `/`). Validation
runs at YAML parse time; non-absolute paths → parse error.

## KindSpec

Tagged union via `kind:` discriminator. Milestone 1 supports exactly:

```yaml
{ kind: enum,   values: [<string>, ...] }   # exact-match
{ kind: number }                            # non-negative integer (u64)
{ kind: string }                            # see PositionalSpec below
```

## PositionalSpec

```yaml
{ kind: <KindSpec>, optional?: bool, repeated?: bool }
```

`optional` and `repeated` default to `false`. A `repeated` positional MUST
be the last in the list.

## `string` kind validation

Reject if any of these characters appear in the value:

| Char  | Reason                       |
|-------|------------------------------|
| `\0`  | null byte                    |
| `;`   | command separator            |
| `&`   | background / chain           |
| `\|`  | pipe                         |
| `$`   | env interpolation            |
| `` ` `` | command substitution      |
| `<`   | redirect                     |
| `>`   | redirect                     |
| `\n`  | newline                      |
| `\r`  | carriage return              |

Returned error: `PositionalRejected` or `FlagValueRejected` with the offending
character named in the message.

## Embedded preset (milestone 1)

`pacts/linux-readonly.yaml`:

```yaml
version: 1
name: linux-readonly
description: Basic POSIX info commands — no side effects
defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576    # 1 MiB

binaries:
  echo:
    path:
      default: [/bin/echo]
    allowed_flags: [-n, -e]
    positional:
      - { kind: { kind: string }, repeated: true }

  date:
    path:
      default: [/bin/date]
    allowed_flags: [-u, -I, -R]
    positional:
      - { kind: { kind: string }, optional: true, repeated: true }

  uname:
    path:
      default: [/usr/bin/uname]
    allowed_flags: [-a, -s, -r, -m, -n, -p]

  whoami:
    path:
      default: [/usr/bin/whoami]

  hostname:
    path:
      linux: [/bin/hostname, /usr/bin/hostname]
      macos: [/bin/hostname]
    allowed_flags: [-s, -f]
```

## Test-only pact (NOT embedded in release)

`crates/warden-pact/tests/fixtures/test-fixtures.yaml` — included via `#[cfg(test)]`. Lives under the test crate's own `tests/` tree (not workspace `pacts/`) so reviewers and any future `pacts/**` distribution glob never see it as production policy:

```yaml
version: 1
name: test-fixtures
description: Binaries used only to exercise executor safety rails
defaults:
  timeout_seconds: 1
  max_output_bytes: 4096

binaries:
  sleep:
    path:
      default: [/bin/sleep]
    positional:
      - { kind: { kind: number } }

  yes:
    path:
      default: [/usr/bin/yes, /bin/yes]
    positional:
      - { kind: { kind: string }, optional: true, repeated: true }
```
