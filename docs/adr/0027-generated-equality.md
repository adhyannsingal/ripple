# ADR 0027 — Generated equality guard precedes selective invalidation

Status: accepted for M5-T05

Use a small deterministic generator in an integration test without adding a test dependency or runtime service. Pin seed/policy/bounds in `fixtures/incremental/regressions/basic-seeds.json`; vary call-chain width and re-export targets while exercising every operation class in each default sequence. A single-seed override supports replay; edit length is bounded at 64. Default CI runs eight seeds × twelve edits through the existing locked workspace test command.

After every edit, compare `CanonicalAnalysis` with a new cold build in a distinct directory, including full snapshot and resolver/config/discovery evidence. Reopen persistent state and run a no-op refresh to establish round-trip equality/idempotence and zero no-op parses. Explicit fault tests delete call edges, weaken confidence and inject stale diagnostics, and require the same comparator to report a seed/step. First divergent seeds and optional input snapshots make real failures reproducible; minimized regressions must be retained before handoff. No real mismatch was observed in the initial bounded campaign.

This guard runs on the conservative T02 updater before any selective cutoff is integrated. T03 changes must continue passing it. M6 expands adversaries, shrinking/regressions and real history; this suite is a baseline, not proof for all TypeScript projects or test sufficiency. No production code changes or performance claims are introduced.
