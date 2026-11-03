//! Deterministic reverse graph queries with independent semantic and module-fallback evidence.
pub mod graph;
pub mod traversal;

pub mod changes;
#[cfg(test)]
mod changes_tests;
#[cfg(test)]
mod traversal_tests;
