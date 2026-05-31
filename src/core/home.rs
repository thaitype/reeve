use std::{fs, path::Path};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HomeInitError {
    #[error("failed to initialise reeve home directory: {0}")]
    Io(#[from] std::io::Error),
}

/// Create the reeve home directory structure under `home`.
///
/// Creates `<home>/workspace` and `<home>/runs` via `create_dir_all`.
/// Idempotent — safe to call on every startup.
pub fn init_home(home: &Path) -> Result<(), HomeInitError> {
    fs::create_dir_all(home.join("workspace"))?;
    fs::create_dir_all(home.join("runs"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn init_home_creates_workspace_and_runs() {
        let tmp = TempDir::new().expect("tempdir");
        let home = tmp.path();

        init_home(home).expect("init_home should succeed");

        assert!(home.join("workspace").is_dir(), "workspace dir should exist");
        assert!(home.join("runs").is_dir(), "runs dir should exist");
    }

    #[test]
    fn init_home_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        let home = tmp.path();

        init_home(home).expect("first call should succeed");
        init_home(home).expect("second call should succeed (idempotent)");
    }
}
