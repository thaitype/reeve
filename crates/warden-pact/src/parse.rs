use std::path::Path;

use crate::error::ParseError;
use crate::schema::{BinaryBody, Pact};

pub fn parse_pact(yaml: &str) -> Result<Pact, ParseError> {
    let pact: Pact = serde_yaml::from_str(yaml)?;

    // V1: version must equal 1
    if pact.version != 1 {
        return Err(ParseError::UnsupportedVersion);
    }

    for (binary_name, binary_spec) in &pact.binaries {
        // Path spec must have at least one populated OS array
        let ps = &binary_spec.path;
        if ps.default.is_none() && ps.linux.is_none() && ps.macos.is_none() {
            return Err(ParseError::PathSpecEmpty {
                binary: binary_name.clone(),
            });
        }

        // Every path in any OS array must be absolute
        for path in ps
            .default
            .iter()
            .flatten()
            .chain(ps.linux.iter().flatten())
            .chain(ps.macos.iter().flatten())
        {
            if !path_is_absolute(path) {
                return Err(ParseError::PathNotAbsolute {
                    binary: binary_name.clone(),
                    path: path.clone(),
                });
            }
        }

        // Subcommands + direct are already checked by BinaryBody::deserialize,
        // but we re-validate here to emit our typed ParseError variant.
        match &binary_spec.body {
            BinaryBody::WithSubcommands(subcmds) => {
                // Validate each subcommand's action spec
                for (subcmd_name, action) in subcmds {
                    validate_repeated_last(
                        binary_name,
                        Some(subcmd_name.as_str()),
                        &action.positional,
                    )?;
                }
            }
            BinaryBody::Direct(action) => {
                validate_repeated_last(binary_name, None, &action.positional)?;
            }
        }
    }

    Ok(pact)
}

fn path_is_absolute(path: &Path) -> bool {
    // We want the path to start with `/`.  PathBuf::is_absolute() works on the
    // host OS which might be Windows in CI, so check the raw bytes directly.
    path.to_str()
        .map(|s| s.starts_with('/'))
        .unwrap_or(false)
}

fn validate_repeated_last(
    binary: &str,
    subcommand: Option<&str>,
    positional: &[crate::schema::PositionalSpec],
) -> Result<(), ParseError> {
    let len = positional.len();
    for (i, pos) in positional.iter().enumerate() {
        if pos.repeated && i + 1 != len {
            return Err(ParseError::RepeatedNotLast {
                binary: binary.to_string(),
                subcommand: subcommand.map(str::to_string),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::BinaryBody;

    // -----------------------------------------------------------------------
    // Happy-path tests
    // -----------------------------------------------------------------------

    #[test]
    fn parses_unix_readonly_preset() {
        let yaml = r#"
version: 1
name: unix-readonly
description: Basic POSIX info commands — no side effects
defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576

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
"#;
        let pact = parse_pact(yaml).expect("should parse cleanly");

        // Must have 5 binaries
        assert_eq!(pact.binaries.len(), 5);

        // hostname has both linux and macos paths
        let hostname = &pact.binaries["hostname"];
        assert!(hostname.path.linux.is_some());
        assert!(hostname.path.macos.is_some());
        assert!(hostname.path.default.is_none());

        // whoami has no flags / positional
        let whoami = &pact.binaries["whoami"];
        match &whoami.body {
            BinaryBody::Direct(action) => {
                assert!(action.allowed_flags.is_empty());
                assert!(action.positional.is_empty());
            }
            _ => panic!("expected Direct body for whoami"),
        }

        // echo positional is repeated
        let echo = &pact.binaries["echo"];
        match &echo.body {
            BinaryBody::Direct(action) => {
                assert_eq!(action.positional.len(), 1);
                assert!(action.positional[0].repeated);
            }
            _ => panic!("expected Direct body for echo"),
        }
    }

    #[test]
    fn parses_test_fixtures_preset() {
        let yaml = r#"
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
"#;
        let pact = parse_pact(yaml).expect("should parse cleanly");
        assert_eq!(pact.binaries.len(), 2);

        let yes = &pact.binaries["yes"];
        match &yes.body {
            BinaryBody::Direct(action) => {
                assert_eq!(action.positional.len(), 1);
                assert!(action.positional[0].optional);
                assert!(action.positional[0].repeated);
            }
            _ => panic!("expected Direct body for yes"),
        }
    }

    // -----------------------------------------------------------------------
    // Reject-path tests
    // -----------------------------------------------------------------------

    #[test]
    fn rejects_unsupported_version() {
        let yaml = r#"
version: 2
name: bad
description: bad
defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576
binaries:
  echo:
    path:
      default: [/bin/echo]
"#;
        let err = parse_pact(yaml).unwrap_err();
        assert!(
            matches!(err, ParseError::UnsupportedVersion),
            "expected UnsupportedVersion, got: {err}"
        );
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let yaml = r#"
version: 1
name: bad
description: bad
defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576
binaries:
  echo:
    path:
      default: [/bin/echo]
forbidden_flags: []
"#;
        let err = parse_pact(yaml).unwrap_err();
        assert!(
            matches!(err, ParseError::Yaml(_)),
            "expected Yaml serde error, got: {err}"
        );
    }

    #[test]
    fn rejects_unknown_binary_field() {
        let yaml = r#"
version: 1
name: bad
description: bad
defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576
binaries:
  echo:
    path:
      default: [/bin/echo]
    mystery_field: true
"#;
        let err = parse_pact(yaml).unwrap_err();
        assert!(
            matches!(err, ParseError::Yaml(_)),
            "expected Yaml serde error, got: {err}"
        );
    }

    #[test]
    fn rejects_empty_path_spec() {
        let yaml = r#"
version: 1
name: bad
description: bad
defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576
binaries:
  echo:
    path: {}
"#;
        let err = parse_pact(yaml).unwrap_err();
        assert!(
            matches!(err, ParseError::PathSpecEmpty { .. }),
            "expected PathSpecEmpty, got: {err}"
        );
    }

    #[test]
    fn rejects_relative_path() {
        let yaml = r#"
version: 1
name: bad
description: bad
defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576
binaries:
  echo:
    path:
      default: [echo]
"#;
        let err = parse_pact(yaml).unwrap_err();
        assert!(
            matches!(err, ParseError::PathNotAbsolute { .. }),
            "expected PathNotAbsolute, got: {err}"
        );
    }

    #[test]
    fn rejects_subcommands_with_direct() {
        let yaml = r#"
version: 1
name: bad
description: bad
defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576
binaries:
  git:
    path:
      default: [/usr/bin/git]
    subcommands:
      log:
        allowed_flags: [--oneline]
    allowed_flags: [--version]
"#;
        let err = parse_pact(yaml).unwrap_err();
        // The BinaryBody custom deserialiser fires first — yields Yaml error
        // wrapping the custom message.
        assert!(
            matches!(err, ParseError::Yaml(_)),
            "expected Yaml serde error (subcommands conflict), got: {err}"
        );
    }

    #[test]
    fn rejects_repeated_not_last() {
        let yaml = r#"
version: 1
name: bad
description: bad
defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576
binaries:
  cmd:
    path:
      default: [/usr/bin/cmd]
    positional:
      - { kind: { kind: string }, repeated: true }
      - { kind: { kind: number } }
"#;
        let err = parse_pact(yaml).unwrap_err();
        assert!(
            matches!(err, ParseError::RepeatedNotLast { .. }),
            "expected RepeatedNotLast, got: {err}"
        );
    }

    #[test]
    fn rejects_unknown_kind() {
        let yaml = r#"
version: 1
name: bad
description: bad
defaults:
  timeout_seconds: 10
  max_output_bytes: 1048576
binaries:
  cmd:
    path:
      default: [/usr/bin/cmd]
    flag_values:
      "-x": { kind: filepath }
"#;
        let err = parse_pact(yaml).unwrap_err();
        assert!(
            matches!(err, ParseError::Yaml(_)),
            "expected Yaml serde error (unknown kind), got: {err}"
        );
    }
}
