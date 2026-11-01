//! Supported semantic resolution from validated captured facts, independent of Tree-sitter.
pub mod exports;
pub mod modules;
pub use exports::{ModuleResolution, resolve};

pub const RESOLVER_VERSION: &str = "ripple-relative-v1";
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionError(pub String);
impl std::fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ResolutionError {}
