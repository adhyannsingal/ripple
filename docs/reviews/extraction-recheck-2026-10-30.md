# Extraction and repository recheck — 2026-10-30

Scope: current seven-crate workspace and contributor checks, with a detailed review of the newly integrated symbol/hash extraction, independent fixture expectations, uncertainty propagation, compatibility metadata and current product claims. Revisited the scanner/parser containment and resource limits, source/semantics/impact dependency boundaries, CLI argument handling, CI and planning state. The earlier [source review](source-review-2026-10-30.md) documents the foundation review and its repairs.

## Finding and repair

**Incomplete uncertainty for assertion-wrapped literal bindings (medium correctness risk).** `export const wrapped = (() => 1) as Fn` previously produced no supported symbol, no unsupported-binding diagnostic and left a neighboring supported function with a complete signature fingerprint. This can mislead a future consumer about extraction completeness; no current impact command or early-cutoff implementation consumes it.

The targeted regression failed before the repair on the missing `unsupported_function_binding` diagnostic. Extraction now recognizes only literal function/class initializers behind parentheses and `as`, `satisfies`, angle-bracket type assertions or postfix non-null wrappers for diagnosis. It does not create new symbol targets. Directly exported bindings and local export aliases mark export shape incomplete, withholding complete file signature hashes while retaining clean body evidence. Ordinary scalar assertions and factory calls with callback arguments are negative controls.

The static asserted-initializers fixture supplies independently specified symbols, diagnostics and signature availability. Its expected `unhashable_declaration` diagnostic follows the existing incomplete-export invariant; the first golden run caught that missing expectation and it was corrected. There are now 14 declaration fixture programs and the existing 24 hash-change pairs. Type names in this fixture are syntax inputs, not TypeScript compiler validity evidence.

The extractor compatibility token advances to `ripple-symbols-v2-ts0.23.2-runtime0.27.1`. Hash encoding stays v1; the independent known-digest oracle is unchanged. See [ADR 0012](../adr/0012-syntax-hashes.md).

## Validation and security evidence

Local macOS checks passed after the repair: 77 Rust tests across the locked workspace, strict all-target Clippy, formatting, locked build, 18 planning tests, plan validation, help/version, clean temporary `cargo install --locked --path crates/ripple-cli --root <temp>` and installed help/version. Local Markdown targets (35) and ADR number uniqueness (11) passed before integration. The temporary installation does not overwrite the user's installed binary.

A fresh cargo-audit 0.22.2 scan of the unchanged lockfile checked 122 packages: zero known vulnerabilities and zero warnings. RustSec database commit `2295466c4e9df26647fd67d8fa1c705ac2fc7f81`, updated `2026-10-30T12:12:39+01:00`, contained 1395 advisories. No dependency changes were required. Review found no further established security defect in this pass. This combines code inspection, tests and known-advisory checking; it is not a formal audit, exhaustive native dependency analysis, fuzzing campaign or proof of vulnerability absence.

[Platform CI on the repaired integrated source](https://github.com/adhyannsingal/ripple/actions/runs/41129995713) passed on macOS, Linux and Windows at `27cf5245e42c395e28d29b7cfe1b8f0001c45e3f`. Local logs are session artifacts (`/tmp/ripple-recheck-final-tests.log`, `/tmp/ripple-recheck-audit.json`, `/tmp/ripple-recheck-install.log`), not durable CI evidence. The integration ledger records exact commands and the source SHA.

## Capabilities and remaining limits

[Current capabilities](../project-status.md) separates working discovery/parser/symbol/hash libraries from shared contracts and future commands. The CLI still only offers help/version. References/imports/exports, semantic resolution, persistent generations, Git diff/impact, incremental analysis and related-test selection remain upcoming. No performance or resolver accuracy measurements were made. Existing [limitations](../limitations.md), including aggregate scan memory and capture freshness, remain visible. The review is integrated and verified; next implementation task is M1-T04.
