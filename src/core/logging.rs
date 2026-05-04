//! Logging host functions: `log_info`, `log_warn`, `log_error`, and `print_line`.
//!
//! Log format (contract): `<ISO-8601 timestamp> <LEVEL> <message>`
//! Timestamp is UTC, millisecond precision, with `Z` suffix.

use chrono::SecondsFormat;

// ---------------------------------------------------------------------------
// Pure helper — tested directly
// ---------------------------------------------------------------------------

/// Format a single log line according to the contract.
///
/// Format: `<ISO-8601 timestamp> <LEVEL> <message>`
pub fn format_log_line(level: &str, msg: &str) -> String {
    let ts = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    format!("{ts} {level} {msg}")
}

// ---------------------------------------------------------------------------
// Public host functions
// ---------------------------------------------------------------------------

pub fn log_info(msg: &str) {
    eprintln!("{}", format_log_line("INFO", msg));
}

pub fn log_warn(msg: &str) {
    eprintln!("{}", format_log_line("WARN", msg));
}

pub fn log_error(msg: &str) {
    eprintln!("{}", format_log_line("ERROR", msg));
}

/// Write `args` + newline to stdout.  Registered as Rhai `print`.
pub fn print_line(args: &str) {
    println!("{args}");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_format_shape() {
        let line = format_log_line("INFO", "hello");
        // Must match: YYYY-MM-DDTHH:MM:SS.mmmZ INFO hello
        let re = regex::Regex::new(
            r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z INFO hello$",
        )
        .unwrap();
        assert!(
            re.is_match(&line),
            "log line did not match expected format: {line:?}"
        );
    }

    #[test]
    fn log_format_warn_shape() {
        let line = format_log_line("WARN", "something");
        let re = regex::Regex::new(
            r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z WARN something$",
        )
        .unwrap();
        assert!(re.is_match(&line), "WARN line did not match: {line:?}");
    }

    #[test]
    fn log_format_error_shape() {
        let line = format_log_line("ERROR", "oops");
        let re = regex::Regex::new(
            r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z ERROR oops$",
        )
        .unwrap();
        assert!(re.is_match(&line), "ERROR line did not match: {line:?}");
    }
}
