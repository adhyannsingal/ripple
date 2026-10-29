# ADR 0003: Atomic SQLite generations with captured-input manifests

Status: accepted (M0-T05); storage implementation belongs to M2-T03.

## Context

A persistent local graph needs files, symbols, test files, opaque boundaries, provenance and diagnostics. Crash safety and a consistent database transaction do not by themselves guarantee consistent source input.

## Options

An ad-hoc serialized graph; server storage; or local SQLite with versioned semantic records, captured-input manifests and transactional generations.

## Decision

SQLite is authoritative. Use the build spec's typed nodes and file/symbol detail tables, node-ID foreign keys on edges, unresolved records, cached syntax facts, metadata and complete input manifests. Numeric row IDs remain storage details and never become semantic IDs or lexical ScopeIds. Use foreign keys, migrations and explicit schema/analyzer/hash compatibility checks. Publish a complete generation in one write transaction and read its graph under a read transaction; serialize writers and make contention/failure actionable.

Capture supported source/config/ignore/package/lockfile inputs, analyze captured data, then re-observe membership, digests and HEAD before publication. Allow at most three total capture attempts. Continued concurrent edits or failed writes preserve the last complete generation, which may be stale and cannot be presented as current. The index layer implements real scanning/hashing/retry/transaction behavior; core comparison of supplied observations performs no IO and is not proof of freshness.

Keep `.ripple/metadata.json` as a derived summary. Write it after database commit with temp-file/rename; reconstruct disagreement from the committed DB. Do not let a partial summary become a second authority.

## Reasoning

SQLite supports the local no-server product while transaction boundaries make publication reviewable. Manifests keep source consistency separate from DB atomicity. Explicit versions prevent accidental reuse after incompatible analyzer/config changes.

## Consequences

SnapshotData is an untrusted transport record until validated against captured UTF-8 sources; runtime IndexSnapshot has no unchecked deserializer. Validate manifests, facts, graph membership/spans and compatibility before use. Digest shape validation does not compute or verify content hashes: production capture must do that. A changing worktree is a bounded failure, not an excuse to publish mixed state. Initial verified queries may scan/hash inputs; measure that cost separately from graph traversal.

M0 contract tests do not establish SQLite migration, crash-recovery, read/write concurrency or filesystem guarantees. M2/M5 must test those actual implementations.

## Revisit condition

Revisit for a proven generation-consistency issue, migration need or measured storage/distribution bottleneck. Never replace correctness with a cache-speed assumption.
