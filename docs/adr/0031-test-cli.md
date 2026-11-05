# ADR 0031 — Conservative immutable test-impact CLI

Status: accepted (M7-T02).

`ripple test A..B` shares the existing pinned Git endpoint/cold snapshot/classification/traversal boundaries and projects the validated result through `tests::select`. It never executes tests, checks out user files, loads or publishes the user's persistent index, or fetches Git objects. Explicit two-dot endpoints and operational error semantics match diff.

The format-v1 JSON envelope exposes complete selection partitions, per-file seed/revision/track witnesses, raw counts, Git diagnostics and full revision analysis. Display limits affect only human rows; depth restrictions are explicit incompleteness. Head-only symbol-backed/combined/module-fallback ratios store numerator, supported indexed head-test denominator and fraction; denominator zero yields null. Base-only and present-unindexed evidence never enters the active numerator. Static supported head test classification is not runtime runner discovery.

Human output labels fallback-only paths, shows uncertainty, omissions and recomputable ratios, and states potentially related rather than sufficient. Default conservative union may equal file-import reachability; ratios alone establish no precision gain or safe omission. M7's pinned external examples are incomplete static reports, not executed-test or fault-catching evidence. M8 supplies that evaluation. The existing 0.3.0-dev internal version remains until the M8/v0.4 capability gate; no publication/tag is performed here.
