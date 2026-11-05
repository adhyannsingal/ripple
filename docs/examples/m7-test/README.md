# Pinned upstream test-impact demo

Two historical comparisons from the MIT-licensed [jridgewell/sourcemap-codec](https://github.com/jridgewell/sourcemap-codec) repository are frozen here. The [upstream license notice](UPSTREAM-LICENSE) is retained. [License at the pinned head](https://github.com/jridgewell/sourcemap-codec/blob/b3bb7f01c1c0dc8a5fa95c071e468f261d9e328c/LICENSE).

```sh
cargo build --locked -p ripple-cli
python3 docs/examples/m7-test/reproduce.py --binary target/debug/ripple --output /tmp/ripple-test-demo-new
```

Output must be a new directory. Optional `--repository PATH` reuses local upstream objects. The script fetches exact pinned commits into a disposable bare-worktree directory without checkout, dependency installation or upstream script execution. Ripple itself never fetches or runs tests. Each subprocess has a 180-second timeout. Both fresh-fetch and existing-object runs produced identical raw JSON hashes, and every comparison is run twice for byte equality. Analyzer source commit, binary hash, versions and report hash are recorded in each summary; build the binary from the recorded analyzer source to reproduce those artifacts.

| Example | Base → head | Symbol-backed / head tests | Combined / head tests |
|---|---|---:|---:|
| [Source-only](source-only/summary.json) | `154656b1be82e3104620a05c4bf3430e1db9507d` → `b3bb7f01c1c0dc8a5fa95c071e468f261d9e328c` | 0/2 | 0/2 |
| [Source and test](source-and-test/summary.json) | `ed80f4c897040892c8802fdf70008be976db2a79` → `d06c9ebeb50ba02077239d79549618f2c888bb78` | 0/2 | 1/2 |

Both analyses are incomplete. Source-only retains 849 graph-wide unknowns per endpoint; source-and-test retains 923/852. Original package/config/syntax limitations are retained; this is the complete committed repository under Ripple's scanner policy, not the source-only oracle projection. The second selection includes changed `test/scopes.test.ts` through fallback. It does not establish accurate dependency inference. Combined equals module-fallback selection in both examples; no precision gain is claimed. **An empty incomplete selection is not evidence that tests may safely be omitted.** No upstream tests were executed and no fault-catching metric is measured.

Each example directory contains `report.json.gz` (complete format-v1 JSON; uncompressed hash in summary), `report.txt` and `stderr.txt`. Owned fixtures establish exact supported behavior, denominator/zero cases, deleted/unindexed evidence and preserved Git/index state separately from this limited external evidence.
