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
    if path.split('/').any(|c| c == "..") {
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

    // Canonicalize the full path (resolves symlinks) and verify it stays in workspace.
    let canonical = std::fs::canonicalize(&candidate).map_err(|_| path_denied(path))?;
    let canonical_workspace =
        std::fs::canonicalize(workspace_root).map_err(|e| io_error(path, &e.to_string()))?;
    if !canonical.starts_with(&canonical_workspace) {
        return Err(path_denied(path));
    }

    std::fs::read_to_string(&canonical).map_err(|e| io_error(path, &e.to_string()))
}

/// Read a file's contents as an array of lines (without trailing newlines).
pub fn read_lines(workspace_root: &Path, path: &str) -> Result<Array, Box<EvalAltResult>> {
    let content = read_file(workspace_root, path)?;
    // `.lines()` correctly handles trailing newlines and \r\n without producing
    // phantom empty elements.
    let lines: Array = content.lines().map(|l| Dynamic::from(l.to_owned())).collect();
    Ok(lines)
}

/// Check whether a path exists within the workspace.
/// Returns `false` (not an error) for paths that would be `PathDenied`.
pub fn exists(workspace_root: &Path, path: &str) -> bool {
    // Unsafe paths → false, not an error
    if path.starts_with('/') || path.split('/').any(|c| c == "..") {
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

    // If the candidate is a symlink, resolve it and verify it stays in workspace.
    if let Ok(meta) = candidate.symlink_metadata() {
        if meta.file_type().is_symlink() {
            match std::fs::canonicalize(&candidate) {
                Ok(resolved) => {
                    let Ok(cws) = std::fs::canonicalize(workspace_root) else { return false; };
                    if !resolved.starts_with(&cws) {
                        return false;
                    }
                }
                Err(_) => return false, // dead symlink or unresolvable
            }
        }
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

    // Deny if path is a symlink (dead or alive) — prevents overwriting via symlink.
    if candidate
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(path_denied(path));
    }

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

    // If the candidate already exists (including via symlink), canonicalize and recheck.
    if candidate.exists() {
        let canonical = std::fs::canonicalize(&candidate).map_err(|_| path_denied(path))?;
        let canonical_workspace =
            std::fs::canonicalize(workspace_root).map_err(|e| io_error(path, &e.to_string()))?;
        if !canonical.starts_with(&canonical_workspace) {
            return Err(path_denied(path));
        }
    }

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

    // TD-4: read_lines with trailing newline must not produce phantom empty element
    #[test]
    fn read_lines_strips_trailing_newline_artifact() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        // Single trailing newline — split('\n') would give ["line1", "line2", ""] before pop
        // .lines() gives ["line1", "line2"] — correct
        std::fs::write(workspace.join("trail.txt"), "line1\nline2\n").unwrap();

        let lines = read_lines(&workspace, "trail.txt").unwrap();
        assert_eq!(
            lines.len(),
            2,
            "trailing newline should not produce extra empty element, got: {:?}",
            lines
        );
        assert_eq!(lines[0].clone().cast::<String>(), "line1");
        assert_eq!(lines[1].clone().cast::<String>(), "line2");
    }

    // SF-1: symlink inside workspace pointing outside must be denied
    #[test]
    fn symlink_inside_workspace_pointing_outside_is_denied() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        // Create a symlink inside workspace pointing to /etc/passwd (or any outside path)
        let link = workspace.join("evil-link");
        std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();

        // read_file following the symlink must be denied
        let result = read_file(&workspace, "evil-link");
        let err = result.unwrap_err();
        let map = err_to_map(*err);
        assert_eq!(map_kind(&map), "PathDenied");
    }

    // Fix 2: exists() on symlink pointing outside → false
    #[test]
    fn exists_symlink_pointing_outside_returns_false() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        // Create a symlink inside workspace pointing outside
        let link = workspace.join("outside-link");
        std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();

        // exists() should return false — not follow the symlink
        assert!(
            !exists(&workspace, "outside-link"),
            "exists() must not return true for a symlink pointing outside workspace"
        );
    }

    // Fix 3: write_file on a dead symlink → PathDenied
    #[test]
    fn write_file_on_dead_symlink_denied() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        // Dead symlink (target doesn't exist)
        let link = workspace.join("dead-link");
        std::os::unix::fs::symlink("/nonexistent/path/secret", &link).unwrap();

        let result = write_file(&workspace, "dead-link", "evil");
        let err = result.unwrap_err();
        let map = err_to_map(*err);
        assert_eq!(map_kind(&map), "PathDenied");
    }

    // Fix 4: write_file creates intermediate directories
    #[test]
    fn write_file_creates_intermediate_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        // Path with a subdirectory that doesn't exist yet
        let result = write_file(&workspace, "subdir/output.txt", "hello");
        assert!(result.is_ok(), "write_file should create intermediate dirs: {:?}", result);

        let content = std::fs::read_to_string(workspace.join("subdir/output.txt")).unwrap();
        assert_eq!(content, "hello");
    }

    // Fix 5: append_file creates intermediate directories
    #[test]
    fn append_file_creates_intermediate_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        let result = append_file(&workspace, "newdir/log.txt", "first");
        assert!(result.is_ok(), "append_file should create intermediate dirs: {:?}", result);

        let result2 = append_file(&workspace, "newdir/log.txt", " second");
        assert!(result2.is_ok());

        let content = std::fs::read_to_string(workspace.join("newdir/log.txt")).unwrap();
        assert_eq!(content, "first second");
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
