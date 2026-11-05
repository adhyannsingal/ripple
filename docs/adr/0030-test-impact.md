# ADR 0030 — Revision-aware test selection projection

Status: accepted (M7-T01).

Reuse the immutable, validated base/head `RevisionImpact`; do not introduce another traversal or widen symbol evidence through containment. A pure `tests::select` projection creates per-presence evidence partitions and per-file witnesses. Active head test candidates are the union of old and new dependency evidence restricted to indexed head test nodes. Physically absent old test paths and present-but-unindexed old paths are reported separately. Each path retains all seed/revision/evidence-track witnesses with their original confidence and provenance.

The conservative combined set is the default. Symbol-backed, module-fallback and fallback-only views are explicit. Module fallback may equal file-import reachability; no precision advantage is inferred. Head indexed test count is the denominator for later CLI ratios; deleted/unindexed evidence is excluded from active counts. Zero denominator must be represented as undefined rather than a fabricated fraction.

Machine selection is independent of display limits. Requested depth and truncation, snapshot/classification uncertainty, unresolved context and unindexed changes remain explicit. Unknown targets cannot prove relevance or irrelevance, and no selected set guarantees test sufficiency. No test execution, coverage inference, runtime discovery, semantic resolver change or historical-store publication is introduced. M7-T02 composes this projection into the CLI; M8 evaluates fault-catching behavior.
