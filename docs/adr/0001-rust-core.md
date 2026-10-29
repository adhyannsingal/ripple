# ADR 0001: Rust core with explicit subsystem boundaries

Status: accepted (M0-T02).

## Context

Ripple is a local CLI with native parsing, persistent indexing, graph traversal and reproducible correctness requirements. Shared contracts must not couple syntax facts to parser/DB/Git implementations.

## Options

One monolithic Rust crate; a mixed-language production pipeline; or a small Rust workspace with backend-independent contracts.

## Decision

Use seven Rust crates in the dependency direction documented in architecture.md. `ripple-core` contains source/identity/syntax contracts and later graph/result/revision records, with no backend dependency. Serialize typed facts with serde and report contract failures with thiserror. The CLI composes the independent providers/engines. Node is reserved for evaluation, not production execution.

## Reasoning

Separate syntax, semantics and impact ownership permits independently tested behavior and controlled integration. Rust provides a single native distribution target without requiring a service/runtime for users. These choices implement the authoritative build spec rather than asserting language superiority.

## Consequences

Shared contract changes require coordinator review and consumer updates. More manifests are maintained. Empty subsystems remain honest scaffolds until their own tasks implement them. Production packages are unpublished until licensing/release tasks establish metadata.

## Revisit condition

Revisit if a measured ownership/dependency problem warrants merging/splitting a crate; preserve the syntax/semantics/impact boundary.
