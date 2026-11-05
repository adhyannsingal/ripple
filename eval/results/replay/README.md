# M6-T02 replay evidence

`replay.json` records five exact upstream commits plus three curated add/config/delete steps, repeated with fresh storage: **16/16 full canonical comparisons passed**, zero mismatches, identical semantic/counter/diagnostic results across repeats. Source commit and manifest/helper hashes are in the artifact. Reproduction: [runner instructions](../../commit-replay/README.md).

Parsed/reused source files across the first campaign: 4/0, 4/0, 3/1, 2/2, 1/3, 1/4, 5/0 (global config fallback), 0/4 (delete). All steps retain incomplete analysis: 696→651 unresolved references in the upstream projection, with diagnostics retained in the raw artifact. This compares incremental equivalence, not TypeScript correctness or accuracy. Package/export/config limitations remain visible; it is not whole-repository compatibility. Debug update observations are not performance claims.

No mismatch was discovered, so no repair or minimized failure is claimed. M6-T01 generated/fault/race checks remain required; M6-T03 benchmarks only after integration verification.
