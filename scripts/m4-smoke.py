#!/usr/bin/env python3
"""Installed v0.2 capability gate: M2 commands plus immutable base/head diff."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
choice = parser.add_mutually_exclusive_group(required=True)
choice.add_argument("--install-root", type=Path)
choice.add_argument("--binary", type=Path)
args = parser.parse_args()
binary = (args.binary or args.install_root / "bin" / ("ripple.exe" if os.name == "nt" else "ripple")).resolve(strict=True)
env = {k: v for k, v in os.environ.items() if not k.upper().startswith("GIT_")}
env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull, GIT_TERMINAL_PROMPT="0")

def run(argv, root, success=True):
    out = subprocess.run([str(a) for a in argv], cwd=root, env=env, capture_output=True, timeout=120)
    assert (out.returncode == 0) == success, (argv, out.returncode, out.stderr.decode("utf-8", "replace"))
    return out

m2 = run([sys.executable, Path(__file__).with_name("m2-smoke.py"), "--binary", binary], Path.cwd())
with tempfile.TemporaryDirectory(prefix="ripple-m4-") as temp:
    root = Path(temp) / "project space Ω"
    root.mkdir()
    template = Path(temp) / "template"
    template.mkdir()
    git = ["git", "-c", "core.hooksPath=", "-c", "core.fsmonitor=false", "-c", "core.autocrlf=false"]
    run([*git, "init", "-q", "--initial-branch=main", f"--template={template}"], root)
    for key, value in [("user.name", "Ripple Fixture"), ("user.email", "fixture@ripple.invalid"), ("commit.gpgsign", "false")]:
        run([*git, "config", "--local", key, value], root)
    def write(name, text):
        (root / name).write_bytes(text.encode("utf-8"))
    def commit():
        run([*git, "add", "--all"], root)
        run([*git, "commit", "-qm", "fixture"], root)
        return run([*git, "rev-parse", "HEAD"], root).stdout.decode().strip()
    def command(*argv):
        return json.loads(run([binary, *argv, "--format", "json"], root).stdout)
    def state():
        return {p.relative_to(root).as_posix(): p.read_bytes() for p in root.rglob("*") if p.is_file()}
    write(".gitignore", ".ripple/\n")
    write("lib.ts", "export function f() { return 1; }\nexport function g() { return 2; }\n")
    write("caller.ts", "import { f } from './lib'; export function caller() { return f(); }\n")
    write("a.test.ts", "import { caller } from './caller'; caller();\n")
    write("b.test.ts", "import { g } from './lib'; g();\n")
    base = commit()
    write("lib.ts", "export function g() { return 3; }\n")
    (root / "a.test.ts").unlink()
    head = commit()
    comparison = f"{base}..{head}"
    command("index", ".")
    write("lib.ts", "invalid staged content")
    run([*git, "add", "lib.ts"], root)
    write("lib.ts", "invalid dirty content")
    write("untracked.ts", "invalid untracked content")
    before = state()
    out = command("diff", comparison)
    assert out["data"]["comparison"]["endpoints"] == {"base": base, "head": head}
    result = out["data"]["result"]
    kinds = {s["identity"]["qualified_name"][0]: s["kind"] for s in result["changes"]["symbols"]}
    assert kinds["f"] == "DELETED" and kinds["g"] == "BODY_MODIFIED"
    assert any(w["revision"] == "base" and w["target"]["identity"]["path"] == "a.test.ts" and len(w["hops"]) == 2 for w in result["evidence"]["symbol_backed"])
    assert any(w["revision"] == "head" and w["target"]["identity"]["path"] == "b.test.ts" for w in result["evidence"]["symbol_backed"])
    assert [n["identity"]["path"] for n in result["tests"]["base_only"]] == ["a.test.ts"]
    assert result["tests"]["total_head_indexed_test_files"] == 1
    limited = command("diff", comparison, "--limit", "0")["data"]["result"]
    for key in ["evidence", "rows", "tests", "changes"]:
        assert limited[key] == result[key]
    assert limited["display_omitted"] > 0
    depth = command("diff", comparison, "--depth", "0")
    assert depth["incomplete_analysis"]
    assert command("diff", f"{head}..{head}")["data"]["result"]["rows"] == []
    human = run([binary, "diff", comparison], root).stdout
    assert b"Base SymbolBacked witness" in human and b"Head SymbolBacked witness" in human
    assert b"Potentially related test files" in human and b"DELETED" in human
    invalid = run([binary, "diff", "main...HEAD"], root, success=False)
    assert not invalid.stdout and b"three-dot" in invalid.stderr
    assert state() == before
    print(json.dumps({"m4_smoke": "passed", "prior_capabilities": json.loads(m2.stdout), "revision_bound_witnesses": True, "deleted_test_retained": True, "worktree_and_indexes_preserved": True}))
