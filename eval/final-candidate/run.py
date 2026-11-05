#!/usr/bin/env python3
"""Refresh final oracle/replay/mutation/performance evidence from one candidate source."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
ROOT=Path(__file__).resolve().parents[2]
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--binary',type=Path,required=True);p.add_argument('--node',type=Path,required=True);p.add_argument('--typescript',type=Path,required=True)
p.add_argument('--incremental-helper',type=Path,required=True);p.add_argument('--workflow-helper',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
args=p.parse_args()
if args.output.exists():raise ValueError('new output required')
out=args.output.resolve();out.mkdir(parents=True)
binary=args.binary.resolve(strict=True);node=args.node.resolve(strict=True)
env=os.environ.copy();env['PATH']=str(node.parent)+os.pathsep+env.get('PATH','')
def run(argv):
    subprocess.run([str(a)for a in argv],cwd=ROOT,env=env,check=True,timeout=600)
run(['python3',ROOT/'eval/mutation/run.py','--binary',binary,'--node',node,'--typescript',args.typescript.resolve(),'--output',out/'mutation'])
run(['python3',ROOT/'eval/commit-replay/run.py','--helper',args.incremental_helper.resolve(),'--output',out/'replay'])
run(['python3',ROOT/'eval/oracle/compare_run.py','--source',ROOT/'eval/corpora/mini','--binary',binary,'--output',out/'oracle-mini'])
run(['python3',ROOT/'eval/oracle/fetch.py',ROOT/'eval/corpora/sourcemap-codec.json',out/'upstream-source'])
run(['python3',ROOT/'eval/oracle/compare_run.py','--source',out/'upstream-source','--manifest',ROOT/'eval/corpora/sourcemap-codec.json','--binary',binary,'--output',out/'oracle-external'])
run(['python3',ROOT/'bench/scripts/incremental.py','--helper',args.incremental_helper.resolve(),'--output',out/'incremental','--samples','20'])
run(['python3',ROOT/'bench/scripts/workflows.py','--binary',binary,'--helper',args.workflow_helper.resolve(),'--output',out/'workflows','--samples','20'])
run(['python3',ROOT/'docs/examples/quickstart.py','--binary',binary])
run(['python3',ROOT/'docs/examples/m7-test/reproduce.py','--binary',binary,'--output',out/'external-test-demo'])
# All evidence stays pinned to one source; corpus bytes are disposable downloads.
shutil.rmtree(out/'upstream-source')
summary={'source_commit':subprocess.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'version':subprocess.check_output([str(binary),'--version'],text=True).strip(),'command':' '.join(__import__('sys').argv)}
summary['mutation']=json.load(open(out/'mutation/mutations.json'))['aggregate']
summary['replay_mismatches']=json.load(open(out/'replay/replay.json'))['canonical_mismatches']
summary['incremental_mismatches']=json.load(open(out/'incremental/incremental.json'))['canonical_mismatches']
summary['oracle']={name:json.load(open(out/name/'comparison.json'))['metrics']for name in ['oracle-mini','oracle-external']}
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
