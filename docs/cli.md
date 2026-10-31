# Using Ripple

The available analysis command is `inspect`. It shows what Ripple extracts from one TypeScript/TSX file; it does not resolve dependencies or calculate change impact yet.

```sh
cargo run --locked -p ripple-cli -- inspect src/caller.ts --root fixtures/inspect/project
cargo run --locked -p ripple-cli -- inspect src/view.tsx --root fixtures/inspect/project --format json
```

After installing, use `ripple inspect src/file.ts` from your project root. Use `--root /path/to/project` when invoking it elsewhere; the file argument remains relative to that root. Supported untracked files are included, and repository ignores/mandatory exclusions apply. Ignored, excluded, missing and symlink files cannot be inspected. Root selection is explicit and does not walk to a Git ancestor.

Human output includes symbols, signature/body fingerprints, imports/exports, lexical scopes/bindings and references with source coordinates and symbol/file ownership. Coordinates use half-open UTF-8 bytes and one-based lines/byte-columns. Tree-sitter syntax and matching names do not prove semantic dependencies.

## JSON and diagnostics

`--format json` can appear before or after `inspect`. Stdout is one versioned document with these fields:

| Field | Meaning |
|---|---|
| `format_version` | CLI report version, currently 1; separate from database/hash versions. |
| `command` | `inspect`. |
| `data` | Syntax stage, resolver availability, analyzer token, file classification and extracted facts. |
| `diagnostics` | Sorted path/code/severity/message records with optional source spans. |
| `incomplete_analysis` | Non-clean parsing or reported discovery/syntax diagnostics. |

Diagnostics also go to stderr in both formats. With JSON, capture stdout for tools and stderr for readable explanations. Reported source text is escaped in human output. No timestamps, machine root paths or timing measurements enter successful output by default, so supported fixture reports are byte-stable across identical captures.

`resolution_available` is always false at this stage. `incomplete_analysis: false` means no known syntax/discovery issue was reported; it does not prove semantic completeness, resolved imports or safe test skipping. Recovered parsing and unsupported/dynamic forms stay visible. See [limitations](limitations.md) and [the output decision](adr/0014-inspect-output.md).

## Exit status

| Code | Meaning |
|---|---|
| 0 | Report produced, possibly incomplete; or help/version. |
| 1 | Operational failure: discovery/input/output failure, unavailable selected file or not-yet-implemented command. No success report is emitted. |
| 2 | Usage error: missing/invalid arguments, unsupported extension, invalid relative path or format. |

Inspection currently captures the whole selected root before analyzing the chosen file. A bad unrelated discovered input can fail the command; a skipped symlink elsewhere can make its discovery report incomplete. Input limits and symlink policy are inherited from the scanner/parser. It does not write `.ripple/`, change Git state or publish a freshness-verified index.

`index`, `impact`, `diff`, `test` and `status` are registered help placeholders and return an explicit failure. Their implementation belongs to later milestones.
