#!/usr/bin/env python3
"""Compile and independently test an owned disposable TypeScript Git project."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

FILES = {
    '.gitignore': '.ripple/\nbuild/\n',
    'tsconfig.json': '{"compilerOptions":{"strict":true,"target":"es2022","module":"commonjs"}}\n',
    'pricing.ts': 'export function total(price: number, quantity: number): number { return price * quantity; }\nexport function shipping(): number { return 5; }\n',
    'checkout.ts': "import { total } from './pricing';\nexport function checkout(): number { return total(10, 2); }\n",
    'unrelated.ts': 'export function unrelated(): number { return 7; }\n',
    'checkout.test.ts': "import { checkout } from './checkout';\nif (checkout() !== 20) { throw new Error('checkout regression'); }\n",
    'shipping.test.ts': "import { shipping } from './pricing';\nif (shipping() !== 5) { throw new Error('shipping regression'); }\n",
    'unrelated.test.ts': "import { unrelated } from './unrelated';\nif (unrelated() !== 7) { throw new Error('unrelated regression'); }\n",
}
TESTS = ['checkout.test.ts', 'shipping.test.ts', 'unrelated.test.ts']
ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--typescript', type=Path, required=True)
    parser.add_argument('--helper', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError('output must be new')
    binary, node, typescript, helper = [p.resolve(strict=True) for p in (args.binary,args.node,args.typescript,args.helper)]
    env = {k:v for k,v in os.environ.items() if not k.upper().startswith('GIT_')}
    env.update(GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL=os.devnull,GIT_TERMINAL_PROMPT='0',GIT_AUTHOR_DATE='2000-01-01T00:00:00Z',GIT_COMMITTER_DATE='2000-01-01T00:00:00Z')
    commands = []
    def run(argv, cwd, check=True):
        result = subprocess.run([str(a) for a in argv],cwd=cwd,env=env,capture_output=True,timeout=120)
        if check and result.returncode:
            raise RuntimeError(str(argv)+': '+(result.stdout+result.stderr).decode('utf8','replace'))
        return result
    with tempfile.TemporaryDirectory(prefix='ripple-small-repo-') as directory:
        temp=Path(directory);root=temp/'checkout project';root.mkdir();template=temp/'empty-template';template.mkdir()
        git=['git','-c','core.hooksPath=','-c','core.fsmonitor=false','-c','core.autocrlf=false','-c','commit.gpgsign=false']
        def g(*argv):return run([*git,*argv],root).stdout
        g('init','-q','--initial-branch=main',f'--template={template}')
        g('config','user.name','Ripple Fixture');g('config','user.email','fixture@ripple.invalid')
        for path,text in FILES.items():(root/path).write_bytes(text.encode())
        def commit():
            g('add','--all');g('commit','-qm','owned fixture');return g('rev-parse','HEAD').decode().strip()
        def cli(*argv,check=True):
            result=run([binary,*argv,'--format','json'],root,check=check);commands.append(argv[0])
            return json.loads(result.stdout) if check else result
        def suite():
            run([node,typescript/'lib/tsc.js','--ignoreConfig','--strict','--target','es2022','--module','commonjs','--noEmitOnError','--outDir','build',*sorted(p for p in FILES if p.endswith('.ts'))],root)
            rows=[]
            for test in TESTS:
                result=run([node,root/'build'/test.replace('.ts','.js')],root,check=False)
                rows.append({'test':test,'exit':result.returncode,'stderr':result.stderr.decode('utf8','replace')[:4096]})
            return rows
        baseline=suite();assert all(r['exit']==0 for r in baseline),baseline
        base=commit();cli('index','.');cli('inspect','pricing.ts');assert cli('status')['data']['freshness']=='verified'
        impact=cli('impact','pricing.ts::total')
        original=FILES['pricing.ts'];changed=original.replace('price * quantity','price + quantity')
        (root/'pricing.ts').write_bytes(changed.encode())
        stale=cli('status',check=False);assert stale.returncode!=0 and not stale.stdout
        head=commit();updated=cli('index','.');assert cli('status')['data']['freshness']=='verified'
        pair=f'{base}..{head}';diff=cli('diff',pair);selection=cli('test',pair,'--limit','0')
        active=selection['data']['selection']['active_head']
        assert active['symbol_backed']==['checkout.test.ts'],active
        assert active['fallback_only']==['shipping.test.ts'],active
        assert active['combined']==['checkout.test.ts','shipping.test.ts'],active
        assert not selection['data']['sufficiency_guaranteed']
        assert cli('test',pair,'--limit','0')==selection
        changed_suite=suite();failures=[r['test']for r in changed_suite if r['exit']]
        assert failures==['checkout.test.ts'],changed_suite
        assert set(failures)<=set(active['symbol_backed'])
        # Compare persisted incremental analysis with an independent cold pipeline.
        cold=temp/'cold';cold.mkdir()
        for path in FILES:(cold/path).write_bytes((root/path).read_bytes())
        canonical=json.loads(run([helper,root,cold],root).stdout);assert canonical['canonical_equal']
        cli('index','.','--full');assert cli('status')['data']['freshness']=='verified'
        # Historical queries must preserve staged/unstaged files and private index bytes.
        (root/'notes.txt').write_text('staged\n');g('add','notes.txt');(root/'notes.txt').write_text('unstaged\n')
        before_index=(root/'.git/index').read_bytes();before_status=g('status','--porcelain=v1','-z');before_db=(root/'.ripple/index.db').read_bytes()
        assert cli('test',pair,'--limit','0')==selection
        assert cli('diff',pair)==diff
        assert before_index==(root/'.git/index').read_bytes()
        assert before_status==g('status','--porcelain=v1','-z')
        assert before_db==(root/'.ripple/index.db').read_bytes()
        assert (root/'notes.txt').read_text()=='unstaged\n'
        evidence={'status':'passed','scope':'owned compiling checkout calculation fixture; finite direct-call coverage only','source_commit':subprocess.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'version':run([binary,'--version'],root).stdout.decode().strip(),'node':run([node,'--version'],root).stdout.decode().strip(),'typescript':run([node,typescript/'lib/tsc.js','--version'],root).stdout.decode().strip(),'commands':sorted(set(commands)),'base':base,'head':head,'files':FILES,'changed_pricing':changed,'baseline_suite':baseline,'changed_suite':changed_suite,'symbol_backed':active['symbol_backed'],'fallback_only':active['fallback_only'],'combined':active['combined'],'selection_ratios':selection['data']['selection_ratios'],'incomplete_analysis':selection['incomplete_analysis'],'stale_query_rejected':True,'canonical':canonical,'git_index_and_worktree_preserved':True,'historical_query_database_unchanged':True,'no_tests_executed_by_ripple':True,'tests_executed_independently_by_harness':True,'impact':impact,'diff':diff,'selection':selection,'incremental_update':updated['data']['update']}
    args.output.mkdir(parents=True)
    (args.output/'report.json').write_text(json.dumps(evidence,indent=2)+'\n')
    print(json.dumps({k:evidence[k]for k in ['status','commands','symbol_backed','fallback_only','combined','stale_query_rejected','git_index_and_worktree_preserved']}))


if __name__=='__main__':main()
