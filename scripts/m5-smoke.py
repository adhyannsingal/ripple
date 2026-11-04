#!/usr/bin/env python3
"""Installed incremental capability gate, including every prior implemented command."""
import argparse
from contextlib import closing
import json
import os
from pathlib import Path
import sqlite3
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

prior = run([sys.executable, Path(__file__).with_name("m4-smoke.py"), "--binary", binary], Path.cwd())
with tempfile.TemporaryDirectory(prefix="ripple-m5-") as temp:
    root = Path(temp) / "project space Ω"
    root.mkdir()
    template = Path(temp) / "template"
    template.mkdir()
    git = ["git", "-c", "core.hooksPath=", "-c", "core.fsmonitor=false", "-c", "core.autocrlf=false"]
    run([*git, "init", "-q", "--initial-branch=main", f"--template={template}"], root)
    for key, value in [("user.name", "Ripple Fixture"), ("user.email", "fixture@ripple.invalid"), ("commit.gpgsign", "false")]:
        run([*git, "config", "--local", key, value], root)
    def write(path, text):
        (root / path).write_bytes(text.encode("utf-8"))
    def command(*argv):
        return json.loads(run([binary, *argv, "--format", "json"], root).stdout)
    def generation():
        with closing(sqlite3.connect((root / ".ripple/index.db").as_uri() + "?mode=ro", uri=True)) as db:
            return db.execute("SELECT generation_id FROM current_generation WHERE singleton=1").fetchone()[0]
    write(".gitignore", ".ripple/\n")
    write("lib.ts", "export function f() { return 1; }\n")
    write("caller.ts", "import { f } from './lib'; export function caller() { return f(); }\n")
    write("main.test.ts", "import { caller } from './caller'; caller();\n")
    run([*git, "add", "--all"], root)
    run([*git, "commit", "-qm", "fixture"], root)
    original_index = (root / ".git/index").read_bytes()
    first = command("index")
    assert first["data"]["update"]["rebuild_reason"] == "empty_index"
    assert first["data"]["update"]["parsed_files"] == 3
    warm = command("index")["data"]["update"]
    assert warm["parsed_files"] == 0 and warm["reused_files"] == 3
    assert warm["module_files_resolved"] == 0 and warm["reference_files_resolved"] == 0
    write("lib.ts", "export function f() { return 2; }\n")
    stale = run([binary, "status", "--format", "json"], root, success=False)
    assert not stale.stdout and b"stale_index" in stale.stderr
    body = command("index")["data"]["update"]
    assert body["parsed_files"] == 1 and body["reused_files"] == 2
    assert body["module_files_resolved"] == 0 and body["reference_files_resolved"] == 1
    assert body["module_cache_reused"]
    assert command("status")["data"]["freshness"] == "verified"
    write("tsconfig.json", '{"compilerOptions":{"strict":true}}')
    global_update = command("index")["data"]["update"]
    assert global_update["rebuild_reason"] == "global_inputs" and global_update["parsed_files"] == 3
    forced = command("index", "--full")["data"]["update"]
    assert forced["rebuild_reason"] == "explicit_full" and forced["parsed_files"] == 3
    before = generation()
    original_source = (root / "lib.ts").read_bytes()
    (root / "lib.ts").write_bytes(b"\xff")
    failure = run([binary, "index", "--format", "json"], root, success=False)
    assert not failure.stdout and generation() == before
    (root / "lib.ts").write_bytes(original_source)
    assert command("status")["data"]["metadata"]["generation"] == before
    human = run([binary, "index"], root).stdout
    assert b"0 files parsed, 3 parse facts reused" in human
    assert b"Module analysis:" in human and b"Reference analysis:" in human
    assert (root / ".git/index").read_bytes() == original_index
    summary = {"m5_smoke": "passed", "prior_capabilities": json.loads(prior.stdout), "no_op_parsed": 0, "body_edit_parsed": 1, "body_module_files_resolved": 0, "body_reference_files_resolved": 1, "failed_generation_preserved": True, "git_index_preserved": True}
print(json.dumps(summary))
