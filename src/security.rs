use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse security.yaml: {0}")]
    Parse(#[from] serde_yaml::Error),

    #[error("HOME environment variable is not set")]
    HomeMissing,
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawSecurityConfig {
    reeve_home: String,
    allowed_roots: Vec<String>,
    deny_traversal: bool,
    env_passthrough: Vec<String>,
    audit: AuditConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuditConfig {
    pub capture_command: bool,
    pub capture_stdout: bool,
    pub capture_stderr: bool,
}

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub reeve_home: PathBuf,
    pub allowed_roots: Vec<String>,
    pub deny_traversal: bool,
    pub env_passthrough: Vec<String>,
    pub audit: AuditConfig,
}

impl SecurityConfig {
    /// Load and parse the embedded `security.yaml`.
    ///
    /// `$HOME` in `reeve_home` is expanded using `std::env::var("HOME")`.
    /// All other template variables in `allowed_roots` are stored as-is.
    pub fn load() -> Result<Self, ConfigError> {
        let yaml = include_str!("../security.yaml");
        let raw: RawSecurityConfig = serde_yaml::from_str(yaml)?;

        let home = std::env::var("HOME").map_err(|_| ConfigError::HomeMissing)?;
        let reeve_home = PathBuf::from(raw.reeve_home.replace("$HOME", &home));

        Ok(SecurityConfig {
            reeve_home,
            allowed_roots: raw.allowed_roots,
            deny_traversal: raw.deny_traversal,
            env_passthrough: raw.env_passthrough,
            audit: raw.audit,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_parses_embedded_yaml() {
        let cfg = SecurityConfig::load().expect("load() should succeed");
        assert!(cfg.deny_traversal);
        assert!(cfg.audit.capture_command);
        assert!(!cfg.audit.capture_stdout);
        assert!(!cfg.audit.capture_stderr);
        assert!(!cfg.env_passthrough.is_empty());
    }

    #[test]
    fn reeve_home_has_home_expanded() {
        let cfg = SecurityConfig::load().expect("load() should succeed");
        let home = std::env::var("HOME").expect("HOME must be set in test env");
        let reeve_home_str = cfg.reeve_home.to_string_lossy();
        assert!(
            !reeve_home_str.contains("$HOME"),
            "reeve_home must not contain literal $HOME, got: {reeve_home_str}"
        );
        assert!(
            reeve_home_str.starts_with(&home),
            "reeve_home should start with HOME={home}, got: {reeve_home_str}"
        );
    }
}
