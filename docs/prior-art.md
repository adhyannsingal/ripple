# Prior art

Reviewed 2026-11-05 using primary project documentation and the original research paper. This is a scope comparison, not a performance ranking or an exhaustive survey.

| System / source | Relevant approach | Relationship to Ripple |
|---|---|---|
| [Nx affected](https://nx.dev/docs/features/ci-features/affected) | Current v24 docs select tasks using changed files, declared inputs and output dependencies; earlier versions used project-based affected. | Established affected-task orchestration. Ripple focuses on supported TypeScript symbols, revision-bound witnesses and explicit static uncertainty. No comparative benchmark was run. |
| [Turborepo run --affected](https://turborepo.dev/docs/reference/run#--affected) | Git-based affected package filtering by default; documented task-input mode is also available. | Established repository task filtering. Ripple's narrow language graph and potentially related test report are different evidence granularity. |
| [SCIP protocol](https://github.com/scip-code/scip) | Language-independent index format for navigation, with definitions/references and language indexers. | A stronger tooling ecosystem relevant to oracle choices; Ripple uses a pinned TypeScript compiler API projection, not a claim to replace complete code intelligence. |
| [TypeScript compiler API](https://github.com/microsoft/TypeScript/wiki/Using-the-Compiler-API) | Compiler program/source/type-checker APIs. | Evaluation reference for eligible edges; production uses bounded Tree-sitter syntax plus a declared semantic subset. |
| [Ekstazi paper: Practical Regression Test Selection with Dynamic File Dependencies](https://users.ece.utexas.edu/~gligoric/papers/GligoricETAL15Ekstazi.pdf) (Gligoric, Eloussi, Marinov, 2015) | Runtime file dependencies for regression-test selection, implemented/evaluated for Java/JUnit. | Demonstrates established RTS research. Ripple's static incomplete TypeScript evidence and small owned mutation campaign do not inherit that paper's safety/evaluation conclusions. |

Ripple's engineering focus is local immutable Git comparison, semantic IDs and structured hashes, independent symbol/file-fallback witnesses, verified atomic incremental state, and reproducible scoped evidence. Dynamic constructs and global state yield known misses; conservative union can equal file-import selection. [Evaluation](evaluation.md) reports those outcomes rather than novelty or safe-skip claims. Documentation/version behavior can change; links and comparisons are dated above.
