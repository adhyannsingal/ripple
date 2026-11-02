//! Supported semantic resolution from validated captured facts, independent of Tree-sitter.
pub mod config;
mod config_json;
pub mod exports;
pub mod modules;
pub mod paths;
pub mod reexports;
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

pub mod symbols;
