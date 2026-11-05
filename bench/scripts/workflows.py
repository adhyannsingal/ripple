#!/usr/bin/env python3
"""Actual verified CLI workflows plus library stage/resource measurements."""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import time
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('incremental',ROOT/'bench/scripts/incremental.py');inc=importlib.util.module_from_spec(spec);spec.loader.exec_module(inc)

def run(argv,cwd,env):
    start=time.perf_counter();out=subprocess.run([str(a) for a in argv],cwd=cwd,env=env,capture_output=True,timeout=120,check=True)
    return out,(time.perf_counter()-start)*1000

def stat(values,p):return sorted(values)[math.ceil(len(values)*p)-1]
def summary(rows):return {'count':len(rows),'p50_ms':stat(rows,.5),'p95_ms':stat(rows,.95),'p99_ms':stat(rows,.99)}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);p.add_argument('--helper',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--samples',type=int,default=20);args=p.parse_args()
    if args.output.exists() or not 5<=args.samples<=100:raise ValueError('new output and 5..100 samples required')
    binary=args.binary.resolve(strict=True);helper=args.helper.resolve(strict=True)
    env={k:v for k,v in os.environ.items() if not k.upper().startswith('GIT_')};env.update(GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL=os.devnull,GIT_TERMINAL_PROMPT='0')
    base=inc.supported_chain();base['.gitignore']=b'.ripple/\ntemplate/\n'
    cold=[];noop=[];body=[];impact=[];startup=[];phases=[];resources=[];sizes=[]
    with tempfile.TemporaryDirectory(prefix='ripple-workflows-') as directory:
        container=Path(directory)
        def repo(root):
            root.mkdir();template=root/'template';template.mkdir()
            for path,raw in base.items():(root/path).write_bytes(raw)
            git=['git','-c','core.hooksPath=','-c','core.fsmonitor=false','-c','core.autocrlf=false','-c','commit.gpgsign=false']
            run([*git,'init','-q','--initial-branch=main',f'--template={template}'],root,env)
            for key,value in [('user.name','Ripple Fixture'),('user.email','fixture@ripple.invalid')]:run([*git,'config',key,value],root,env)
            run([*git,'add','--all'],root,env);run([*git,'commit','-qm','fixture'],root,env)
        for sample in range(args.samples+1):
            root=container/f'run-{sample}';repo(root)
            out,c=run([binary,'index',root,'--format','json'],root,env)
            out,n=run([binary,'index',root,'--format','json'],root,env);assert json.loads(out.stdout)['data']['update']['parsed_files']==0
            out,i=run([binary,'impact','m0.ts::f0','--root',root,'--format','json'],root,env)
            phase,_=run([helper,root],root,env)
            _,s=run([binary,'--version'],root,env)
            # A same-size body mutation plus refresh measures real Git dirty provenance.
            (root/'m0.ts').write_bytes(base['m0.ts'].replace(b'return 0;',b'return 1;'))
            out,b=run([binary,'index',root,'--format','json'],root,env);assert json.loads(out.stdout)['data']['update']['parsed_files']==1
            size=sum(f.stat().st_size for f in (root/'.ripple').iterdir() if f.is_file())
            # Exact CLI peak RSS, including startup, via BSD time on this recorded macOS host.
            if platform.system()=='Darwin':
                timed,_=run(['/usr/bin/time','-l',binary,'impact','m0.ts::f0','--root',root,'--format','json'],root,env)
                rss=int(next(line.split()[0] for line in timed.stderr.decode().splitlines() if 'maximum resident set size' in line));resources.append(rss)
            if sample:cold.append(c);noop.append(n);impact.append(i);body.append(b);startup.append(s);phases.append(json.loads(phase.stdout));sizes.append(size)
            shutil.rmtree(root)
        # Reject a metadata-only freshness shortcut using a same-size, restored-mtime edit.
        root=container/'counterexample';repo(root);run([binary,'index',root],root,env)
        file=root/'m0.ts';before=file.stat();file.write_bytes(base['m0.ts'].replace(b'return 0;',b'return 1;'));os.utime(file,ns=(before.st_atime_ns,before.st_mtime_ns));after=file.stat()
        rejected=subprocess.run([str(binary),'status','--root',str(root),'--format','json'],capture_output=True,env=env,timeout=120)
        counterexample={'metadata_equal':(before.st_size,before.st_mtime_ns)==(after.st_size,after.st_mtime_ns),'production_rejected_stale':rejected.returncode!=0 and b'stale' in rejected.stderr,'stderr':rejected.stderr.decode()}
        assert counterexample['metadata_equal'] and counterexample['production_rejected_stale']
    metrics={'cold':summary(cold),'no_op':summary(noop),'body':summary(body),'verified_impact':summary(impact),'startup_version':summary(startup)}
    for key in phases[0]:
        if key.endswith('_ms'):metrics[key]=summary([r[key] for r in phases])
    loc=sum(raw.count(b'\n') for path,raw in base.items() if path.endswith('.ts'))
    result={'source_commit':subprocess.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'helper_sha256':hashlib.sha256(helper.read_bytes()).hexdigest(),'samples':args.samples,'warmups':1,'workload':{'id':'supported-chain-65-v1','files':65,'loc':loc,'sha256':hashlib.sha256(json.dumps({p:hashlib.sha256(b).hexdigest() for p,b in sorted(base.items())},sort_keys=True).encode()).hexdigest()},'hardware':{'os':platform.platform(),'cpu':subprocess.check_output(['sysctl','-n','machdep.cpu.brand_string'],text=True).strip() if platform.system()=='Darwin' else platform.processor(),'memory_bytes':int(subprocess.check_output(['sysctl','-n','hw.memsize'],text=True)) if platform.system()=='Darwin' else None},'metrics':metrics,'cold_files_per_second':65/(metrics['cold']['p50_ms']/1000),'cold_loc_per_second':loc/(metrics['cold']['p50_ms']/1000),'raw':{'cold_ms':cold,'no_op_ms':noop,'body_ms':body,'impact_ms':impact,'startup_ms':startup,'phases':phases,'index_bytes_three_generations':sizes,'peak_impact_rss_bytes':resources},'rejected_optimization':counterexample,'state':'Release; warm OS cache, no cache eviction; fresh per-sample Git roots/indexes, one discarded warm-up. CLI time includes startup, real Git provenance, verification, serialization. Phase helper compiles actual production capture source and checks query equality with production load_current; stage times are sequential observations, not subtraction. Peak RSS: BSD time -l, impact only. Fixed case order; background load uncontrolled. Startup version is a proxy, not subtracted from queries. No latency thresholds.','command':' '.join(__import__('sys').argv)}
    args.output.mkdir(parents=True);(args.output/'workflows.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(metrics,indent=2))
if __name__=='__main__':main()
