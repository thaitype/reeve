# Contract — `security.yaml` + `SecurityConfig` + `RunContext`

## `security.yaml` (repo root, compile-time embedded)

```yaml
reeve_home: "$HOME/.reeve"        # expanded at runtime; $HOME from std::env
allowed_roots:                    # reserved for Layer 2 (not enforced this milestone)
  - "$CWD"
  - "$HOME/.reeve/workspace"
deny_traversal: true
env_passthrough: [PATH, HOME, LANG]
audit:
  capture_command: true           # always true; field kept for future override
  capture_stdout: false
  capture_stderr: false
```

Embedded via `include_str!("../security.yaml")` in `src/security.rs`.
Parsed at process startup; startup aborts with exit code 3 on parse error.

`$HOME` in `reeve_home` is expanded by calling `std::env::var("HOME")` at
startup — NOT shell expansion. `$CWD` expansion is deferred to Layer 2.

## `SecurityConfig` struct (`src/security.rs`)

```rust
pub struct SecurityConfig {
    pub reeve_home:      PathBuf,          // $HOME expanded
    pub allowed_roots:   Vec<String>,      // raw strings; expansion deferred
    pub deny_traversal:  bool,
    pub env_passthrough: Vec<String>,
    pub audit:           AuditConfig,
}

pub struct AuditConfig {
    pub capture_command: bool,
    pub capture_stdout:  bool,
    pub capture_stderr:  bool,
}
```

Constructor: `SecurityConfig::load() -> Result<Self, ConfigError>`.
Called once in `main()`; result wrapped in `Arc` before passing to `RunContext`.

## `RunContext` (`src/core/run_context.rs`)

```rust
pub struct RunContext {
    pub security: Arc<SecurityConfig>,
    pub audit:    Arc<Mutex<AuditWriter>>,
}
```

Created in `main()` after `SecurityConfig::load()` and home init succeed.
Passed into `build_engine_with_args(args, Arc<RunContext>)`.
Each host-fn closure captures `Arc::clone(&ctx)`.

## Home init (`src/core/home.rs`)

Called once in `main()` before engine build, after `SecurityConfig::load()`.

```rust
pub fn init_home(home: &Path) -> Result<(), HomeInitError>
```

Steps:
1. `fs::create_dir_all(home.join("workspace"))` 
2. `fs::create_dir_all(home.join("runs"))`
3. Return `Ok(())`.

`HomeInitError` wraps `std::io::Error`. Maps to exit code 3 in the CLI.
No sentinel file. No existence checks beyond what `create_dir_all` provides.

## Module layout additions

```
src/
├── security.rs              # SecurityConfig, AuditConfig, load()
└── core/
    ├── run_context.rs       # RunContext struct
    ├── home.rs              # init_home()
    └── audit.rs             # AuditWriter (see contract 03)
```
