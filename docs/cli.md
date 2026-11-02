# Using Ripple

Implemented commands are `index`, `inspect`, `status` and `impact`. The binary is `ripple 0.1.0-dev`; Git is required for persistent commands, and SQLite is bundled. Node, a database server and project-script execution are not required.

From a supported single-project Git root:

```sh
ripple index .
ripple status
ripple impact authenticate
ripple impact src/auth.ts::authenticate --format json
ripple impact authenticate --depth 2 --limit 100
ripple inspect src/auth.ts
```

Install locally with `cargo install --path crates/ripple-cli --locked --root /path/to/install`; the executable is under its bin directory. To run without installation, prefix commands with `cargo run --locked -p ripple-cli --`. `index ROOT` accepts a positional Git root. The other commands accept `--root ROOT` and default to the current directory. Inspect can operate on an explicit non-Git root; persistent commands require the Git repository root and reject submodules/nested Git projects.

## Index and freshness

Index always performs a cold build. It captures supported `.ts`/`.tsx`, config/inheritance, ignore, package and lock inputs, analyzes captured bytes, verifies a fresh manifest and HEAD/dirty observation, then atomically publishes a complete `.ripple/index.db` generation. Concurrent changes cause at most two retries; continued changes/failures preserve the previous generation. Repeated unchanged cold builds have equal canonical semantic state, despite different generation/time metadata.

Status reports stored origin/analyzer/schema/config metadata and factual file/symbol/edge/unresolved counts only after content/path/config/HEAD verification. Impact applies the same verification before querying. Stale/changing inputs fail with a refresh instruction; neither command silently updates an index. Edits after the final verification can make it stale immediately, so this is a verified captured-input view, not an OS-wide atomic snapshot. Queries read/hash inputs and recompute module completeness; no graph-only latency is claimed.

`metadata.json` and `modules-N.json` are derived summaries/evidence caches. If a derived write is pending after DB commit, index reports the committed generation and pending state. SQLite remains authoritative. Incompatible database formats are rejected; safely archive incompatible local `.ripple` state while no writer uses it, then build again. No destructive automatic migration or cache deletion is performed.

Git provenance uses repository-local settings with system/global configuration isolated, filesystem monitor commands disabled and optional index writes disabled. External clean/process filters and submodules are unsupported and rejected. Git dirty status follows that declared policy, not arbitrary user-global conversion/ignore settings. The user worktree and Git index are not modified.

## Impact evidence

Choose a unique symbol by qualified name (`Class.method` where supported), path plus name (`src/file.ts::name`), or exact semantic key. Ambiguous short names fail with qualified alternatives.

Human output shows separate `SymbolBacked` and `ModuleFallback` witnesses, original hop rules/spans, confidence, counts and potentially related tests. Imports alone never prove a particular symbol dependency. JSON retains all evidence and symbol-backed/fallback-only/combined test-file sets within the requested depth. `--limit` only limits human display rows; `--depth` limits traversal, reports frontier nodes and marks the report incomplete. Zero-hop seed evidence is retained. No selected set is claimed sufficient to skip other tests.

## Inspect

```sh
cargo run --locked -p ripple-cli -- inspect src/caller.ts --root fixtures/inspect/project
cargo run --locked -p ripple-cli -- inspect src/view.tsx --root fixtures/inspect/project --format json
```

Inspect remains syntax-only (`resolution_available: false`), with supported symbols, hashes, import/export/scope/reference facts and diagnostics. It writes no index/Git state and does not verify persistent freshness. File arguments remain relative to the explicit root; ignored/excluded/missing/symlink files fail. Supported untracked inputs are included. Source coordinates use half-open UTF-8 bytes and one-based lines/byte-columns. Inspect currently captures the whole selected root, so another invalid input can fail it.

## JSON, diagnostics and exits

`--format json` works before or after an implemented command. Stdout is one document with `format_version: 1`, command, data and `incomplete_analysis`; inspect additionally exposes its detailed diagnostics envelope. Persistent reports include provenance/counters/results and relevant config/discovery diagnostics in data. Human output escapes controls/direction markers; JSON preserves original values. Inspect output excludes timestamps; index/status/impact intentionally include operational generation/time and Git provenance. Incomplete static analysis warns on stderr and can exit 0; tools must inspect the completeness fields.

| Exit | Meaning |
|---|---|
| 0 | Report produced, possibly incomplete; help/version. |
| 1 | Operational failure, missing/stale/incompatible index, unknown/ambiguous symbol or unimplemented command. No success report. |
| 2 | Invalid/missing arguments or format. |

`diff` and `test` remain explicit unimplemented placeholders. Supported config/language limitations and bounds are in [limitations](limitations.md), [resolution](resolution.md) and [cold-index ADR](adr/0021-cold-index.md). Reproduce the installed capability gate with `python3 scripts/m2-smoke.py --install-root /path/to/install`.
