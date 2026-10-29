# ADR 0006: Separate edge confidence from impact evidence

Status: accepted (M0-T04).

## Context

A resolved file import can conservatively connect a test to a module without proving dependence on a specific changed export. Unknown calls must not acquire guessed precise targets. Explanations must survive persistence and revision comparisons.

## Options

One mixed graph walk with a single confidence score; omitting all uncertainty; or separate semantic and file-fallback witness tracks with explicit resolution confidence.

## Decision

Use RESOLVED/CANDIDATE targeted edges with rules and spans; represent UNRESOLVED references with opaque boundary records and reasons. DEFINES is file → symbol; IMPORTS/REEXPORTS consumer → provider; CALLS/REFERENCES dependent → symbol. Require typed endpoints and owning-file provenance in validated graph snapshots.

Symbol-backed reverse witnesses use only CALLS/REFERENCES. Module fallback reverses an initial DEFINES bridge if starting from a symbol, then file IMPORTS/REEXPORTS. Fallback results remain file-level and are never relabelled precise because imports resolve. Keep weakest-hop confidence independently from evidence kind. Preserve seed and revision side; retain both tracks and their conservative union. Validate paths against their revision graph before presentation.

## Reasoning

This preserves explicit uncertainty and explainability without discarding useful conservative context. The two-export fixture demonstrates why known imports must not masquerade as symbol-specific influence. Unknown edges cannot fabricate downstream proof.

## Consequences

Consumers manage two tracks and may display two witnesses for a result. Conservative selection can equal a file-import baseline; measure and disclose this. Graph validation proves record/path consistency, not resolver correctness or shortestness. BFS/limits and oracle measurement remain separate tasks. File-level changes use fallback evidence, with no test-sufficiency guarantee.

## Revisit condition

Revisit for a measured supported-language gap or a demonstrated false-confidence/incomplete-path bug. Any transition-policy change needs fixtures, oracle/equality checks where applicable, and updated result semantics.
