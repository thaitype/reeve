use crate::pact::schema::KindSpec;

/// Reason a kind validation failed.
#[derive(Debug)]
pub struct KindRejection {
    pub reason: String,
}

impl KindRejection {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for KindRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

/// The shell metacharacters forbidden in `string` kind values.
///
/// Each tuple is `(character, human-readable label for the error message)`.
const FORBIDDEN: &[(char, &str)] = &[
    ('\0', "null byte"),
    (';', "';'"),
    ('&', "'&'"),
    ('|', "'|'"),
    ('$', "'$'"),
    ('`', "'`'"),
    ('<', "'<'"),
    ('>', "'>'"),
    ('\n', "newline"),
    ('\r', "carriage return"),
];

/// Validate `value` against `kind`.
pub fn validate(kind: &KindSpec, value: &str) -> Result<(), KindRejection> {
    match kind {
        KindSpec::Enum { values } => {
            if values.iter().any(|v| v == value) {
                Ok(())
            } else {
                Err(KindRejection::new(format!(
                    "not in allowed values [{}]",
                    values.join(", ")
                )))
            }
        }
        KindSpec::Number => value
            .parse::<u64>()
            .map(|_| ())
            .map_err(|_| KindRejection::new("not a non-negative integer")),
        KindSpec::String_ => {
            for (ch, label) in FORBIDDEN {
                if value.contains(*ch) {
                    return Err(KindRejection::new(format!(
                        "contains forbidden character {label}"
                    )));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pact::schema::KindSpec;

    // --- Enum ---

    #[test]
    fn enum_accepts_listed() {
        let kind = KindSpec::Enum {
            values: vec!["a".into(), "b".into()],
        };
        assert!(validate(&kind, "a").is_ok());
        assert!(validate(&kind, "b").is_ok());
    }

    #[test]
    fn enum_rejects_unlisted() {
        let kind = KindSpec::Enum {
            values: vec!["a".into(), "b".into()],
        };
        let err = validate(&kind, "c").unwrap_err();
        assert!(err.reason.contains("not in allowed values"), "{}", err.reason);
    }

    // --- Number ---

    #[test]
    fn number_accepts_zero_and_positive() {
        let kind = KindSpec::Number;
        assert!(validate(&kind, "0").is_ok());
        assert!(validate(&kind, "42").is_ok());
        assert!(validate(&kind, "18446744073709551615").is_ok()); // u64::MAX
    }

    #[test]
    fn number_rejects_negative() {
        let kind = KindSpec::Number;
        let err = validate(&kind, "-1").unwrap_err();
        assert!(err.reason.contains("non-negative integer"), "{}", err.reason);
    }

    #[test]
    fn number_rejects_alpha() {
        let kind = KindSpec::Number;
        let err = validate(&kind, "abc").unwrap_err();
        assert!(err.reason.contains("non-negative integer"), "{}", err.reason);
    }

    // --- String ---

    #[test]
    fn string_accepts_plain() {
        let kind = KindSpec::String_;
        assert!(validate(&kind, "hello world").is_ok());
        assert!(validate(&kind, "some-flag-value_123").is_ok());
    }

    #[test]
    fn string_rejects_metacharacters() {
        let kind = KindSpec::String_;
        // One rejection test per forbidden metacharacter (10 total).
        let cases: &[&str] = &[
            "\0",  // null byte
            ";",   // command separator
            "&",   // background/chain
            "|",   // pipe
            "$",   // env interpolation
            "`",   // command substitution
            "<",   // redirect
            ">",   // redirect
            "\n",  // newline
            "\r",  // carriage return
        ];
        for input in cases {
            let err = validate(&kind, input)
                .unwrap_err_or_panic(&format!("expected rejection for {input:?}"));
            assert!(
                err.reason.contains("forbidden character"),
                "reason should mention 'forbidden character' for {input:?}, got: {}",
                err.reason
            );
        }
    }

    // Small helper: turns Ok into a panic, returns the Err value.
    trait UnwrapErrOrPanic<E> {
        fn unwrap_err_or_panic(self, msg: &str) -> E;
    }
    impl<T, E> UnwrapErrOrPanic<E> for Result<T, E> {
        fn unwrap_err_or_panic(self, msg: &str) -> E {
            match self {
                Err(e) => e,
                Ok(_) => panic!("{msg}"),
            }
        }
    }
}
