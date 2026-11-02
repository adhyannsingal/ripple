#!/usr/bin/env python3
"""Clean-install M2 capability smoke; disposable Git root, no project scripts."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("--install-root", type=Path)
parser.add_argument("--binary", type=Path)
args = parser.parse_args()
binary = args.binary or (args.install_root / "bin" / ("ripple.exe" if os.name == "nt" else "ripple"))
binary = binary.resolve(strict=True)
env = {k: v for k, v in os.environ.items() if not k.upper().startswith("GIT_")}
env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull, GIT_TERMINAL_PROMPT="0")

def run(argv, root, success=True):
    result = subprocess.run([str(a) for a in argv], cwd=root, env=env, capture_output=True, timeout=60)
    assert (result.returncode == 0) == success, (argv, result.returncode, result.stderr.decode("utf-8", "replace"))
    return result

with tempfile.TemporaryDirectory(prefix="ripple-m2-") as temp:
    base = Path(temp)
    root = base / "project space Ω"
    shutil.copytree(Path(__file__).resolve().parents[1] / "fixtures/cold-index", root)
    template = base / "empty-template"
    template.mkdir()
    hooks = base / "no-hooks"
    hooks.mkdir()
    git = ["git", "-c", f"core.hooksPath={hooks}", "-c", "core.fsmonitor=false"]
    run([*git, "init", "--quiet", "--initial-branch=main", f"--template={template}"], root)
    for key, value in [("user.name", "Ripple Fixture"), ("user.email", "fixture@ripple.invalid"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")]:
        run([*git, "config", "--local", key, value], root)
    run([*git, "add", "."], root)
    run([*git, "commit", "--quiet", "-m", "fixture"], root)
    original_index = (root / ".git/index").read_bytes()
    def command(*argv):
        return json.loads(run([binary, *argv, "--format", "json"], root).stdout)
    assert b"ripple" in run([binary, "--version"], root).stdout
    assert b"status" in run([binary, "--help"], root).stdout
    assert command("inspect", "lib.ts")["data"]["analysis_stage"] == "syntax"
    indexed = command("index", ".")
    assert indexed["data"]["counts"]["files"] == 4
    assert indexed["data"]["counts"]["symbols"] == 3
    assert not indexed["incomplete_analysis"]
    status = command("status")
    assert status["data"]["freshness"] == "verified"
    assert not status["data"]["metadata"]["origin"]["dirty"]
    impact = command("impact", "a")["data"]["result"]
    paths = lambda nodes: {n["identity"]["path"] for n in nodes}
    assert paths(impact["tests"]["symbol_backed"]) == {"a.test.ts"}
    assert paths(impact["tests"]["fallback_only"]) == {"b.test.ts"}
    assert paths(impact["tests"]["combined"]) == {"a.test.ts", "b.test.ts"}
    assert any(w["target"]["identity"]["path"] == "a.test.ts" and len(w["hops"]) == 2 for w in impact["impact"]["evidence"]["symbol_backed"])
    assert command("impact", "a", "--limit", "0")["data"]["result"]["tests"] == impact["tests"]
    assert command("impact", "a", "--depth", "1")["incomplete_analysis"]
    human = run([binary, "impact", "lib.ts::a"], root).stdout
    assert b"SymbolBacked witness" in human and b"ModuleFallback witness" in human
    assert b"Potentially related tests" in human
    source = root / "lib.ts"
    source.write_text(source.read_text(encoding="utf-8").replace("return 1", "return 7"), encoding="utf-8")
    stale = run([binary, "status", "--format", "json"], root, success=False)
    assert not stale.stdout and b"stale_index" in stale.stderr
    command("index", ".")
    assert command("status")["data"]["metadata"]["origin"]["dirty"]
    assert (root / ".git/index").read_bytes() == original_index
    print(json.dumps({"m2_smoke": "passed", "source_files": 4, "symbol_backed_tests": 1, "fallback_only_tests": 1, "combined_tests": 2, "git_index_preserved": True}))
