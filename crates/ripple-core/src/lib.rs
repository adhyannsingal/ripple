//! Backend-independent source, identity and syntax contracts.

pub mod canonical;
pub mod graph;
pub mod identity;
pub mod index;
pub mod results;
pub mod revision;
pub mod source;
pub mod syntax;

#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    #[error("invalid repository-relative path: {0}")]
    InvalidPath(String),
    #[error("invalid identity: {0}")]
    InvalidIdentity(&'static str),
    #[error("invalid source span: {0}")]
    InvalidSpan(&'static str),
    #[error("invalid syntax facts: {0}")]
    InvalidFacts(&'static str),
    #[error("duplicate captured source: {0}")]
    DuplicateSource(identity::RepoPath),
}
