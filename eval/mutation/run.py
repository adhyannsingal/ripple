#!/usr/bin/env python3
"""Controlled owned mutations; never execute fetched project scripts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
FILES = {
 'lib.ts': 'export function f(): number { return 1; }\nexport function g(): number { return 10; }\n',
 'caller.ts': "import {f} from './lib'; export function caller(): number { return f(); }\n",
 'alpha.test.ts': "import {caller} from './caller'; if(caller()!==1) throw new Error('alpha');\n",
 'beta.test.ts': "import {g} from './lib'; if(g()!==10) throw new Error('beta');\n",
 'dyn.ts':'export function dyn(): number { return 1; }\n',
 'dynamic.test.ts':"import('./dyn').then(({dyn})=>{if(dyn()!==1) throw new Error('dynamic');});\n",
 'effect.ts':'export function value(): number { return 1; }\n(globalThis as any).rippleValue=value();\n',
 'side.test.ts':"import './effect'; if((globalThis as any).rippleValue!==1) throw new Error('side');\n",
 'other.test.ts':"if(2+2!==4) throw new Error('unrelated');\n",
}
CASES = [
 ('direct','lib.ts','return 1;','return 2;','supported_direct', 'alpha.test.ts'),
 ('other-export','lib.ts','return 10;','return 11;','supported_direct','beta.test.ts'),
 ('caller','caller.ts','return f();','return f()+1;','supported_direct','alpha.test.ts'),
 ('dynamic','dyn.ts','return 1;','return 2;','unsupported_dynamic_import','dynamic.test.ts'),
 ('side-effect','effect.ts','return 1;','return 2;','unsupported_global_state','side.test.ts'),
 ('survivor','lib.ts','return 1;','return 1+0;','survivor',None),
 ('invalid','lib.ts','return 1;',"return 'invalid';",'compile_invalid',None),
 ('timeout','lib.ts','return 1;','while(true) {}','timeout',None),
]


def execute(argv, cwd, timeout=60, env=None):
    result = subprocess.run([str(a) for a in argv],cwd=cwd,env=env,capture_output=True,timeout=timeout)
    return result


def checked(argv,cwd,env=None):
    out=execute(argv,cwd,env=env)
    if out.returncode: raise RuntimeError(out.stderr.decode('utf8','replace')[:8192])
    return out


def fraction(n,d):
    return {'numerator':n,'denominator':d,'fraction':n/d if d else None}


def import_selection(imports, changed, tests):
    reached=set(changed)
    while True:
        following={f for f,targets in imports.items() if reached.intersection(targets)} | reached
        if following==reached: return sorted(reached.intersection(tests))
        reached=following


def metrics(selected,failures,total):
    caught=sorted(set(selected).intersection(failures))
    return {'selected':sorted(selected),'selected_failing':caught,'missed':sorted(set(failures)-set(selected)),
            'recall':fraction(len(caught),len(failures)), 'selection_ratio':fraction(len(selected),total)}


def suite(node, root, tests, timeout):
    failures=[];timeouts=[];results=[]
    for test in tests:
        try:
            out=execute([node,root/'build'/test.replace('.ts','.js')],root,timeout=timeout)
            if out.returncode: failures.append(test)
            results.append({'file':test,'exit_code':out.returncode,'stderr':out.stderr.decode('utf8','replace')[:8192]})
        except subprocess.TimeoutExpired:
            timeouts.append(test);results.append({'file':test,'timeout':True})
    return failures,timeouts,results


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--node',type=Path,required=True)
    parser.add_argument('--typescript',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--timeout',type=float,default=2.0)
    args=parser.parse_args()
    if args.output.exists() or not .1<=args.timeout<=10: raise ValueError('new output and .1..10s timeout required')
    binary=args.binary.resolve(strict=True);node=args.node.resolve(strict=True);ts=args.typescript.resolve(strict=True)
    env={k:v for k,v in os.environ.items() if not k.upper().startswith('GIT_')}
    env.update(GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL=os.devnull,GIT_TERMINAL_PROMPT='0',GIT_AUTHOR_DATE='2000-01-01T00:00:00Z',GIT_COMMITTER_DATE='2000-01-01T00:00:00Z')
    rows=[]
    for name,path,old,new,category,expected_failure in CASES:
        with tempfile.TemporaryDirectory(prefix='ripple-mutation-') as directory:
            root=Path(directory);template=root/'template';template.mkdir()
            git=['git','-c','core.hooksPath=','-c','core.fsmonitor=false','-c','core.autocrlf=false','-c','commit.gpgsign=false']
            def g(*argv): return checked([*git,*argv],root,env)
            g('init','-q','--initial-branch=main',f'--template={template}')
            g('config','user.name','Ripple Fixture');g('config','user.email','fixture@ripple.invalid')
            for file,text in FILES.items(): (root/file).write_bytes(text.encode())
            (root/'.gitignore').write_text('build/\ntemplate/\n.ripple/\n')
            inventory=json.loads(checked([node,ROOT/'eval/mutation/inventory.cjs',root,ts],root).stdout)
            tests=inventory['tests']
            compiler=[node,ts/'lib/tsc.js','--strict','--target','es2022','--module','commonjs','--noEmitOnError','--outDir','build',*sorted(FILES)]
            checked(compiler,root)
            before=suite(node,root,tests,args.timeout)
            if before[0] or before[1]: raise RuntimeError('baseline suite is not green')
            g('add','--all');g('commit','-qm','baseline');base=g('rev-parse','HEAD').stdout.decode().strip()
            if FILES[path].count(old)!=1: raise ValueError('ambiguous mutation target')
            (root/path).write_bytes(FILES[path].replace(old,new).encode())
            g('add','--all');g('commit','-qm',name);head=g('rev-parse','HEAD').stdout.decode().strip()
            compiled=execute(compiler,root)
            row={'id':name,'category':category,'changed_file':path,'base':base,'head':head,'baseline_passed':True,'compile_exit':compiled.returncode,'compiler_output':(compiled.stdout+compiled.stderr).decode('utf8','replace')[:8192]}
            if compiled.returncode:
                row.update(status='invalid',failing_files=[],policies=None)
            else:
                failures,timeouts,runtime=suite(node,root,tests,args.timeout)
                row.update(status='timeout' if timeouts else 'killed' if failures else 'survived',failing_files=failures,timeouts=timeouts,runtime=runtime)
                raw=checked([binary,'test',f'{base}..{head}','--root',root,'--format','json','--limit','0'],root,env)
                report=json.loads(raw.stdout);data=report['data'];selection=data['selection']['active_head']
                policies={'symbol_backed':selection['symbol_backed'],'combined':selection['combined'],
                          'file_import':import_selection(inventory['imports'],[path],tests),'all_tests':tests}
                row['policies']={key:metrics(selected,failures,len(tests)) for key,selected in policies.items()}
                row.update(incomplete_analysis=report['incomplete_analysis'],head_test_files=data['counts']['total_head_test_files'],ripple_report_sha256=hashlib.sha256(raw.stdout).hexdigest(),selection_completeness=data['selection']['completeness'])
                if row['head_test_files']!=len(tests): raise ValueError('fixture/runtime denominator mismatch')
                if expected_failure and failures!=[expected_failure]: raise ValueError('mutation did not fail its independently expected file')
                if category=='supported_direct' and any(expected_failure not in policies[key] for key in ['symbol_backed','combined']): raise RuntimeError('supported direct selection miss; retain fixture and repair before release')
                for policy,values in row['policies'].items():
                    values['miss_categories']=[category for _ in values['missed']]
            if name=='invalid' and row['status']!='invalid': raise ValueError('invalid self-check failed')
            if name=='timeout' and row['status']!='timeout': raise ValueError('timeout self-check failed')
            if name=='survivor' and row['status']!='survived': raise ValueError('survivor self-check failed')
            rows.append(row);print(name,row['status'],flush=True)
    eligible=[r for r in rows if r['status']=='killed']
    aggregates={}
    for policy in ['symbol_backed','combined','file_import','all_tests']:
        aggregates[policy]={'fault_catching_recall':fraction(sum(len(r['policies'][policy]['selected_failing']) for r in eligible),sum(len(r['failing_files']) for r in eligible)),
                            'selection_ratio_killed_micro':fraction(sum(len(r['policies'][policy]['selected']) for r in eligible),sum(r['head_test_files'] for r in eligible))}
    result={'format_version':1,'analyzer_source_commit':checked(['git','rev-parse','HEAD'],ROOT,env).stdout.decode().strip(),
            'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'node_version':checked([node,'--version'],ROOT).stdout.decode().strip(),'typescript_version':inventory['typescript'],
            'fixture_sha256':hashlib.sha256(json.dumps(FILES,sort_keys=True).encode()).hexdigest(),'baseline_tests':tests,'mutations':rows,'aggregate':aggregates,
            'policy':'Micro recall across compiling, completed, killed mutations only; survivors have undefined recall; invalid/timeout rows excluded from aggregate. Selection ratio uses same killed cases and supported head file denominator. No runtime safety guarantee.',
            'excluded':{s:[r['id'] for r in rows if r['status']==s] for s in ['invalid','timeout','survived']},'command':' '.join(__import__('sys').argv)}
    args.output.mkdir(parents=True);(args.output/'mutations.json').write_text(json.dumps(result,indent=2)+'\n')


if __name__=='__main__': main()
