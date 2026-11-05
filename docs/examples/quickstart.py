#!/usr/bin/env python3
"""Disposable all-command demo; uses only Git, Python and the installed Ripple binary."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);args=p.parse_args();binary=args.binary.resolve(strict=True)
env={k:v for k,v in os.environ.items() if not k.upper().startswith('GIT_')};env.update(GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL=os.devnull,GIT_TERMINAL_PROMPT='0',GIT_AUTHOR_DATE='2000-01-01T00:00:00Z',GIT_COMMITTER_DATE='2000-01-01T00:00:00Z')
with tempfile.TemporaryDirectory(prefix='ripple-quickstart-') as directory:
    root=Path(directory)/'project';root.mkdir();template=Path(directory)/'template';template.mkdir()
    def run(argv):return subprocess.run([str(a)for a in argv],cwd=root,env=env,capture_output=True,timeout=120,check=True)
    git=['git','-c','core.hooksPath=','-c','core.fsmonitor=false','-c','core.autocrlf=false','-c','commit.gpgsign=false']
    run([*git,'init','-q','--initial-branch=main',f'--template={template}'])
    for key,value in [('user.name','Ripple Fixture'),('user.email','fixture@ripple.invalid')]:run([*git,'config',key,value])
    def write(path,text):(root/path).write_bytes(text.encode())
    def commit():
        run([*git,'add','--all']);run([*git,'commit','-qm','fixture']);return run([*git,'rev-parse','HEAD']).stdout.decode().strip()
    write('.gitignore','.ripple/\n');write('lib.ts','export function f(){return 1;}\nexport function g(){return 1;}\n')
    write('a.test.ts',"import {f} from './lib'; f();\n");write('b.test.ts',"import {g} from './lib'; g();\n")
    base=commit();commands=[]
    def command(*argv):
        output=run([binary,*argv,'--format','json']);commands.append(argv[0]);return json.loads(output.stdout)
    command('index','.');command('inspect','lib.ts');assert command('status')['data']['freshness']=='verified'
    command('impact','lib.ts::f')
    write('lib.ts','export function f(){return 2;}\nexport function g(){return 1;}\n');head=commit()
    command('index','.');command('diff',f'{base}..{head}')
    selected=command('test',f'{base}..{head}')['data'];assert selected['selection']['active_head']['symbol_backed']==['a.test.ts'];assert selected['selection']['active_head']['fallback_only']==['b.test.ts']
    command('index','.','--full')
print(json.dumps({'demo':'passed','commands':sorted(set(commands)),'symbol_backed':['a.test.ts'],'fallback_only':['b.test.ts'],'combined_ratio':selected['selection_ratios']['combined'],'no_tests_executed_by_ripple':True}))
