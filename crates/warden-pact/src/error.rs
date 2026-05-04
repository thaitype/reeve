use std::path::PathBuf;
use thiserror::Error;

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
