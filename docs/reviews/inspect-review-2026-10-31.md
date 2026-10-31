# Inspect code and security review — 2026-10-31

Reviewed integrated source `f1d280a9fad560b28b7b7ba9affef20968016888`, focusing on M1-T05 composition, CLI routing/exit handling, captured-byte selection, report serialization, diagnostic/completeness propagation, tests, platform workflow and documentation. Rechecked the syntax/semantics boundaries and scanner/parser limits that this command consumes. Earlier foundation and extraction reviews remain historical evidence; this is not a new formal audit of every native dependency.

## Finding: raw Unicode controls in human export snippets

**Low severity — display integrity.** Import specifiers/reference text use debug escaping, but human ImportBinding/ExportKind records used ordinary JSON serialization. JSON permits non-ASCII Unicode controls and direction markers, so an untrusted module string could be rendered with raw U+009B or U+202E despite the documented human escaping policy. The reproduction used cleanly parsed star/named re-exports and confirmed those code points in stdout. No command execution or privilege escalation was demonstrated.

The new CLI regression failed before the repair on raw Unicode characters in the human report. A shared human-snippet renderer now applies Rust's non-printing Unicode policy, emitting valid JSON UTF-16 escapes. It covers supplementary characters with surrogate pairs. Tests verify U+009B, U+202E and U+E0001 escaping, preserve readable `café😀`, and parse each displayed export snippet back to the exact export JSON. Existing plain human goldens remain unchanged. Machine JSON reports retain the exact source values, envelope v1 and analyzer token; no extraction/identity/hash semantics change.

## Other reviewed boundaries

Selected files are looked up in the captured discovery snapshot, not reopened through an unchecked path. Portable relative-path validation, ignores/exclusions and symlink policy remain enforced. Discovery still captures the whole root, so unrelated invalid inputs may fail an inspection; this documented limitation is unchanged. Inspection executes no project scripts, writes no index, and does not claim manifest freshness or an atomic filesystem snapshot.

JSON is rendered before stdout writes, diagnostics retain their path/span and go to stderr, and recovery/unsupported evidence marks the report incomplete. A clean syntax report still explicitly lacks resolution. Future commands fail without a success document. OS write failures can leave partial output, as documented. No further actionable defect was established in this pass.

## Verification and limits

Baseline checks passed before changes (114 Rust tests). After the fix, the macOS workspace passed 115 Rust tests including 15 inspect tests, format/locked build/strict all-target Clippy. All 18 planning tests, plan validation, local links/ADR numbering, inspect demos and clean temporary installed smoke also passed. [Platform CI](https://github.com/adhyannsingal/ripple/actions/runs/41252220857) passed on Linux/macOS/Windows at integrated source `6034163810ebdd901199d8db1b96b9eb5315cc9f`; the task report and ledger retain the evidence. RustSec checked the unchanged 122-package lockfile with zero known vulnerabilities and zero warnings. No new dependencies were added.

This combines code inspection, focused regressions and known-advisory checks. Fuzzing, exhaustive Unicode/terminal emulation, formal proofs and native-dependency audits were not run. Performance is not measured; human embedded records gain one linear escaping pass, while JSON serialization follows the existing path. Existing source-memory/capture and unsupported TypeScript limitations remain in [limitations](../limitations.md).
