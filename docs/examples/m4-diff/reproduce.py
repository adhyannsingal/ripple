#!/usr/bin/env python3
"""Analyze a pinned upstream branch pair without checkout or upstream script execution."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

REPOSITORY = "https://github.com/jridgewell/sourcemap-codec"
BASE = "154656b1be82e3104620a05c4bf3430e1db9507d"
HEAD = "b3bb7f01c1c0dc8a5fa95c071e468f261d9e328c"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binary", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True, help="New output directory")
parser.add_argument("--repository", type=Path, help="Optional existing local upstream Git root containing both pinned objects")
args = parser.parse_args()
binary = args.binary.resolve(strict=True)
env = {k: v for k, v in os.environ.items() if not k.upper().startswith("GIT_")}
env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull, GIT_TERMINAL_PROMPT="0")

def run(argv, root):
    out = subprocess.run([str(a) for a in argv], cwd=root, env=env, capture_output=True, timeout=180)
    if out.returncode:
        raise RuntimeError(out.stderr.decode("utf-8", "replace"))
    return out

with tempfile.TemporaryDirectory(prefix="ripple-upstream-diff-") as temp:
    root = args.repository.resolve(strict=True) if args.repository else Path(temp) / "upstream"
    if not args.repository:
        root.mkdir()
        template = Path(temp) / "empty-template"
        template.mkdir()
        git = ["git", "-c", "core.hooksPath=", "-c", "core.fsmonitor=false", "-c", "credential.helper="]
        run([*git, "init", "-q", "--initial-branch=demo", f"--template={template}"], root)
        # Explicit network acquisition for this demo only. Ripple itself never fetches.
        run([*git, "fetch", "--quiet", "--no-tags", "--depth=1", REPOSITORY, BASE, HEAD], root)
    raw = run([binary, "diff", f"{BASE}..{HEAD}", "--root", root, "--format", "json"], root)
    repeated = run([binary, "diff", f"{BASE}..{HEAD}", "--root", root, "--format", "json"], root)
    assert raw.stdout == repeated.stdout
    data = json.loads(raw.stdout)
    result = data["data"]["result"]
    assert data["data"]["comparison"]["endpoints"] == {"base": BASE, "head": HEAD}
    assert data["data"]["counts"] == {"changed_files": 1, "changed_symbols": 7, "classified_symbols": 7}
    assert {s["identity"]["path"] for s in result["changes"]["symbols"]} == {"src/scopes.ts"}
    assert {s["kind"] for s in result["changes"]["symbols"]} == {"UNCERTAIN"}
    assert data["incomplete_analysis"]
    assert len(result["rows"]) == 8
    assert result["tests"]["total_head_indexed_test_files"] == 2
    assert result["tests"]["combined"] == []
    assert {w["revision"] for track in result["evidence"].values() for w in track} == {"base", "head"}
    human = run([binary, "diff", f"{BASE}..{HEAD}", "--root", root], root)
    summary = {
        "format_version": 1, "repository": REPOSITORY, "upstream_branch": "main",
        "base": BASE, "head": HEAD,
        "upstream_change": "Push to scopes after setting fields to avoid deopt",
        "scope": "Complete committed repository inputs under Ripple scanner/config policy; no projection or config rewriting",
        "counts": data["data"]["counts"], "incomplete_analysis": data["incomplete_analysis"],
        "rows": len(result["rows"]), "classifications": [{"identity":s["identity"],"kind":s["kind"],"reasons":s["reasons"]} for s in result["changes"]["symbols"]],
        "evidence_counts": {track:len(items) for track,items in result["evidence"].items()},
        "test_counts": {key:len(value) if isinstance(value,list) else value for key,value in result["tests"].items()},
        "endpoint_context": {side:{"unresolved":len(result[side]["query"]["impact"]["unresolved"]),"snapshot_diagnostics":len(result[side]["snapshot_diagnostics"])} for side in ["base","head"]},
        "repeatable_json": True, "report_sha256": hashlib.sha256(raw.stdout).hexdigest(),
        "analyzer_versions": data["data"]["versions"],
        "limits": "Unsupported package entries/config options and syntax remain explicit; empty selection is not a test sufficiency or accuracy claim. Performance not measured."
    }
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "report.json.gz").write_bytes(gzip.compress(raw.stdout, mtime=0))
    (args.output / "report.txt").write_bytes(human.stdout)
    (args.output / "stderr.txt").write_bytes(raw.stderr)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2)+"\n")
    print(json.dumps({"demo":"passed", "report_sha256":summary["report_sha256"], "incomplete_analysis":True}))
