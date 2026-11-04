# Pinned upstream Git diff demo

This analyzes the complete committed sourcemap-codec repository on upstream `main` between `154656b1be82e3104620a05c4bf3430e1db9507d` and `b3bb7f01c1c0dc8a5fa95c071e468f261d9e328c`. The upstream change is “Push to scopes after setting fields to avoid deopt”: one `src/scopes.ts` edit. The [upstream commit](https://github.com/jridgewell/sourcemap-codec/commit/b3bb7f01c1c0dc8a5fa95c071e468f261d9e328c) and [MIT license](https://github.com/jridgewell/sourcemap-codec/blob/b3bb7f01c1c0dc8a5fa95c071e468f261d9e328c/LICENSE) identify the source. This is historical evidence, not a statement about upstream's latest branch.

```sh
cargo build --locked -p ripple-cli
python3 docs/examples/m4-diff/reproduce.py --binary target/debug/ripple --output /tmp/ripple-diff-demo
```

Use a new output directory. The script fetches only the pinned commits into a disposable repository, never checks out files, installs dependencies or runs upstream scripts, then runs the comparison twice and checks exact JSON equality. Ripple itself does not fetch. `--repository /path/to/existing/upstream` reuses local objects without network access. Each subprocess has a 180-second timeout.

The [summary](summary.json) records seven UNCERTAIN symbol classifications, eight impact targets and 849 graph-wide unresolved records at each endpoint. Both revision labels survive. There are two indexed head test files and zero selected tests. Unsupported package entry/config policy and syntax make this report incomplete; it does not demonstrate test sufficiency, reduction or general resolver accuracy. The source-only M3 oracle projection is a different scope and its metrics do not apply to this full-repository run.

[report.json.gz](report.json.gz) contains the complete format-v1 JSON stdout, with its uncompressed SHA-256 recorded in `summary.json`. [Human output](report.txt) and [stderr](stderr.txt) preserve the visible warnings. Inspect the JSON with `python3 -c "import gzip; print(gzip.open('docs/examples/m4-diff/report.json.gz', 'rt').read())"`. The implementation source is commit `3dcb302`; subsequent test/demo/documentation changes do not change the analyzer. The integration ledger records the final verified candidate. Performance is not measured.

Precise supported witness/deletion behaviour is established separately by the 25 classification and eleven revision fixtures and the installed capability smoke, rather than inferred from this incomplete upstream example.
