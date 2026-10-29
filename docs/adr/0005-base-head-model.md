# ADR 0005: Immutable two-endpoint snapshots and revision-bound witnesses

Status: accepted (M0-T05).

## Context

Deleted symbols and removed calls exist only in base. Using only the head graph loses their downstream explanations. A worktree index with HEAD provenance may contain dirty or untracked input and cannot stand in for a committed tree.

## Options

Head-only traversal; checking out user revisions; or separate snapshots built from immutable Git trees/blobs at both explicit endpoints.

## Decision

`A..B` means endpoint A versus endpoint B; reject three-dot/ambiguous syntax initially. Resolve refs exactly once through the Git adapter to full, normalized 40/64-character commit IDs. Read trees/blobs without checkout, reset, stash, hooks or worktree mutation. Apply scanner/config rules to each revision's data, never silently to current-worktree configuration.

SnapshotOrigin distinguishes worktree (optional HEAD plus dirty marker) from immutable commit data. RevisionGraphs binds Base/Head witness sides to snapshots whose immutable origins match the requested pair, project and analyzer formats. A clean worktree origin is still not a commit origin. Keep both graphs, union results by semantic identity and retain revision-labelled witnesses—including base-only deleted code/tests. Never relabel a base-only path as a head path.

RevisionChanges retains old/new paths, change kinds and both Git line ranges. Empty-side ranges may use line zero; nonempty ranges start at one. Git rename similarity is only a file-level hint; path-based symbol identities can become delete+add. Source-range mapping/classification remains M4 work.

## Reasoning

Immutable endpoints make comparisons reproducible and protect user work. Two graphs preserve removed dependencies rather than pretending they remain in head. Explicit side binding prevents explanations from drifting between index origins.

## Consequences

Initial comparisons may cold-build both snapshots; caching is secondary. Core validates IDs, payload consistency and witness membership, not that a Git ref exists or a hunk was actually produced by Git. Git/process/diff integration and real branch evidence belong to M4. Operational origin is retained even though canonical semantic equality deliberately excludes it.

## Revisit condition

Revisit for measured revision-build cost, reliably proven rename/move matching or an intentionally supported comparison mode. Preserve endpoint immutability and old-path evidence.
