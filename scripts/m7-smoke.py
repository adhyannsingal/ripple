#!/usr/bin/env python3
"""Installed test-impact capability gate, including all prior commands."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
choice = parser.add_mutually_exclusive_group(required=True)
choice.add_argument('--binary', type=Path)
choice.add_argument('--install-root', type=Path)
args = parser.parse_args()
binary = (args.binary or args.install_root / 'bin' / ('ripple.exe' if os.name == 'nt' else 'ripple')).resolve(strict=True)
env = {key: value for key, value in os.environ.items() if not key.upper().startswith('GIT_')}
env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull, GIT_TERMINAL_PROMPT='0')


def run(argv, root):
    return subprocess.run([str(a) for a in argv], cwd=root, env=env, capture_output=True, timeout=120, check=True)


prior = json.loads(run([sys.executable, Path(__file__).with_name('m6-smoke.py'), '--binary', binary], Path.cwd()).stdout)
with tempfile.TemporaryDirectory(prefix='ripple-m7-') as directory:
    root=Path(directory)/'project Ω';root.mkdir()
    template=Path(directory)/'template';template.mkdir()
    git=['git','-c','core.hooksPath=','-c','core.fsmonitor=false','-c','core.autocrlf=false']
    run([*git,'init','-q','--initial-branch=main',f'--template={template}'],root)
    for key,value in [('user.name','Ripple Fixture'),('user.email','fixture@ripple.invalid'),('commit.gpgsign','false')]: run([*git,'config','--local',key,value],root)
    def write(path,text):
        target=root/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(text.encode())
    def commit():
        run([*git,'add','--all'],root);run([*git,'commit','-qm','fixture'],root)
        return run([*git,'rev-parse','HEAD'],root).stdout.decode().strip()
    write('.gitignore','.ripple/\n')
    write('lib.ts','export function f(){return 1;}\nexport function g(){return 1;}\n')
    write('a.test.ts',"import {f} from './lib'; f();\n")
    write('b.test.ts',"import {g} from './lib'; g();\n")
    base=commit()
    write('lib.ts','export function f(){return 2;}\nexport function g(){return 1;}\n');head=commit()
    write('lib.ts','staged');run([*git,'add','lib.ts'],root);write('lib.ts','dirty')
    write('untracked.ts','export function untracked(){}');write('.ripple/owned','keep')
    original_index=(root/'.git/index').read_bytes()
    pair=f'{base}..{head}'
    result=run([binary,'test',pair,'--format','json','--limit','0'],root)
    report=json.loads(result.stdout);data=report['data']
    assert report['command']=='test' and not data['sufficiency_guaranteed']
    assert data['selection']['active_head']['symbol_backed']==['a.test.ts']
    assert data['selection']['active_head']['fallback_only']==['b.test.ts']
    assert data['selection']['active_head']['combined']==['a.test.ts','b.test.ts']
    assert data['selection_ratios']['symbol_backed']=={'numerator':1,'denominator':2,'fraction':0.5}
    assert data['selection_ratios']['combined']=={'numerator':2,'denominator':2,'fraction':1.0}
    assert run([binary,'test',pair,'--format','json','--limit','0'],root).stdout==result.stdout
    human=run([binary,'test',pair],root).stdout
    assert b'[fallback-only]' in human and b'not a sufficiency guarantee' in human
    assert original_index==(root/'.git/index').read_bytes() and (root/'lib.ts').read_bytes()==b'dirty'
    assert (root/'.ripple/owned').read_bytes()==b'keep' and not (root/'.ripple/index.db').exists()
print(json.dumps({'m7_smoke':'passed','prior_capabilities':prior,'symbol_backed':1,'fallback_only':1,'combined':2,'head_tests':2,'git_index_and_user_state_preserved':True}))
