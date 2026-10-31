# ADR 0014: Inspect command and syntax report v1

Status: accepted (M1-T05).

## Decision

`ripple inspect <file> [--root <directory>] [--format human|json]` composes existing discovery, bounded parsing and complete syntax extraction. The default root is the current directory; no Git-root guessing is performed. File paths must be portable root-relative `.ts`/`.tsx` paths (including supported separator normalization); reject absolute/traversal/control paths and unsupported extensions as usage errors. Explicit root selection follows the scanner contract. Supported untracked files are included, repository ignores/exclusions apply and symlinks are skipped. No checkout/project scripts run, and no index or repository state is written.

Use `Worktree::discover` and analyze only the selected captured bytes, without reopening the path. Discovery currently reads the entire root: unrelated invalid/oversized inputs or ignore/config discovery errors can fail an inspection. This is a captured syntax report, not a verified persistent generation, atomic filesystem snapshot or freshness guarantee. Index publication verification remains M2-T05.

## Output contract

Report format v1 has `format_version`, `command`, `data`, `diagnostics` and `incomplete_analysis`. Inspect data includes `analysis_stage: "syntax"`, `resolution_available: false`, analyzer compatibility token, normalized path, language, test classification, parse status, symbols, imports, exports, scopes and references. Core facts move into typed output fields; their diagnostics are consolidated at the envelope, not duplicated in data. No resolved targets, edges, selected tests, root absolute paths, timestamps or timing metadata are invented. Preserve extractor ordering; sort/deduplicate consolidated diagnostics by their typed path/code/severity/message/span order.

The incomplete indicator is true for a non-clean parse or any reported discovery/syntax diagnostic. False means no known issue was reported in this syntax capture, not complete TypeScript semantics or resolved imports. No resolver is available regardless of the indicator. Discovery diagnostics can concern another path under the root; each record retains its own path and optional source span.

Human output lists classification, parse/completeness state, symbols and fingerprints, imports/exports, scopes/bindings, and references with owner/scope provenance. Raw source-derived strings are quoted/escaped rather than controlling the terminal. Embedded human JSON snippets additionally escape non-printing Unicode according to Rust debug-string policy, using JSON-compatible UTF-16 escapes (including surrogate pairs), while ordinary Unicode remains readable. M1-T08 repairs this display behavior without changing the v1 JSON report or source facts. JSON contains exactly one newline-terminated document on stdout with complete diagnostic records. Both formats also print escaped diagnostic details to stderr. Operational failures emit only an escaped `error[code]` line to stderr, with no success envelope; usage/help are handled by Clap. Successful reports are fully rendered before any stdout write. An OS write failure can still leave partial output and returns failure.

## Exit codes and future commands

Exit 0 means a report or help/version was produced. Recovered or failed parsing can produce a successful incomplete inspection with diagnostics and no fabricated clean facts. Exit 1 means operational failure (discovery/capture/extraction/output failure, unavailable selected file or a registered future command). Exit 2 means invalid/missing CLI arguments, path form, extension or format. Missing/ignored/excluded/symlink targets have `file_not_discovered` with root/policy guidance; no filesystem bypass is offered.

Register index/impact/diff/test/status in help as not implemented. They fail explicitly with `command_not_implemented`; they do not parse, build or query an index. Common global format handling and typed report/error conventions can be reused by future commands.

## Evidence and limits

Independent TS/TSX projections and empty/populated human goldens check the implemented command. Tests cover byte stability across repeated and relocated roots, root/path Unicode/spaces, recovery, opaque calls, module-test ownership, ignores/excludes/symlinks, invalid UTF-8/oversized inputs, JSON document isolation, escaped text and unchanged Git/index state. Linux/macOS/Windows CI runs these tests and TS human/TSX JSON demos. A clean temporary local installation smoke checks the installed binary without replacing the user's binary.

No performance measurement or resolver accuracy is claimed. This extends [ADR 0009](0009-git-and-output-policy.md), preserving its syntax/semantics boundary and [ADR 0013](0013-reference-extraction.md) uncertainty/budgets. Future output changes require an explicit format compatibility decision.
