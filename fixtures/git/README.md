# Immutable Git fixtures

`crates/ripple-git/tests/revisions.rs` creates independent temporary repositories; no developer worktree is checked out or mutated by the adapter. Manually asserted cases cover:

- Spaces/Unicode paths, immutable base/head bytes and scanner ignores/exclusions.
- Staged plus dirty and untracked data; byte-identical index, HEAD, refs and working files after comparison.
- Moving refs after resolution (pinned SHAs remain authoritative), invalid/malicious-looking refs and missing objects.
- Add/delete/rename, empty commits and same endpoints, empty files, CRLF, unterminated final lines, insertion/deletion zero-width ranges.
- Binary/non-UTF-8 changes with visible diagnostics, bounded oversized reads, symlink exclusion/no target reads (Unix), and disabled external diff/textconv scripts (Unix).

Internal parser tests reject malformed/truncated records, invalid paths, range overflow and fake hunk-like source lines. Bounds and unsupported-path/submodule policy are documented in ADR 0009. Performance is not measured. Git must support `--no-lazy-fetch`; no automatic fetch is permitted.
