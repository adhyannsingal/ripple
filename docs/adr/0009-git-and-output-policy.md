# ADR 0009: Read-only Git adapter and versioned CLI data output

Status: accepted (M0-T05); inspect output is implemented in M1-T05; real Git integration remains M4 work.

## Context

Ripple needs reproducible Git inputs and agent-consumable output without requiring a service, mutating user checkouts or confusing diagnostics with data. Index schema versions and CLI output versions serve different consumers.

## Options

A Git library versus the installed Git executable behind a narrow provider; informal human-only output versus versioned typed command reports alongside human rendering.

## Decision

Start with the installed Git executable behind ripple-git and the core provider/revision contracts. Invoke argument arrays without a shell, resolve refs once to full commit IDs, parse machine-readable/NUL-delimited paths and retain both hunk ranges. Reject unsupported three-dot syntax with explicit endpoint guidance. Use read-only operations: no checkout/reset/stash, repository hooks or project-script execution. Disable external diff/textconv behavior for structured diff reads. Keep the adapter replaceable by gix/git2 if measured needs justify it.

Add human output and `--format json` as commands are implemented. JSON reports use a versioned envelope: `format_version`, command, typed data, diagnostics and an explicit incomplete-analysis indicator. This output version is separate from SQLite `schema_version` and the canonical comparison format. Sort records deterministically, preserve evidence_kind/revision/confidence/provenance, and keep stdout data-only; send progress/errors to stderr. A failure produces no fabricated success report. No timestamps/timings enter deterministic facts by default.

Exit 0 means a report was produced (or help/version), including explicitly reported uncertainty; it never guarantees complete TypeScript support or sufficient selected tests. Usage errors use 2; operational failures such as invalid refs, incompatible/stale state or failed indexing use 1. The inspect command and its renderer tests now establish these behaviors; see [ADR 0014](0014-inspect-output.md). Inspection can report parse diagnostics, while an unusable index build preserves the prior generation and fails.

## Reasoning

An isolated Git executable adapter keeps integration simple while immutable blobs avoid user-worktree side effects. Structured reports support fixtures/agents, and separate version identifiers prevent persistence migrations from silently redefining CLI consumers. Explicit uncertainty is part of successful reporting rather than a hidden correctness guarantee.

## Consequences

Git is a documented installation prerequisite. The adapter needs platform/path/process/error tests and bounded subprocess behavior; none is proven by core DTO tests. Index loaders compare versions to a token from the running analyzer, not the cached record. That token must distinguish incompatible development semantics as well as released versions. Future output changes require deliberate envelope version handling and golden fixtures.

## Revisit condition

Revisit for measured Git process overhead, a required resolution/diff feature, or a demonstrated output compatibility problem. Preserve immutable endpoints, data-only stdout and conservative claims.

## M4-T01 implementation

`ripple_git::revisions::Repository` resolves A..B once with `rev-parse --verify --end-of-options REF^{commit}`, then uses only full immutable object IDs. `RevisionSnapshots` returns base/head `RevisionTree` providers plus validated `RevisionChanges` and explicit non-text/mode diagnostics. These are library APIs; the user-facing diff command remains M4-T04.

The shared `SourceTree`/entry/error contract moves from ripple-parse to ripple-core and is re-exported by the scanner for source compatibility. Git still depends only on core in production. Consumers run the existing scanner over immutable trees, preserving its ignore/exclusion/test classification and symlink policy rather than duplicating it or adding a Git → parser dependency.

Tree/raw-diff paths use NUL records, UTF-8 and normalized portable RepoPath validation; spaces and Unicode are retained, control/backslash/nonportable names fail explicitly. Read regular raw blobs only; symlinks are listed but never followed, and submodules are unsupported. There is no checkout or working-file read. Missing objects fail, replacement refs are disabled and lazy fetching is disabled by both environment and the global `--no-lazy-fetch` flag (Git must support that option; older Git fails closed).

Use bounded existing process reads (10 seconds and 8 MiB per stdout/stderr), at most 100000 tree entries and 256 MiB of blob reads per endpoint provider. Repeated reads count toward the budget. This is not a total RSS/end-to-end deadline guarantee. Tree objects may be pruned by an external process; missing/corrupt inputs fail instead of producing partial success. Git binary/private repository metadata remain trusted.

Parse raw A/D/M/T/R records with rename hints at 50% and a finite rename-search limit; a hint is never semantic symbol continuity. Get text hunks from blob-to-blob zero-context Myers diffs, disabling external diff/textconv/color and controlling context/output indicators. Add/delete ranges count original raw lines including a final unterminated line. Both endpoints' UTF-8 text bytes and line endings remain untouched. Binary/non-UTF-8 and nonregular file changes retain whole-file records with explicit diagnostics and no invented text hunks. Supported config/ignore changes remain in file changes even when not TS sources. [Git diff](https://git-scm.com/docs/git-diff), [Git process controls](https://git-scm.com/docs/git).
