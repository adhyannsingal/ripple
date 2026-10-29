# Graph and evidence contracts

M0-T04 implements validated graph/result records in `ripple-core`, not resolution, adjacency construction or impact BFS. `ripple-impact` builds forward/reverse adjacency from these sorted records in M2-T04; it must not query SQLite at each hop.

## Nodes, edges and provenance

`NodeId` is a repository-local file identity, structured `SymbolId`, or opaque reference identity. File/test classification is metadata on one file node, never two competing identities. Functions/classes/methods use their corresponding symbol kind. Opaque nodes carry a deterministic reference-occurrence key and span; unlike symbol IDs, boundary occurrence keys need not survive source-layout edits. Every non-file node has a defining file. Every symbol has exactly one resolved `DEFINES` edge from that file. Every opaque node has one unresolved record with a reason/provenance and no invented target edge.

| Edge | Direction | Allowed endpoints |
|---|---|---|
| DEFINES | owning file → declaration | FILE/TEST_FILE → symbol in that file |
| IMPORTS / REEXPORTS | consuming file → provider file | FILE/TEST_FILE → FILE/TEST_FILE |
| CALLS / REFERENCES | dependent → dependency | symbol or exact module-level reference owner → symbol |

Edges retain confidence, source-file/span provenance and resolution rule. Cross-project edges are outside initial support. Duplicate nodes/edges, missing endpoints, inconsistent kinds, missing definitions, orphan boundaries and provenance outside a source owner are invalid. Snapshot construction/deserialization validates and sorts records by semantic content. This checks internal consistency; it does not prove a resolver rule is semantically correct. Coordinate/source-byte verification also belongs at the source/index boundary.

## Confidence and evidence kind are independent

`RESOLVED` means a supported deterministic resolver rule identifies one target. `CANDIDATE` retains a justified plausible target without pretending certainty. `UNRESOLVED` belongs to explicit unresolved records/opaque boundaries; a targeted edge with that confidence is rejected. Structural definition edges are resolved. Confidence order is strongest to weakest, and a witness reports its weakest hop.

`symbol_backed` paths reverse only `CALLS`/`REFERENCES`, starting at a symbol. Module-level direct calls/references can therefore reach test-file nodes without expanding through an entire file. References inside top-level test callbacks remain owned by the file according to the source contract.

`module_fallback` paths start from a changed file, or first reverse a symbol's actual `DEFINES` edge to its file, then reverse `IMPORTS`/`REEXPORTS`. Results in this track are files. Definitions are not allowed midway through this track; semantic edges are not silently mixed into it. File/config/top-level changes without a concrete changed symbol seed this conservative track and cannot claim a symbol-backed path. Unsupported boundaries remain visible alongside either track; they are not traversed as invented call edges.

An all-resolved import path is still module fallback: certainty about the import is not certainty about a particular changed export's relevance. Both tracks must be retained. `EvidenceSets` preserves distinct seeds/revisions and both witness types, deterministically deduplicating only exact duplicate witnesses. The default potentially-related selection is the union; fallback-only results are the difference from symbol-backed results. Neither the union nor a narrower view guarantees test sufficiency.

Example: `src/lib.ts` exports unrelated `a` and `b`; `a.test.ts` imports/calls `a`, and `b.test.ts` imports/calls `b`. For an `a` change, symbol-backed test evidence reaches only `a.test.ts`, while module fallback reaches both. The existing unrelated-exports fixture independently specifies these sets.

## Witness, revision and output contracts

A `Witness` records a seed, target, revision side (worktree/base/head), evidence kind and original dependency edges in reverse-walk order. Validation checks membership in the chosen revision graph, continuity, endpoint identity, evidence policy and absence of repeated nodes. The graph itself may contain cycles; explanation paths are simple. The later BFS chooses shortest witnesses with deterministic tie-breaking; core validation does not claim to choose or prove shortestness. Immutable SHA/origin binding arrives in M0-T05.

Distance is hop count within a track. A fallback containment hop counts; it must not be presented as a semantic call distance. There can be an exact and fallback witness to the same result, or base/head witnesses to it; keep these rather than deduplicating just by display name.

Query defaults are unlimited depth and 100 displayed results. `QueryLimits` and `Truncation` explicitly separate depth boundaries from omitted display rows. Display limiting must not limit traversal or the complete test selection. `ImpactResult` is a transport contract: deserialize/partition evidence structurally, then validate each witness against its actual revision graph before rendering. The query engine will populate completeness/limit data; no query is implemented at M0-T04.

## Acceptance evidence

`cargo test --locked -p ripple-core --test graph_contracts` exercises independently authored graph records: unrelated exports, direct file-owned calls, candidates, unresolved boundaries, graph cycles, disconnected/forged witnesses, provenance errors, serialization and evidence partitioning. Tests do not compare a real resolver or claim measured TypeScript accuracy. Actual extraction/resolution fixtures and oracle metrics belong to their later tasks.
