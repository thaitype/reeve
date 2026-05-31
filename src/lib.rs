// Internal modules — kept private so external crates that pull `reeve` as a
// library see no implementation surface.  Only the deliberate `pub use`
// below is part of the public API.
pub mod core;
mod pact;
pub mod security;

// Public API surface — kept intentionally small.  The binary target in
// `src/bin/reeve.rs` is a separate compilation unit that links this lib as
// a normal crate dep, so it needs at least one public symbol.
//
// Stability: unstable.  Anything here may break in any minor `0.x.y`
// release until v1.0.  See README — file an issue first if you depend on
// this surface.
pub use crate::core::build_engine_with_args;
