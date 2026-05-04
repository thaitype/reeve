use std::path::PathBuf;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Parse-time errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unsupported pact version; only version 1 is supported")]
    UnsupportedVersion,

    #[error("binary `{binary}`: path spec must have at least one of `default`, `linux`, or `macos`")]
    PathSpecEmpty { binary: String },

    #[error("binary `{binary}`: path `{path}` is not absolute (must start with `/`)", path = path.display())]
    PathNotAbsolute { binary: String, path: PathBuf },

    #[error("binary `{binary}`: `subcommands` cannot be combined with `allowed_flags`, `flag_values`, or `positional`")]
    SubcommandsConflictWithDirect { binary: String },

    #[error("binary `{binary}`{}: a `repeated: true` positional must be the last in the list", subcommand.as_deref().map(|s| format!(", subcommand `{s}`")).unwrap_or_default())]
    RepeatedNotLast {
        binary: String,
        subcommand: Option<String>,
    },

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

// ---------------------------------------------------------------------------
// Runtime / validate-call errors
//
// Field names match the Rhai error-map keys in _contract/02-host-fns.md §Throws
// exactly, so task-6 can convert PactError → Rhai map mechanically.
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum PactError {
    /// `binary` is not listed in the active pact.
    #[error("binary `{binary}` is not allowed by the active pact")]
    BinaryNotAllowed { binary: String },

    /// `binary` is in the pact but none of its listed paths exist on disk.
    ///
    /// The contract calls this `BinaryNotFound` at *engine startup*, but since
    /// milestone-1 resolves paths at validate-call time (no separate startup
    /// phase), we surface the same condition here as `BinaryNotResolvable`.
    /// task-6 should map this variant to the `BinaryNotFound` Rhai error kind
    /// for script-visible consistency.
    #[error("binary `{binary}` is in the pact but none of its listed paths exist on disk")]
    BinaryNotResolvable { binary: String },

    /// `binary` uses subcommands and the first arg is not a known subcommand.
    #[error("subcommand `{subcommand}` is not allowed for binary `{binary}`")]
    SubcommandNotAllowed { binary: String, subcommand: String },

    /// A flag argument is not in the binary's `allowed_flags` list.
    #[error("flag `{flag}` is not allowed for binary `{binary}`")]
    FlagNotAllowed { binary: String, flag: String },

    /// A flag's value failed kind validation.
    #[error("flag `{flag}` value `{value}` rejected for binary `{binary}`: {reason}")]
    FlagValueRejected {
        binary: String,
        flag: String,
        value: String,
        reason: String,
    },

    /// A positional argument failed kind validation, or an extra positional was
    /// supplied when no positional spec remains.
    #[error(
        "positional argument at index {index} (`{value}`) rejected for binary `{binary}`: {reason}"
    )]
    PositionalRejected {
        binary: String,
        index: usize,
        value: String,
        reason: String,
    },
}
