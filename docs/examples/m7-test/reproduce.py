#!/usr/bin/env python3
"""Pinned real-repository test-impact examples; no checkout or upstream script execution."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

REPOSITORY="https://github.com/jridgewell/sourcemap-codec"
PAIRS=[('source-only','154656b1be82e3104620a05c4bf3430e1db9507d','b3bb7f01c1c0dc8a5fa95c071e468f261d9e328c'),('source-and-test','ed80f4c897040892c8802fdf70008be976db2a79','d06c9ebeb50ba02077239d79549618f2c888bb78')]
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

with tempfile.TemporaryDirectory(prefix="ripple-upstream-test-") as temp:
    root=args.repository.resolve(strict=True) if args.repository else Path(temp)/"upstream"
    if not args.repository:
        root.mkdir();template=Path(temp)/"template";template.mkdir()
        git=["git","-c","core.hooksPath=","-c","core.fsmonitor=false","-c","credential.helper="]
        run([*git,"init","-q","--initial-branch=demo",f"--template={template}"],root)
        pins=sorted({sha for _,base,head in PAIRS for sha in [base,head]})
        run([*git,"fetch","--quiet","--no-tags","--depth=1",REPOSITORY,*pins],root)
    reports=[]
    for name,base,head in PAIRS:
        raw=run([binary,"test",f"{base}..{head}","--root",root,"--format","json","--limit","0"],root)
        repeated=run([binary,"test",f"{base}..{head}","--root",root,"--format","json","--limit","0"],root)
        assert raw.stdout==repeated.stdout
        report=json.loads(raw.stdout);data=report['data']
        assert data['comparison']['endpoints']=={'base':base,'head':head}
        assert data['sufficiency_guaranteed'] is False and report['incomplete_analysis']
        assert data['counts']['total_head_test_files']==2
        for key in ['symbol_backed','combined','module_fallback']:
            ratio=data['selection_ratios'][key]
            assert ratio['numerator']==len(data['selection']['active_head'][key])
            assert ratio['denominator']==2 and ratio['fraction']==ratio['numerator']/2
        if name=='source-only': assert data['counts']['combined_selected']==0
        else: assert 'test/scopes.test.ts' in data['selection']['active_head']['combined']
        human=run([binary,"test",f"{base}..{head}","--root",root],root)
        reports.append((name,raw,human,dict(repository=REPOSITORY,base=base,head=head,scope='Complete committed repository under supported scanner/config policy; no rewriting or source projection',counts=data['counts'],selection_ratios=data['selection_ratios'],incomplete_analysis=True,repeatable_json=True,report_sha256=hashlib.sha256(raw.stdout).hexdigest(),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),analyzer_versions=data['versions'],endpoint_context={side:{'unresolved':len(data['analysis'][side]['query']['impact']['unresolved']),'snapshot_diagnostics':len(data['analysis'][side]['snapshot_diagnostics'])} for side in ['base','head']},limits='Incomplete upstream package/config/syntax. Source-only zero selection is not safe omission; changed-test inclusion is not dependency accuracy. No tests executed.')))
    args.output.mkdir(parents=True,exist_ok=False)
    for name,raw,human,summary in reports:
        directory=args.output/name;directory.mkdir()
        (directory/'report.json.gz').write_bytes(gzip.compress(raw.stdout,mtime=0))
        (directory/'report.txt').write_bytes(human.stdout)
        (directory/'stderr.txt').write_bytes(raw.stderr)
        (directory/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps({'demo':'passed','comparisons':len(reports),'repeatable_json':True,'incomplete_analysis':True}))
