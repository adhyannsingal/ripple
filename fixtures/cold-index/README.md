# M2 cold-index demo

`lib.ts` has unrelated exports a/b; caller and a.test.ts depend on a, while b.test.ts depends on b. The supported symbol path is a → caller → a.test.ts in reverse dependency order. Symbol-backed test evidence contains only a.test.ts; conservative module fallback additionally includes b.test.ts. The package script is fixture data and must never execute.

Run `python3 scripts/m2-smoke.py --binary target/debug/ripple` or pass `--install-root` after a clean cargo install. The script copies this corpus into a disposable Git root with spaces/Unicode, creates a fixture commit and exercises help/version/inspect/index/status/impact in both formats, depth/display semantics, stale detection and unchanged Git-index bytes. No performance or test-sufficiency claim follows from the expected sets.
