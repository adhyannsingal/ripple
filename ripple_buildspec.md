# Ripple
## Final Build Specification, Architecture, and Agent Guide

**Project name:** Ripple\
**Resume title:** Change Impact Engine  
**CLI:** `ripple`\
**Core language:** Rust  
**Analyzed language:** TypeScript / TSX  
**Parser:** Tree-sitter  
**Persistence:** SQLite  
**Source control:** Git  
**Primary interface:** Local CLI  
**Target:** 6–8 active development weeks within one semester  

---

# 1. One-line description

**Ripple is a local Rust developer tool that maps TypeScript Git changes to symbols and traces their downstream dependency paths to explain which code and tests may be affected.**

Casual version:

> **Git tells you what changed. Ripple tells you what that change could affect elsewhere in the codebase.**

---

# 2. Problem

Git is excellent at showing textual changes:

```text
src/auth/token.ts
lines 41–48 modified
```

But developers usually reason about software through:

```text
functions
methods
classes
imports
modules
references
tests
```

A small change may propagate through several layers:

```text
validateToken()
      ↑
authenticate()
      ↑
createCheckoutSession()
      ↑
checkoutRoute()
      ↑
checkout.test.ts
```

A developer wants to know:

- Which symbol actually changed?
- Who depends on it?
- How far can the dependency chain propagate?
- Which test files are connected to the changed path?
- Did the signature change or only the body?
- Why was a result included?
- Is the analyzer confident about each edge?
- Did analysis hit a dynamic construct it cannot safely resolve?

Ripple answers those questions using a local static-analysis index.

---

# 3. Product thesis

Ripple is not a dependency visualizer.

It is:

> **A Git-aware, symbol-level, explainable change-impact engine with explicit uncertainty, incremental local indexing, and reproducible evaluation.**

The project is valuable because of:

1. **Git-to-symbol mapping**
2. **symbol-level graph construction**
3. **confidence-aware resolution**
4. **witness paths**
5. **incremental correctness**
6. **measured resolver quality**
7. **safety-oriented test-impact evaluation**

If a proposed feature does not strengthen one of those areas, it is probably secondary.

---

# 4. Competitive context

Change-impact tooling already exists.

Adjacent categories include:

- package-level affected analysis
- code-intelligence platforms
- test-impact systems
- symbol dependency graphs
- blast-radius tools
- Git/CI analysis tools

Ripple must **not** claim that it invented the category.

Its engineering value is narrower:

- local CLI
- Git revision awareness
- symbol-level mapping
- explainable paths
- explicit confidence states
- incremental indexing
- oracle-backed evaluation
- cold-vs-incremental correctness checks

Before public release, add `docs/prior-art.md` describing relevant systems and how Ripple’s semester scope differs.

---

# 5. Goals

Ripple should support:

1. local TypeScript repository indexing
2. symbol extraction
3. import/call resolution for a defined subset
4. persistent dependency graph storage
5. reverse dependency queries
6. Git base/head comparison
7. source-range to symbol mapping
8. change classification
9. downstream impact traversal
10. witness paths
11. edge confidence
12. unresolved boundaries
13. high-fan-out handling
14. test-file reachability
15. incremental indexing
16. cold-vs-incremental equality testing
17. resolver evaluation against trusted tooling
18. reproducible performance benchmarks
19. local installation by another developer

---

# 6. Non-goals

Do not build these during the semester unless every core milestone is complete:

- web dashboard
- hosted backend
- login
- OAuth
- JWT
- organizations
- billing
- LLM summaries
- AI code review
- full IDE
- LSP server
- Git hosting
- source-control server
- complete TypeScript compiler replacement
- runtime profiler
- coverage platform
- automatic refactoring
- multi-language platform
- distributed index

The CLI is the product.

---

# 7. Initial source scope

Support:

```text
.ts
.tsx
```

Initial semantic support:

- top-level functions
- classes
- methods
- named exports
- default exports
- named imports
- default imports
- aliases
- relative imports
- basic `tsconfig` path aliases
- direct imported function calls
- basic re-exports
- source spans
- test-file imports

Initially limited:

- precise dynamic dispatch
- dependency-injection framework wiring
- computed dynamic imports
- reflection
- decorators with hidden bindings
- event-bus string wiring
- `eval`
- runtime monkey patching
- every TypeScript project-reference case
- full monorepo support

All limitations belong in `docs/limitations.md`.

---

# 8. CLI

## Index

```bash
ripple index .
```

Purpose:

- scan repository
- parse supported files
- extract symbols
- resolve relationships
- build graph
- persist index

Example:

```text
Files discovered:         842
TypeScript files:         613
Symbols extracted:      8,431
Resolved edges:        21,904
Candidate edges:        1,102
Unresolved references:    213

Index written to:
.ripple/index.db
```

## Inspect

```bash
ripple inspect src/auth/token.ts
```

Purpose:

- show what the analyzer understands about one file
- debug parser/resolver behavior

## Impact

```bash
ripple impact validateToken
```

Example:

```text
Symbol:
src/auth/token.ts::validateToken

Direct dependents
  authenticate()
  refreshSession()

Witness path to checkoutRoute

validateToken
  <- [resolved] authenticate
  <- [resolved] createCheckoutSession
  <- [candidate] checkoutRoute
```

## Diff

```bash
ripple diff main..HEAD
```

Example:

```text
Changed symbols: 3

src/auth/token.ts::validateToken
  BODY_MODIFIED

  direct dependents:      2
  transitive dependents: 11
  reachable test files:   4

src/auth/token.ts::decodeToken
  SIGNATURE_MODIFIED

src/auth/legacy.ts::oldValidator
  DELETED
```

## Test

```bash
ripple test main..HEAD
```

Output wording must be conservative:

```text
Potentially related test files
```

Never:

```text
The only tests required
```

## Status

```bash
ripple status
```

Show:

- indexed revision
- analyzer version
- schema version
- file count
- symbol count
- config hash status
- whether full rebuild is required

---

# 9. Architecture

```text
                         ┌──────────────────────┐
                         │   Git Repository     │
                         └──────────┬───────────┘
                                    │
                                    ▼
                         ┌──────────────────────┐
                         │ Repository Scanner   │
                         └──────────┬───────────┘
                                    │
                           .ts / .tsx files
                                    │
                                    ▼
                         ┌──────────────────────┐
                         │ Tree-sitter Parser   │
                         └──────────┬───────────┘
                                    │
                                    ▼
                         ┌──────────────────────┐
                         │ Symbol Extractor     │
                         └──────────┬───────────┘
                                    │
                                    ▼
                         ┌──────────────────────┐
                         │ Symbol Resolver      │
                         └──────────┬───────────┘
                                    │
                                    ▼
                  ┌─────────────────────────────────┐
                  │      Dependency Graph           │
                  │                                 │
                  │ Symbol A ──CALLS────> Symbol B  │
                  │ File A   ──IMPORTS──> File B    │
                  └───────────────┬─────────────────┘
                                  │
                       ┌──────────┴──────────┐
                       ▼                     ▼
              ┌────────────────┐    ┌────────────────┐
              │ SQLite Index   │    │ In-memory Graph│
              │ durable state  │    │ fast traversal │
              └────────────────┘    └────────┬───────┘
                                             │
                                             ▼
                                  ┌──────────────────┐
                                  │ Impact Engine    │
                                  └────────┬─────────┘
                                           │
                        ┌──────────────────┼──────────────────┐
                        ▼                  ▼                  ▼
                    impact              diff                test
```

Git path:

```text
base/head revisions
       ↓
changed files + hunks
       ↓
changed ranges
       ↓
base/head symbol comparison
       ↓
changed symbols
       ↓
reverse dependency traversal
       ↓
witness paths
```

---

# 10. Architectural boundaries

Keep three layers separate.

## Syntax

Question:

> What constructs appear in this file?

Owned by:

```text
scanner
parser
extractor
```

## Semantics

Question:

> What actual symbol does this reference mean?

Owned by:

```text
module resolver
symbol resolver
```

## Impact

Question:

> What depends on this symbol?

Owned by:

```text
graph
Git analyzer
impact engine
```

Agents must not collapse these layers into one module.

---

# 11. Tech stack

## Rust

Responsibilities:

- CLI
- scanner
- parser orchestration
- resolver
- graph model
- SQLite integration
- Git analysis
- incremental indexing
- impact engine
- benchmarks

Likely crates:

```text
clap
tree-sitter
tree-sitter-typescript
rusqlite
gix or git2
serde
thiserror
tracing
proptest
criterion
rayon later if profiling supports it
```

Every important dependency choice should be documented in an ADR.

## Tree-sitter

Purpose:

- structured TypeScript/TSX parsing
- source spans
- error-tolerant syntax trees

Important limitation:

Tree-sitter does not provide full TypeScript semantic resolution.

## SQLite

Purpose:

- durable local index
- no server
- transactions
- migrations
- simple distribution

Files:

```text
.ripple/
  index.db
  metadata.json
```

## Git

Purpose:

- revisions
- diffs
- hunks
- renames
- base/head trees

## In-memory graph

Purpose:

fast traversal.

SQLite persists the graph. Queries load efficient adjacency structures rather than issuing SQL at every hop.

---

# 12. Repository scanner

Responsibilities:

- normalize root
- find `.ts` and `.tsx`
- apply ignore rules
- locate `tsconfig.json`
- locate `package.json`
- produce normalized relative paths

Initial excludes:

```text
node_modules/
.git/
dist/
build/
coverage/
generated/
```

Start with one TypeScript project rooted at one main `tsconfig`.

---

# 13. Parser

Input:

```text
source bytes
```

Output:

```text
Tree-sitter syntax tree
```

Example:

```ts
import { authenticate } from "./auth";

export function checkout() {
    authenticate();
}
```

Conceptually:

```text
source_file
├── import_statement
└── function_declaration
    └── call_expression
```

The parser does not resolve names.

---

# 14. Symbol extractor

Convert syntax into Ripple records.

Example:

```text
kind: FUNCTION
name: checkout
qualified_name: checkout
file: src/checkout.ts
exported: true
span: ...
signature_hash: ...
body_hash: ...
```

Initial symbol kinds:

```text
FUNCTION
CLASS
METHOD
```

Only add additional kinds when they improve analysis.

---

# 15. Source spans

Every symbol stores:

```text
start byte
end byte
start line
end line
start column
end column
```

Why:

Git gives changed text ranges.

Ripple must map:

```text
lines 42–48
```

to:

```text
validateToken
```

---

# 16. Stable symbol identity

Do not use line number as identity.

Use:

```text
project
+
normalized path
+
qualified name
+
symbol kind
+
disambiguator if needed
```

Example:

```text
app::src/auth.ts::AuthService.authenticate::METHOD
```

SQLite may still use integer IDs internally.

Stable semantic IDs matter for:

- rebuild comparison
- base/head comparison
- incremental updates
- canonical equality

---

# 17. Signature and body hashes

Every function/method stores:

```text
signature_hash
body_hash
```

Body-only change:

```text
BODY_MODIFIED
```

Signature change:

```text
SIGNATURE_MODIFIED
```

Why:

A body change may alter behavior.

A signature change may also structurally break callers.

Hash syntax structure and meaningful lexical content with versioned signature/body domains. Do not hash only whitespace-stripped token streams. Ordinary trivia-only edits are unchanged only when parsed structure and semantic content are unchanged.

Automatic semicolon insertion must be observable: `return 7;` and `return` followed by a newline then `7;` have different behavior and must have different body hashes. Include newline-containing comments, TSX text, strings and template literals in regression fixtures. Parse failures remain explicit rather than receiving unsupported semantic guarantees.

---

# 18. Module resolution

Initial rules:

- relative imports
- TypeScript extension lookup
- named imports
- default imports
- aliases
- basic `tsconfig` paths
- basic re-exports

Example:

```ts
import { login as authenticate } from "./auth";
```

Resolve:

```text
local authenticate
      ↓
import alias
      ↓
exported login
      ↓
src/auth.ts::login
```

Do not attempt every Node/TypeScript package-resolution rule before the core path works.

---

# 19. Symbol resolver

Input:

```text
checkout contains call named authenticate
checkout.ts imports authenticate from ./auth
```

Output:

```text
src/checkout.ts::checkout
  CALLS
src/auth.ts::authenticate
```

This resolver is a central part of the project.

It must be:

- deterministic
- testable
- confidence-aware
- measured against a reference

---

# 20. Confidence model

## RESOLVED

One target is determined by a supported deterministic rule.

```text
checkout -> authenticate
RESOLVED
```

## CANDIDATE

Several plausible targets remain.

```ts
repository.save();
```

May map to multiple `save()` methods.

## UNRESOLVED

No safe static target.

```ts
handlers[eventName](payload);
```

Record:

```text
reason
source span
reference kind
```

When uncertain, report uncertainty rather than inventing a precise edge.

---

# 21. Graph model

Initial node types:

```text
FILE
FUNCTION
CLASS
METHOD
TEST_FILE
OPAQUE_BOUNDARY
```

Initial edge types:

```text
DEFINES
IMPORTS
CALLS
REFERENCES
REEXPORTS
```

Each edge carries:

```text
source
target
type
confidence
provenance span
resolution rule
```

Example:

```text
source:
src/checkout.ts::checkout

target:
src/auth.ts::authenticate

type:
CALLS

confidence:
RESOLVED

provenance:
src/checkout.ts:18

rule:
named_import_direct_call
```

Impact evidence has a separate `evidence_kind`:

- `symbol_backed`: a supported symbol-level call/reference path, including exact module-level references from test files.
- `module_fallback`: conservative file-import/re-export reachability, starting from the changed symbol's defining file or a changed-file seed.

Compute and retain witnesses for both tracks independently. Do not automatically expand symbols into their containing files within the symbol-backed track. An import edge may be `RESOLVED` while its relevance to a particular changed symbol remains module fallback. When both tracks reach a result, keep both kinds of evidence.

Default potentially-related test selection uses the conservative union. Show symbol-backed, fallback-only and combined results/counts separately; narrower evidence views never guarantee safe test omission. A two-export/two-test fixture must distinguish exact symbol paths from conservative fallback additions.

---

# 22. Forward and reverse adjacency

Forward:

```text
checkout
   ↓
authenticate
   ↓
validateToken
```

Reverse:

```text
validateToken
   ↑
authenticate
   ↑
checkout
```

Impact analysis primarily uses the reverse graph.

Both directions should be loaded efficiently.

---

# 23. SQLite schema

Initial logical model; record concrete constraints and indexes in the persistence ADR before migration v1. All graph endpoints reference typed nodes so files, tests and opaque boundaries can participate alongside symbols.

## nodes

```text
id
semantic_identity (unique)
node_type
```

## files

```text
id
node_id (unique foreign key to nodes)
normalized_path (unique)
content_hash
language
last_indexed_revision
parse_status
```

## symbols

```text
id
node_id (unique foreign key to nodes)
file_id
kind
name
qualified_name
span_start_byte
span_end_byte
span_start_line
span_end_line
span_start_column
span_end_column
signature_hash
body_hash
exported
```

## edges

```text
id
source_node_id
target_node_id
edge_type
confidence
source_file_id
source_span
resolution_rule
```

## unresolved_references

```text
id
source_node_id
reference_text
reference_kind
source_span
reason
```

## index_metadata and captured inputs

```text
schema_version
analyzer_version
hash_version
repository_revision
input_manifest_hash
worktree_dirty
effective_config_hash
generation
created_at
```

Persist the captured input manifest, effective configuration inputs, parse diagnostics and reusable syntax facts needed to reproduce a generation and support later incremental updates. Provenance bookkeeping such as per-file last-indexed revision is operational metadata, excluded from canonical semantic equality.

Use foreign keys and migrations. Publish complete generations in a transaction and load each graph generation in a read transaction. SQLite is authoritative; `metadata.json` is a derived summary recovered from the committed database when out of sync.

---

# 24. Impact engine

Use reverse BFS initially.

Why:

- direct dependents appear first
- distance comes naturally
- shortest witness path is easy
- depth limits are simple

Example:

```text
depth 0: validateToken
depth 1: authenticate
depth 2: createSession
depth 3: checkoutRoute
```

Use a visited set for cycles.

---

# 25. Witness paths

Required output:

```text
validateToken
  <- [resolved] authenticate
  <- [resolved] createSession
  <- [candidate] checkoutRoute
```

Each hop should retain:

- source
- target
- edge type
- confidence
- provenance

This is more valuable than an opaque score.

---

# 26. Hub-node handling

A shared logger may reach most of the repo.

Avoid dumping thousands of nodes.

Initial summary:

```text
High fan-out node encountered:
logger()

Direct dependents: 61
Depth 2:          284
Depth 3+:       1,479
```

CLI controls:

```bash
ripple impact logger --depth 2
ripple impact logger --limit 100
```

Order by:

1. distance
2. confidence
3. deterministic tie-break

---

# 27. Git analyzer

Input:

```bash
ripple diff main..HEAD
```

Git provides:

```text
base
head
changed files
added files
deleted files
renamed files
hunks
```

Map:

```text
hunk
 ↓
changed source span
 ↓
symbol span lookup
 ↓
changed symbol
```

Example:

```text
src/auth.ts lines 42–48
```

maps to:

```text
src/auth.ts::validateToken
```

---

# 28. Two-revision model

A deleted symbol exists only in base.

Therefore Ripple cannot rely only on the head index.

Concept:

```text
BASE INDEX
   +
HEAD INDEX
```

Compare symbols across both.

Correctness comes before optimizing this model.

---

# 29. Change classification

Required:

```text
ADDED
DELETED
BODY_MODIFIED
SIGNATURE_MODIFIED
UNCHANGED
```

Later if reliable:

```text
MOVED
RENAMED
```

If rename confidence is weak, represent it as delete + add.

---

# 30. Global invalidators

Changes to some files may invalidate resolution globally.

Examples:

```text
tsconfig.json
package.json
lockfiles
```

Initial behavior:

```text
global configuration changed
→ require full rebuild
```

Keep this simple.

---

# 31. Incremental indexing

Cold:

```text
scan all
parse all
resolve all
persist all
```

Incremental:

```text
detect changed file
       ↓
compare hash
       ↓
reparse changed file
       ↓
re-extract facts
       ↓
replace outgoing edges
       ↓
re-resolve importers if needed
       ↓
stop when externally visible facts no longer change
```

Start with file-level invalidation.

Worktree indexing captures supported file membership and source/config/ignore bytes once and analyzes those captured bytes. Before publication, verify the manifest with a fresh scan and content hashes, including HEAD provenance. Retry capture at most twice; persistent concurrent modification reports an actionable error and preserves the previous generation. This is a verified captured-input view, not an OS-wide atomic filesystem snapshot. Edits after verification can make it stale and must be detected by later worktree queries.

Use immutable Git blobs for historical inputs without checking out/stashing user files. Benchmark freshness verification separately from adjacency traversal; a fast in-memory BFS does not imply an equally fast verified end-to-end CLI query.

---

# 32. Early cutoff

If internal code changes but exported facts do not:

```text
local change
   ↓
export table unchanged
   ↓
stop dependency-resolution propagation
```

This must be guarded by correctness tests.

Establish a passing generated incremental-versus-cold edit-sequence suite, idempotence tests and persistence round trips before integrating selective invalidation or early cutoff. Expand those suites and add real history replay afterward; do not defer the initial correctness harness until after optimization.

---

# 33. Core incremental invariant

For repository `R1` and change `Δ`:

```text
canonical(IncrementalIndex(R1, Δ))
==
canonical(ColdIndex(R1 + Δ))
```

Canonical equality must ignore:

- SQLite insertion order
- internal numeric IDs
- thread scheduling
- nondeterministic iteration

Compare semantic content:

- symbols
- normalized paths
- hashes
- edges
- confidence

Canonical equality also includes source spans/provenance, parse state, unresolved records, diagnostics, evidence-relevant facts and effective configuration semantics. Ignore timestamps, benchmark timings and generation counters as well as internal storage IDs; do not hide stale diagnostics or provenance to obtain equality.

---

# 34. Test-file detection

Start at file level.

Defaults:

```text
*.test.ts
*.test.tsx
*.spec.ts
*.spec.tsx
```

Do not analyze individual `it()` blocks initially.

Implement these default patterns during initial source discovery, before `diff` needs reachable-test counts. The later test-impact milestone builds the dedicated command, revision-aware selection and evaluation on this existing classification.

---

# 35. Test reachability

Example:

```text
checkout.test.ts
      ↓
checkout.ts
      ↓
authenticate
      ↓
validateToken
```

A `validateToken` change can reverse-traverse to `checkout.test.ts`.

Report:

```text
Potentially related test file
```

Never claim:

```text
Only required test
```

---

# 36. Test safety evaluation

A smaller test set is useless if it misses important failing tests.

Measure:

## selection ratio

```text
selected test files / all test files
```

## fault-catching recall

Basic mutation process:

1. apply controlled behavior change
2. run full suite
3. record failing test files
4. run Ripple selection
5. measure how many failing files were selected

Example:

```text
failing test files: 17
selected failing files: 16
recall: 16/17
```

Document methodology and limitations.

---

# 37. Oracle evaluation

Use stronger TypeScript tooling as a reference.

Candidates:

```text
scip-typescript
TypeScript language service/compiler API
```

Ripple does not need to beat them.

It needs to measure how correct its own supported resolver is.

---

# 38. Oracle mapping

Reference granularity may differ.

Define:

```text
oracle reference
   ↓
oracle symbol
   ↓
source span
   ↓
Ripple enclosing symbol
   ↓
comparable graph edge
```

Without this mapping, precision/recall is ambiguous.

---

# 39. Resolver metrics

Track:

```text
precision
recall
resolved edge count
candidate edge count
unresolved count
failure category
```

Failure categories may include:

```text
re-export
path alias
receiver ambiguity
dynamic import
unsupported syntax
```

Use these numbers to choose future resolver work.

---

# 40. Benchmark metrics

Measure:

| Metric | Purpose |
|---|---|
| cold indexing time | initial cost |
| files/sec | processing rate |
| LOC/sec | scale-normalized rate |
| incremental p50/p95 | update responsiveness |
| impact query p50/p99 | traversal responsiveness |
| peak RSS | memory |
| index size | storage |
| resolver precision | correctness |
| resolver recall | coverage |
| test selection ratio | efficiency |
| fault-catching recall | safety-oriented behavior |

Every public number must be reproducible.

---

# 41. Benchmark records

Every benchmark result records:

```text
Ripple version
repo URL
repo commit SHA
hardware
OS
CPU
RAM
configuration
cold/warm state
run count
reported statistic
```

Store commands/scripts under:

```text
bench/
eval/
```

---

# 42. Testing

## Unit tests

One per extraction/resolution rule.

## Golden fixtures

Example fixture folders:

```text
named-import/
default-import/
alias-import/
barrel-export/
circular-import/
tsconfig-path/
body-change/
signature-change/
deleted-symbol/
dynamic-call/
```

## Property tests

Required:

```text
incremental == cold
re-index unchanged input is idempotent
serialization preserves canonical state
```

## Differential tests

Compare with oracle.

## Commit replay

Replay real commit sequences and compare incremental/cold.

## Fuzzing

Optional after core stability.

## Performance tests

Track regressions without fragile shared-runner thresholds.

Generated cold-equality, no-op idempotence and persistence round-trip tests begin with the conservative incremental updater and gate early cutoff. Real-history replay owns repair of any defects it reveals (or explicit repair prerequisite tasks); a failing replay never unlocks the dependent benchmark task. Pin seeds, corpus commits and analyzer commits, and rerun affected evaluation evidence on the final integrated candidate.

---

# 43. Repository structure

```text
ripple/

├── crates/
│   ├── ripple-core/
│   ├── ripple-parse/
│   ├── ripple-resolve/
│   ├── ripple-index/
│   ├── ripple-git/
│   ├── ripple-impact/
│   └── ripple-cli/
│
├── fixtures/
├── eval/
│   ├── oracle/
│   ├── commit-replay/
│   ├── mutation/
│   └── corpora/
│
├── bench/
│   ├── scripts/
│   └── results/
│
├── docs/
│   ├── thesis.md
│   ├── architecture.md
│   ├── graph-model.md
│   ├── resolution.md
│   ├── incremental.md
│   ├── evaluation.md
│   ├── benchmarks.md
│   ├── limitations.md
│   ├── prior-art.md
│   └── adr/
│
├── .github/
│   ├── workflows/
│   ├── ISSUE_TEMPLATE/
│   └── pull_request_template.md
│
├── AGENTS.md
├── ripple_buildspec.md
├── CONTRIBUTING.md
├── CHANGELOG.md
├── Cargo.toml
├── README.md
└── LICENSE
```

---

# 44. ADRs

Initial ADRs:

```text
0001-rust-core.md
0002-tree-sitter-parser.md
0003-sqlite-persistence.md
0004-symbol-identity.md
0005-base-head-model.md
0006-confidence-semantics.md
0007-canonical-graph-equality.md
0008-oracle-selection.md
```

ADR format:

```text
Context
Options
Decision
Reasoning
Consequences
Revisit condition
```

---

# 45. AGENTS.md rules

Agents must read:

1. this build spec
2. relevant ADRs
3. existing tests/fixtures
4. crate ownership

Before coding, state:

```text
problem
expected behavior
non-goals
affected invariant
acceptance test
```

Agents must:

- keep syntax/resolution/impact layers separate
- avoid frontend/auth/cloud work
- preserve deterministic output
- add fixtures for semantic behavior
- preserve explicit uncertainty
- not invent unsupported guarantees
- update ADRs for architecture changes

Before completion report:

```text
what changed
why
tests run
fixtures added
invariants affected
performance impact
known limitations
follow-up issues
```

---

# 46. Timeline

## Week 1: foundation + extraction

Implement:

- workspace
- CLI
- scanner and test-file pattern classification
- Tree-sitter
- functions/classes/methods
- imports/exports
- calls
- spans
- fixtures
- CI
- `inspect`

Exit:

```text
ripple inspect <fixture>
```

returns deterministic expected facts.

## Week 2: resolver + graph

Implement:

- semantic IDs
- relative module resolution
- imports/defaults/aliases
- direct calls
- graph
- confidence
- SQLite
- reverse adjacency
- witness paths
- `impact`

Exit:

multi-file fixture returns correct reverse path.

## Week 3: oracle harness

Implement:

- reference tool
- mapping
- precision/recall
- mismatch categories
- first resolver improvements based on data

Exit:

one reproducible accuracy report on a pinned repo.

## Week 4: Git impact

Implement:

- base/head
- hunks
- range-to-symbol
- body/signature hashes
- change classification
- basic rename handling
- `diff`
- hub controls

Exit:

real branch works end to end:

```text
Git diff
→ changed symbols
→ impact
→ witness paths
```

**Resume checkpoint:** after this milestone, the project can already appear as active work if it works on real repositories.

## Week 5: incremental indexing

Implement:

- content hashes
- changed-file update
- edge replacement
- importer invalidation
- generated cold-equality/idempotence/round-trip baseline before early cutoff
- early cutoff guarded by that baseline
- global invalidators
- canonical representation

Exit:

one-file edit avoids full parse.

## Week 6: incremental correctness

Implement:

- expanded cold-vs-incremental property campaigns
- idempotence
- deterministic serialization
- commit replay with repairs before performance handoff
- incremental benchmarks

Exit:

one pinned real commit sequence passes equality at every tested step.

## Week 7: test impact

Implement:

- revision-aware test selection using existing detection
- separate symbol-backed and module-fallback test reachability
- witness paths
- `test`
- selection ratio

Exit:

real repo produces explainable test-file set.

## Week 8: safety evaluation + release

Implement:

- mutation or historical failure evaluation
- fault-catching recall
- baseline comparison
- performance benchmarks
- profiling
- first data-driven optimization
- release binaries
- docs
- changelog

Exit:

another developer can install, index, diff, and understand limitations.

---

# 47. Agent work split

Agents can handle heavily:

- CLI boilerplate
- SQLite plumbing
- migrations
- CI
- fixture generation
- straightforward tests
- serialization
- benchmark scripts
- release automation
- mechanical refactors
- documentation cleanup

The project owner must understand:

- graph model
- symbol identity
- module resolution
- symbol resolution
- confidence meanings
- two-revision model
- signature/body distinction
- invalidation
- early cutoff
- canonical equality
- oracle mapping
- precision/recall
- test safety methodology
- benchmark interpretation

---

# 48. GitHub workflow

Use focused commits:

```text
parse: extract exported function declarations
resolve: support named import aliases
graph: add reverse adjacency index
git: map hunks to enclosing symbols
git: distinguish signature and body changes
incremental: invalidate importers after export changes
eval: compare call edges against SCIP
impact: emit confidence-aware witness paths
```

Issues should contain measured evidence when possible.

PR template:

```text
What changed?
Why?
Which subsystem owns it?
Which invariant changes?
How was it tested?
Did resolver metrics change?
Did performance change?
What remains unsupported?
```

---

# 49. Releases

Suggested:

```text
v0.1.0  parser + resolver + graph + impact
v0.2.0  Git-aware diff
v0.3.0  incremental indexing
v0.4.0  test reachability + evaluation
```

Use `v1.0.0` only when:

- CLI behavior is stable
- migration policy exists
- limitations are documented
- evaluation is reproducible
- install experience is reliable

Candidate gates are capability-specific. v0.1 requires implemented inspect/index/status/impact installation and witness checks; v0.2 adds Git endpoint/deletion behavior and an oracle baseline; v0.3 adds property-guarded incremental updates and successful real replay; v0.4 semester readiness adds the dedicated test command, final safety/performance evidence and complete documentation/install checks. Earlier candidates do not require unfinished semester features.

The semester checklist applies to final v0.4 readiness; its public tag/release occurs only when publication is authorized. Final accuracy, replay, safety and performance reports must identify the integrated analyzer version they measure. Measure precise and conservative-union selection separately against file-import/all-tests baselines, even when conservative fallback offers no reduction.

---

# 50. Installation

Development:

```bash
cargo install --path crates/ripple-cli
```

Public later:

```text
GitHub Releases
crates.io
Homebrew tap if useful
```

Target:

```text
macOS
Linux
Windows
```

The user should not need:

- DB server
- web server
- account
- cloud credentials

---

# 51. Frontend decision

No frontend is required.

Optional after core completion:

```bash
ripple graph <symbol> --format dot
```

or a local read-only visualization.

Do not spend semester build time on React.

---

# 52. Authentication decision

No authentication.

No accounts.

No sessions.

No hosted repository upload.

Ripple analyzes local code.

---

# 53. Risks and mitigations

## TypeScript scope expands too far

Use:

- one language
- supported subset
- confidence tiers
- early oracle evaluation
- explicit limitations

## Tool becomes only a graph viewer

Keep central workflow:

```text
Git diff
→ changed symbols
→ impact
→ witness path
```

## Impact becomes entire repo

Use:

- distance grouping
- high-fan-out warnings
- depth limits
- output limits

## Test selection is oversold

Use:

- conservative wording
- unresolved boundaries
- safety-oriented evaluation

## Incremental result is stale

Require:

```text
incremental == cold
```

## Agent-written code becomes hard to defend

Require owner understanding of architecture/resolver/incremental/evaluation logic.

---

# 54. 30-second explanation

> **Ripple is a Rust CLI for TypeScript repositories. It parses the repo, resolves functions and imports into a dependency graph, then takes a Git diff and maps changed lines to changed symbols. From those symbols it walks the reverse graph to find downstream callers and test files, and it shows the dependency path behind every result. I also track resolution confidence, update the index incrementally, and compare the resolver against TypeScript code-intelligence tooling so its accuracy is measured rather than assumed.**

---

# 55. Interview system-design explanation

> **The repository scanner discovers TypeScript source files. Tree-sitter produces syntax trees and spans. The extractor converts those into Ripple symbols and unresolved references. The resolver maps supported imports and calls to concrete targets and emits confidence-annotated edges. SQLite stores the durable index, while forward and reverse adjacency structures are loaded in memory for fast queries. Git analysis compares base and head revisions, maps changed ranges to symbols, and classifies body/signature/add/delete changes. The impact engine performs reverse BFS and keeps predecessor information for witness paths. Incremental indexing reprocesses changed files and affected importers, and correctness is checked by comparing canonical incremental output with a clean rebuild.**

---

# 56. Questions the owner must answer

1. Why Tree-sitter?
2. What semantic information does Tree-sitter lack?
3. How are symbols identified across edits?
4. How do import aliases resolve?
5. How are re-exports handled?
6. What does `RESOLVED` guarantee?
7. What happens with ambiguous method calls?
8. What happens at dynamic imports?
9. Why use base and head indexes?
10. How is body change different from signature change?
11. Why SQLite plus an in-memory graph?
12. Why reverse BFS?
13. How are witness paths reconstructed?
14. How are cycles handled?
15. How are hub nodes handled?
16. What causes invalidation?
17. When can early cutoff occur?
18. What exactly does incremental-equals-cold compare?
19. How is canonicalization defined?
20. What is the oracle?
21. How does oracle mapping work?
22. What do resolver precision and recall mean?
23. Why is test selection not complete?
24. What does fault-catching recall mean?
25. What bottleneck did profiling identify?

---

# 57. Claims policy

Acceptable:

```text
maps supported Git changes to symbols
reports statically reachable downstream dependencies
provides witness paths
reports confidence
measures resolver agreement against a reference
updates supported index state incrementally
```

Do not claim without proof:

```text
knows what will break
finds every dependency
fully supports TypeScript
guarantees selected tests are sufficient
```

---

# 58. Resume framing

```text
Ripple — Change Impact Engine
Rust, Tree-sitter, SQLite, Git
```

Current bullets:

- Building a Rust static-analysis engine that maps TypeScript Git diffs to symbol-level dependency graphs, tracing downstream impact across functions, modules, and related test files.
- Designing incremental indexing, confidence-aware symbol resolution, and reproducible evaluation against TypeScript code-intelligence references to measure resolver accuracy and impact-query performance.

Replace general wording with real measurements as soon as benchmarks exist.

---

# 59. Semester definition of done

## Core

- [ ] CLI
- [ ] scanner
- [ ] TypeScript/TSX parser
- [ ] symbol extraction
- [ ] spans
- [ ] stable semantic IDs
- [ ] signature/body hashes
- [ ] basic module resolution
- [ ] basic call resolution
- [ ] confidence model
- [ ] persistent graph
- [ ] reverse traversal
- [ ] witness paths

## Git

- [ ] base/head
- [ ] hunks
- [ ] range-to-symbol
- [ ] add/delete/body/signature classification
- [ ] basic rename handling
- [ ] global invalidators
- [ ] `ripple diff`

## Evaluation

- [ ] oracle harness
- [ ] mapping documented
- [ ] precision/recall report
- [ ] mismatch categories

## Incremental

- [ ] content hashes
- [ ] changed-file updates
- [ ] importer invalidation
- [ ] early cutoff
- [ ] canonical representation
- [ ] incremental-vs-cold property
- [ ] real commit replay

## Test impact

- [ ] test patterns
- [ ] test reachability
- [ ] witness paths
- [ ] selection ratio
- [ ] safety-oriented evaluation

## Release

- [ ] benchmarks
- [ ] docs
- [ ] limitations
- [ ] prior-art doc
- [ ] ADRs
- [ ] AGENTS.md
- [ ] CI
- [ ] tagged release
- [ ] install instructions
- [ ] dogfood on an external repo

---

# 60. Scope freeze

For this semester:

> **Ripple is a Rust CLI that analyzes TypeScript repositories, builds a persistent confidence-annotated symbol dependency graph, maps Git diffs to changed symbols, traces downstream impact with explainable witness paths, updates its index incrementally, and evaluates resolver and test-impact behavior with reproducible measurements.**

It is not:

```text
a SaaS app
a frontend project
an AI reviewer
a cloud service
a multi-language platform
a Git hosting product
```

Any scope expansion must answer:

1. What measured limitation does it fix?
2. Why does it belong in the core thesis?
3. Which milestone would be delayed?

If those answers are weak, reject the feature.

---

# 61. Mental model

Ripple has six stages:

```text
1. UNDERSTAND SOURCE
TypeScript
   ↓
symbols + references

2. RESOLVE
imports/calls
   ↓
confidence-aware graph

3. UNDERSTAND CHANGE
Git base/head
   ↓
changed symbols

4. TRACE IMPACT
changed symbols
   ↓
reverse traversal

5. EXPLAIN
result
   ↓
witness path + confidence

6. VERIFY
oracle + properties + benchmarks
   ↓
measured correctness and performance
```

---

# 62. Name

## Final project name: Ripple

Ripple describes how a code change can propagate through downstream dependencies.

Use **Ripple** for the product, `ripple` for the repository and CLI, `ripple-` prefixes for crates, and `.ripple/` for the local index directory.

```bash
ripple index .
ripple diff main..HEAD
ripple impact validateToken
ripple test main..HEAD
```

---

# 63. Agent priority order

Agents must optimize in this order:

```text
correctness
>
explicit uncertainty
>
explainability
>
measured behavior
>
performance
>
feature count
>
presentation polish
```

If a relationship cannot be resolved safely, mark it unresolved.

If incremental behavior changes, compare it with cold rebuilds.

If resolver behavior changes, update fixtures and oracle evaluation.

If hot-path behavior changes, benchmark it.

If architecture changes, update or add an ADR.

---

# 64. Canonical status

This file is the authoritative implementation scope for Ripple during the semester.

Agents should not infer requirements from older brainstorming notes.

If another document conflicts with this file:

1. this file controls unless a newer ADR explicitly supersedes a section
2. intentional scope changes must update this file
3. architectural changes require an ADR
4. README claims must describe implemented behavior, not planned behavior
