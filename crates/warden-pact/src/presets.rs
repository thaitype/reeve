use std::sync::OnceLock;

use crate::parse::parse_pact;
use crate::schema::Pact;

const LINUX_READONLY_YAML: &str =
    include_str!("../../../pacts/linux-readonly.yaml");

/// Return a reference to the parsed `linux-readonly` preset.
///
/// The pact is parsed once on first call and cached for the lifetime of the
/// process.  Panics if the embedded YAML is malformed — this is a programming
/// error, not a user error.
pub fn linux_readonly() -> &'static Pact {
    static INSTANCE: OnceLock<Pact> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        parse_pact(LINUX_READONLY_YAML)
            .expect("embedded linux-readonly pact must parse")
    })
}

#[cfg(test)]
const TEST_FIXTURES_YAML: &str =
    include_str!("../tests/fixtures/test-fixtures.yaml");

#[cfg(test)]
pub fn test_fixtures() -> &'static Pact {
    static INSTANCE: OnceLock<Pact> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        parse_pact(TEST_FIXTURES_YAML)
            .expect("embedded test-fixtures pact must parse")
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::validate_call;

    #[test]
    fn linux_readonly_yaml_parses() {
        let pact = linux_readonly();
        assert_eq!(pact.binaries.len(), 5);
        assert!(pact.binaries.contains_key("echo"));
        assert!(pact.binaries.contains_key("date"));
        assert!(pact.binaries.contains_key("uname"));
        assert!(pact.binaries.contains_key("whoami"));
        assert!(pact.binaries.contains_key("hostname"));
    }

    #[test]
    fn linux_readonly_validates_whoami_call() {
        let pact = linux_readonly();
        let result = validate_call(pact, "whoami", &[]);
        match result {
            Ok(_) => {}
            // whoami may not exist on all CI images; path resolution failure
            // is acceptable here — the policy validation passed.
            Err(crate::error::PactError::BinaryNotResolvable { .. }) => {}
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn test_fixtures_yaml_parses() {
        let pact = test_fixtures();
        assert_eq!(pact.binaries.len(), 2);
        assert!(pact.binaries.contains_key("sleep"));
        assert!(pact.binaries.contains_key("yes"));
    }

    #[test]
    fn test_fixtures_validates_sleep_call() {
        let pact = test_fixtures();
        let result = validate_call(pact, "sleep", &["1".to_owned()]);
        match result {
            Ok(_) => {}
            Err(crate::error::PactError::BinaryNotResolvable { .. }) => {}
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
}
