# ADR 0009: Read-only Git adapter and versioned CLI data output

Status: accepted (M0-T05); real Git/command integration remains M4/M1 work.

## Context

Ripple needs reproducible Git inputs and agent-consumable output without requiring a service, mutating user checkouts or confusing diagnostics with data. Index schema versions and CLI output versions serve different consumers.

## Options

A Git library versus the installed Git executable behind a narrow provider; informal human-only output versus versioned typed command reports alongside human rendering.

## Decision

Start with the installed Git executable behind ripple-git and the core provider/revision contracts. Invoke argument arrays without a shell, resolve refs once to full commit IDs, parse machine-readable/NUL-delimited paths and retain both hunk ranges. Reject unsupported three-dot syntax with explicit endpoint guidance. Use read-only operations: no checkout/reset/stash, repository hooks or project-script execution. Disable external diff/textconv behavior for structured diff reads. Keep the adapter replaceable by gix/git2 if measured needs justify it.

Add human output and `--format json` as commands are implemented. JSON reports use a versioned envelope: `format_version`, command, typed data, diagnostics and an explicit incomplete-analysis indicator. This output version is separate from SQLite `schema_version` and the canonical comparison format. Sort records deterministically, preserve evidence_kind/revision/confidence/provenance, and keep stdout data-only; send progress/errors to stderr. A failure produces no fabricated success report. No timestamps/timings enter deterministic facts by default.

Exit 0 means a report was produced (or help/version), including explicitly reported uncertainty; it never guarantees complete TypeScript support or sufficient selected tests. Usage errors use 2; operational failures such as invalid refs, incompatible/stale state or failed indexing use 1. Actual commands/renderer tests must establish these behaviors; M0 CLI still implements only help/version. Inspection can report parse diagnostics, while an unusable index build preserves the prior generation and fails.

## Reasoning

An isolated Git executable adapter keeps integration simple while immutable blobs avoid user-worktree side effects. Structured reports support fixtures/agents, and separate version identifiers prevent persistence migrations from silently redefining CLI consumers. Explicit uncertainty is part of successful reporting rather than a hidden correctness guarantee.

## Consequences

Git is a documented installation prerequisite. The adapter needs platform/path/process/error tests and bounded subprocess behavior; none is proven by core DTO tests. Index loaders compare versions to a token from the running analyzer, not the cached record. That token must distinguish incompatible development semantics as well as released versions. Future output changes require deliberate envelope version handling and golden fixtures.

## Revisit condition

Revisit for measured Git process overhead, a required resolution/diff feature, or a demonstrated output compatibility problem. Preserve immutable endpoints, data-only stdout and conservative claims.
