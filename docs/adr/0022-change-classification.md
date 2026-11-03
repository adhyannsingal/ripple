# ADR 0022: Pure two-endpoint syntax change classification

Status: implemented by M4-T02; integration/CI evidence is recorded separately.

## Boundary and inputs

`ripple_impact::changes::classify` consumes validated `RevisionChanges` and two declared immutable endpoints: commit IDs, analyzer token, captured supported sources, FileFacts and captured global-input paths. It performs no parsing, Git processes, SQL or filesystem reads. Production dependencies remain core/serde; Git/parser dependencies are test-only. The caller must bind bytes/facts to actual commits; this API validates declared IDs, analyzer compatibility, source/fact membership, unique project/file identities and source spans, not Git cryptographic provenance. M4-T03 supplies these inputs from complete immutable revision builds.

No CLI diff wiring, cross-revision traversal or incremental cutoff is introduced. ChangeSet retains the analyzer token, immutable revision labels, complete per-file records and per-symbol base/head facts with source spans/fingerprints. Its version token is `ripple-changes-v1`.

## Attribution versus classification

Normalize Git nonempty ranges from one-based lines to half-open byte intervals using captured raw UTF-8/LF offsets. CRLF bytes are retained. A zero-count range anchors after its Git line, including line zero/EOF; reject out-of-bounds ranges rather than clamping. Nonexistent or unindexed sides have no fabricated byte span; original Git line ranges/path presence remain available.

Build a per-file interval partition from supported declaration spans. Each interval chooses the smallest enclosing supported symbol; equal-size ties are explicit ambiguity. Zero-width anchors must be strictly inside the selected declaration, otherwise remain file-level boundaries. A line hunk can touch multiple declarations, class headers, methods and outside text. These owners are **coarsely touched**, not actual semantic changes: downstream consumers must seed from classified changes, not blindly from hunk-owner lists.

Compare the union of stable symbol IDs in each changed file, independently of hunk attribution. Different paths/renames become delete+add; there is no speculative symbol continuity. A rename row retains its Git hint and `RENAME_AS_DELETE_ADD` reason. Unilateral presence in a modified file with incomplete/unindexed opposite facts is `UNCERTAIN`, not a proven addition/deletion. Actual file addition/deletion/rename establishes path presence transitions.

Paired comparable signature/body hashes produce ADDED/DELETED/BODY_MODIFIED/SIGNATURE_MODIFIED/UNCHANGED where justified. Signature changes take display precedence, while both signature/body delta flags are retained. Missing or version-incompatible comparison fingerprints remain unknown; never treat two absent hashes as equal. Known signature changes can coexist with an unknown body flag; incomplete evidence remains visible. No changed-fingerprint reason is emitted without an observed differing fingerprint.

Class hashes already exclude supported method bodies. Equal paired classes are omitted from symbol-change rows, so method-body-only edits do not double-report the parent class. Changed class/member shape remains signature evidence. Equal non-class symbols are included as UNCHANGED only when a hunk touches them; other unchanged declarations are omitted. All Git file rows remain, even when there is no supported symbol change.

## Conservative file evidence and limits

Unowned/ambiguous hunk portions, unsupported/unindexed/diagnosed endpoints, no-text-hunk changes and renamed files retain explicit file-level reasons and ranges. Coarse line hunks may include unchanged whitespace outside declarations; this can conservatively retain file fallback even for a symbol classified UNCHANGED. Do not claim exact changed-character attribution or that an unchanged syntax hash proves full TypeScript semantic equivalence.

Captured global inputs (including arbitrary inherited config names) plus recognized tsconfig/package/lock/ignore files request global invalidation. Both base/head input sets participate so removed config inputs remain observable. Unindexed inputs are not silently ignored or presented as complete symbol analysis. Interpretation/traversal belongs to M4-T03.

File/symbol/hunk input records are capped at one million; this is not a hard total-process memory or end-to-end deadline guarantee. Intervals are built once per file and mapped using ordered lookup. Performance is not measured; no optimization claim or benchmark harness is introduced. These classifications must not enable incremental early cutoff or test skipping without the later equality/safety gates.

## Evidence

`fixtures/changes/cases.json` contains 25 manually expected Git-backed cases; unit tests additionally check provenance/version/inventory failures, absent hashes, ambiguous spans, zero-width/out-of-bounds anchors and identical revisions. Source ordering does not change the resulting ChangeSet. Whole-project oracle, replay, mutation safety and benchmarks are separate gates; this task changes no resolver behavior or parser hash format.
