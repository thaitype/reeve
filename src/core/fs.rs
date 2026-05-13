//! Layer 1 FS host functions scoped to `<reeve_home>/workspace/`.
//!
//! All paths are validated to stay within the workspace root before any
//! filesystem access is performed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rhai::{Array, Dynamic, EvalAltResult, Map, Position};

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

fn path_denied(path: &str) -> Box<EvalAltResult> {
    let mut map = Map::new();
    map.insert("kind".into(), Dynamic::from("PathDenied".to_owned()));
    map.insert("path".into(), Dynamic::from(path.to_owned()));
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from_map(map),
        Position::NONE,
    ))
}

fn file_not_found(path: &str) -> Box<EvalAltResult> {
    let mut map = Map::new();
    map.insert("kind".into(), Dynamic::from("FileNotFound".to_owned()));
    map.insert("path".into(), Dynamic::from(path.to_owned()));
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from_map(map),
        Position::NONE,
    ))
}

fn file_already_exists(path: &str) -> Box<EvalAltResult> {
    let mut map = Map::new();
    map.insert("kind".into(), Dynamic::from("FileAlreadyExists".to_owned()));
    map.insert("path".into(), Dynamic::from(path.to_owned()));
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from_map(map),
        Position::NONE,
    ))
}

fn io_error(path: &str, msg: &str) -> Box<EvalAltResult> {
    let mut map = Map::new();
    map.insert("kind".into(), Dynamic::from("IoError".to_owned()));
    map.insert("path".into(), Dynamic::from(path.to_owned()));
    map.insert("msg".into(), Dynamic::from(msg.to_owned()));
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from_map(map),
        Position::NONE,
    ))
}

// ---------------------------------------------------------------------------
// Path validation
// ---------------------------------------------------------------------------

/// Validate a script-supplied path for read/write operations.
///
/// Returns the resolved `PathBuf` (workspace_root joined with path) on
/// success, or a `PathDenied` error if the path is unsafe.
///
/// Rules:
/// 1. Reject absolute paths (start with `/`).
/// 2. Reject paths containing `..`.
/// 3. Canonicalize the parent dir of the candidate; reject if it escapes
///    the workspace root.
fn validate_path(workspace_root: &Path, path: &str) -> Result<PathBuf, Box<EvalAltResult>> {
    if path.starts_with('/') {
        return Err(path_denied(path));
    }
    if path.contains("..") {
        return Err(path_denied(path));
    }

    let candidate = workspace_root.join(path);

    // Canonicalize the parent directory (it must already exist for writes too
    // — if it doesn't exist we return IoError from the actual FS call later,
    // but first ensure the parent stays within workspace).
    let parent = candidate.parent().unwrap_or(workspace_root);

    // If the parent does not exist yet, walk up to the nearest existing ancestor
    // and check that.
    let existing_parent = nearest_existing_ancestor(parent);

    let canonical_parent =
        std::fs::canonicalize(&existing_parent).map_err(|e| io_error(path, &e.to_string()))?;

    // workspace_root itself must also be canonical for the prefix check.
    let canonical_workspace = std::fs::canonicalize(workspace_root)
        .map_err(|e| io_error(path, &e.to_string()))?;

    if !canonical_parent.starts_with(&canonical_workspace) {
        return Err(path_denied(path));
    }

    Ok(candidate)
}

/// Walk up the directory tree to find the nearest ancestor that exists on
/// disk. Returns the path itself if it exists, otherwise walks upward.
fn nearest_existing_ancestor(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    loop {
        if current.exists() {
            return current;
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => return current,
        }
    }
}

// ---------------------------------------------------------------------------
// FS host functions (called by Rhai closures)
// ---------------------------------------------------------------------------

/// Read a file's contents to a string.
pub fn read_file(workspace_root: &Path, path: &str) -> Result<String, Box<EvalAltResult>> {
    let candidate = validate_path(workspace_root, path)?;

    if !candidate.exists() {
        return Err(file_not_found(path));
    }

    std::fs::read_to_string(&candidate).map_err(|e| io_error(path, &e.to_string()))
}

/// Read a file's contents as an array of lines (without trailing newlines).
pub fn read_lines(workspace_root: &Path, path: &str) -> Result<Array, Box<EvalAltResult>> {
    let content = read_file(workspace_root, path)?;
    let lines: Array = content
        .split('\n')
        .map(|line| {
            let stripped = line.trim_end_matches('\r');
            Dynamic::from(stripped.to_owned())
        })
        .collect();

    // Remove trailing empty element caused by a final newline
    let mut lines = lines;
    if let Some(last) = lines.last() {
        if last.clone().cast::<String>().is_empty() {
            lines.pop();
        }
    }

    Ok(lines)
}

/// Check whether a path exists within the workspace.
/// Returns `false` (not an error) for paths that would be `PathDenied`.
pub fn exists(workspace_root: &Path, path: &str) -> bool {
    // Unsafe paths → false, not an error
    if path.starts_with('/') || path.contains("..") {
        return false;
    }

    let candidate = workspace_root.join(path);
    let parent = candidate.parent().unwrap_or(workspace_root);
    let existing_parent = nearest_existing_ancestor(parent);

    let Ok(canonical_parent) = std::fs::canonicalize(&existing_parent) else {
        return false;
    };
    let Ok(canonical_workspace) = std::fs::canonicalize(workspace_root) else {
        return false;
    };

    if !canonical_parent.starts_with(&canonical_workspace) {
        return false;
    }

    candidate.exists()
}

/// Create a new file with the given content. Fails if the file already exists.
pub fn write_file(
    workspace_root: &Path,
    path: &str,
    content: &str,
) -> Result<(), Box<EvalAltResult>> {
    let candidate = validate_path(workspace_root, path)?;

    if candidate.exists() {
        return Err(file_already_exists(path));
    }

    // Create intermediate directories if needed.
    if let Some(parent) = candidate.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_error(path, &e.to_string()))?;
    }

    std::fs::write(&candidate, content).map_err(|e| io_error(path, &e.to_string()))
}

/// Append content to a file, creating it if absent.
pub fn append_file(
    workspace_root: &Path,
    path: &str,
    content: &str,
) -> Result<(), Box<EvalAltResult>> {
    let candidate = validate_path(workspace_root, path)?;

    // Create intermediate directories if needed.
    if let Some(parent) = candidate.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_error(path, &e.to_string()))?;
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&candidate)
        .map_err(|e| io_error(path, &e.to_string()))?;

    file.write_all(content.as_bytes())
        .map_err(|e| io_error(path, &e.to_string()))
}

// ---------------------------------------------------------------------------
// Rhai registration
// ---------------------------------------------------------------------------

/// Register all FS host functions on `engine`, scoped to `workspace_root`.
pub fn register(engine: &mut rhai::Engine, workspace_root: Arc<Path>) {
    let ws_read_file = Arc::clone(&workspace_root);
    engine.register_fn(
        "read_file",
        move |path: &str| -> Result<String, Box<EvalAltResult>> {
            read_file(&ws_read_file, path)
        },
    );

    let ws_read_lines = Arc::clone(&workspace_root);
    engine.register_fn(
        "read_lines",
        move |path: &str| -> Result<Array, Box<EvalAltResult>> {
            read_lines(&ws_read_lines, path)
        },
    );

    let ws_exists = Arc::clone(&workspace_root);
    engine.register_fn("exists", move |path: &str| -> bool { exists(&ws_exists, path) });

    let ws_write_file = Arc::clone(&workspace_root);
    engine.register_fn(
        "write_file",
        move |path: &str, content: &str| -> Result<(), Box<EvalAltResult>> {
            write_file(&ws_write_file, path, content)
        },
    );

    let ws_append_file = Arc::clone(&workspace_root);
    engine.register_fn(
        "append_file",
        move |path: &str, content: &str| -> Result<(), Box<EvalAltResult>> {
            append_file(&ws_append_file, path, content)
        },
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        TempDir::new().expect("tempdir")
    }

    fn ws(tmp: &TempDir) -> &Path {
        tmp.path()
    }

    // --- Bypass-resistance tests ---

    // B1: absolute path → PathDenied
    #[test]
    fn b1_read_file_absolute_path_denied() {
        let tmp = setup();
        let err = read_file(ws(&tmp), "/etc/passwd").unwrap_err();
        let map = err_to_map(*err);
        assert_eq!(map_kind(&map), "PathDenied");
    }

    // B2: traversal → PathDenied
    #[test]
    fn b2_read_file_traversal_path_denied() {
        let tmp = setup();
        let err = read_file(ws(&tmp), "../../etc/passwd").unwrap_err();
        let map = err_to_map(*err);
        assert_eq!(map_kind(&map), "PathDenied");
    }

    // B3: double write → FileAlreadyExists
    #[test]
    fn b3_write_file_already_exists() {
        let tmp = setup();
        write_file(ws(&tmp), "out.json", "x").expect("first write should succeed");
        let err = write_file(ws(&tmp), "out.json", "y").unwrap_err();
        let map = err_to_map(*err);
        assert_eq!(map_kind(&map), "FileAlreadyExists");
    }

    // B4: append to absolute path → PathDenied
    #[test]
    fn b4_append_file_absolute_path_denied() {
        let tmp = setup();
        let err = append_file(ws(&tmp), "/etc/hosts", "x").unwrap_err();
        let map = err_to_map(*err);
        assert_eq!(map_kind(&map), "PathDenied");
    }

    // B5: traversal into adjacent dir → PathDenied
    #[test]
    fn b5_read_file_traversal_adjacent_denied() {
        let tmp = setup();
        let err = read_file(ws(&tmp), "../runs/anything/audit.jsonl").unwrap_err();
        let map = err_to_map(*err);
        assert_eq!(map_kind(&map), "PathDenied");
    }

    // --- Happy-path tests ---

    // H1: write then read
    #[test]
    fn h1_write_then_read() {
        let tmp = setup();
        write_file(ws(&tmp), "out.txt", "hello").expect("write");
        let content = read_file(ws(&tmp), "out.txt").expect("read");
        assert_eq!(content, "hello");
    }

    // H2: two appends then read
    #[test]
    fn h2_append_then_read() {
        let tmp = setup();
        append_file(ws(&tmp), "log.txt", "a").expect("append a");
        append_file(ws(&tmp), "log.txt", "b").expect("append b");
        let content = read_file(ws(&tmp), "log.txt").expect("read");
        assert_eq!(content, "ab");
    }

    // H3: exists returns true after write
    #[test]
    fn h3_exists_true_after_write() {
        let tmp = setup();
        write_file(ws(&tmp), "x.txt", "y").expect("write");
        assert!(exists(ws(&tmp), "x.txt"));
    }

    // H4: exists returns false for missing file
    #[test]
    fn h4_exists_false_for_missing() {
        let tmp = setup();
        assert!(!exists(ws(&tmp), "missing.txt"));
    }

    // H5: read_lines strips trailing newlines
    #[test]
    fn h5_read_lines_two_lines() {
        let tmp = setup();
        write_file(ws(&tmp), "two.txt", "line1\nline2").expect("write");
        let lines = read_lines(ws(&tmp), "two.txt").expect("read_lines");
        assert_eq!(lines.len(), 2, "expected 2 lines, got: {:?}", lines);
        assert_eq!(lines[0].clone().cast::<String>(), "line1");
        assert_eq!(lines[1].clone().cast::<String>(), "line2");
    }

    // --- Helpers ---

    fn err_to_map(err: EvalAltResult) -> Map {
        match err {
            EvalAltResult::ErrorRuntime(dyn_val, _) => dyn_val.cast::<Map>(),
            other => panic!("expected ErrorRuntime, got: {:?}", other),
        }
    }

    fn map_kind(map: &Map) -> String {
        map.get("kind")
            .cloned()
            .expect("kind field")
            .cast::<String>()
    }
}
