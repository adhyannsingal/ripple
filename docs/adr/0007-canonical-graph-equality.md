# ADR 0007: Versioned canonical semantic state, separate freshness checks

Status: accepted (M0-T05); broader fingerprints/properties arrive in M5/M6.

## Context

Incremental output must equal a cold rebuild independently of storage IDs, insertion order or scheduling. Making equality pass by omitting confidence, unresolved facts, diagnostics or provenance would conceal stale state.

## Options

Compare database files or row IDs; compare only graph counts; or compare a versioned, deterministic projection of complete semantic records.

## Decision

Canonical format v1 includes project identity, schema/analyzer/hash compatibility identifiers, effective configuration fingerprint, normalized captured-input manifests, FileFacts and typed graph records. Sort files and nested symbol/import/export/scope/binding/reference/diagnostic records; graph snapshots already sort semantic nodes/edges/boundaries. Retain raw-input hashes, spans, parse status, rule/provenance, confidence, unknown-reference text/reasons and effective-config inputs. Preserve multiplicity of syntax facts rather than hiding duplicates with a set.

Exclude operational generation counters, creation times, actual checkout origin/commit and DB row IDs. Identical inputs can have equal semantics across commits, but origin still matters for freshness and witness binding. `check_observation` and RevisionGraphs perform separate checks; canonical equality never certifies the live worktree or substitutes for revision labels. Lexical ScopeIds are deterministic source-local structure, not insertion-dependent DB IDs.

Reject unknown canonical comparison format versions. The comparison artifact cannot be loaded as a runtime IndexSnapshot; deserialize SnapshotData and validate against captured sources for that. Cold-equals-incremental tests compare builds of the same final input under the same analyzer/configuration.

## Reasoning

The projection detects stale relationships and diagnostics while removing irrelevant persistence noise. Separating semantics from provenance avoids confusing equal graphs with equal revision/input freshness.

## Consequences

An encoding or analyzer-rule change requires intentional compatibility/version handling. The analyzer token supplied by the loader must identify its own supported semantics, including development builds; reading an expected token from the cache would defeat the check. Hash algorithms/encodings are fixed by the eventual schema/hash-version implementation, not guessed from a 64-character digest. M0 uses synthetic digest records to test contracts and computes no hashes.

Canonical tests verify ordering, serialization and retained differences; they do not prove an incremental updater correct. Establish the generated updater properties before early cutoff in M5-T05, expand them/replay real history in M6, and fix discrepancies rather than dropping fields.

## Revisit condition

Revisit on a demonstrated false equality, nondeterministic field, new semantic fact or required format migration. No performance shortcut may silently weaken equality.
