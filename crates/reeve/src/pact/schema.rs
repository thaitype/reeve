use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pact {
    pub version: u32,
    pub name: String,
    pub description: String,
    pub defaults: Defaults,
    pub binaries: BTreeMap<String, BinarySpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    pub timeout_seconds: u32,
    pub max_output_bytes: u64,
}

/// Flat deserialisation helper — captures all possible fields from the binary
/// body so we can check for the `subcommands` + direct XOR after parsing.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BinarySpec {
    pub path: PathSpec,
    #[serde(flatten)]
    pub body: BinaryBody,
}

/// Either `subcommands:` map **or** the direct action fields.  We use a flat
/// helper during deserialisation so that `deny_unknown_fields` on the parent
/// still works, then validate the mutual-exclusion rule in `parse.rs`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryBodyRaw {
    #[serde(default)]
    pub subcommands: Option<BTreeMap<String, ActionSpec>>,
    #[serde(default)]
    pub allowed_flags: Vec<String>,
    #[serde(default)]
    pub flag_values: BTreeMap<String, KindSpec>,
    #[serde(default)]
    pub positional: Vec<PositionalSpec>,
}

/// The validated, typed representation of the binary body.
#[derive(Debug, Clone, Serialize)]
pub enum BinaryBody {
    WithSubcommands(BTreeMap<String, ActionSpec>),
    Direct(ActionSpec),
}

impl<'de> Deserialize<'de> for BinaryBody {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = BinaryBodyRaw::deserialize(deserializer)?;

        let has_subcommands = raw.subcommands.is_some();
        let has_direct = !raw.allowed_flags.is_empty()
            || !raw.flag_values.is_empty()
            || !raw.positional.is_empty();

        if has_subcommands && has_direct {
            // We cannot emit ParseError here (wrong type), so we use a serde
            // custom error.  The post-parse validator in parse.rs also checks
            // this, but having it here gives a cleaner message on raw
            // deserialise calls.
            return Err(serde::de::Error::custom(
                "`subcommands` cannot be combined with `allowed_flags`, `flag_values`, or `positional`",
            ));
        }

        if let Some(subcmds) = raw.subcommands {
            Ok(BinaryBody::WithSubcommands(subcmds))
        } else {
            Ok(BinaryBody::Direct(ActionSpec {
                allowed_flags: raw.allowed_flags,
                flag_values: raw.flag_values,
                positional: raw.positional,
            }))
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionSpec {
    #[serde(default)]
    pub allowed_flags: Vec<String>,
    #[serde(default)]
    pub flag_values: BTreeMap<String, KindSpec>,
    #[serde(default)]
    pub positional: Vec<PositionalSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PathSpec {
    pub default: Option<Vec<PathBuf>>,
    pub linux: Option<Vec<PathBuf>>,
    pub macos: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum KindSpec {
    Enum { values: Vec<String> },
    Number,
    #[serde(rename = "string")]
    String_,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PositionalSpec {
    pub kind: KindSpec,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub repeated: bool,
}
