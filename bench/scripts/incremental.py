#!/usr/bin/env python3
"""Repeated release-mode update/cold measurements, each guarded by canonical equality."""
import argparse
import hashlib
import importlib.util
import json
import math
import platform
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('replay', ROOT / 'eval/commit-replay/run.py')
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)


def supported_chain():
    files = {'m0.ts': b'export function f0() { return 0; }\n'}
    for i in range(1, 64):
        files[f'm{i}.ts'] = f"import {{ f{i-1} }} from './m{i-1}'; export function f{i}() {{ return f{i-1}(); }}\n".encode()
    files['main.test.ts'] = b"import { f63 } from './m63'; f63();\n"
    return files


def changed(base, workload, case):
    files = dict(base)
    if case in ('cold', 'no_op'):
        return files
    if case == 'config':
        config = json.loads(files.get('tsconfig.json', b'{}'))
        config.setdefault('compilerOptions', {})['baseUrl'] = '.'
        files['tsconfig.json'] = (json.dumps(config, sort_keys=True) + '\n').encode()
        return files
    path = 'm0.ts' if workload == 'supported-chain-65' else 'src/vlq.ts'
    old, new = ((b'return 0;', b'return 1;') if workload == 'supported-chain-65' else
                (b'let value = 0;', b'let value = 1;')) if case == 'body' else (
                    (b'function f0(', b'function f0Changed(') if workload == 'supported-chain-65' else
                    (b'function decodeInteger(', b'function decodeIntegerChanged('))
    if files[path].count(old) != 1:
        raise ValueError('pinned edit target changed')
    files[path] = files[path].replace(old, new)
    return files


def compact(result):
    update = result['update']
    return {key: result[key] for key in ('incremental_ms', 'cold_ms', 'helper_wall_ms', 'canonical_sha256', 'counts', 'incomplete')} | {
        'update': {key: value for key, value in update.items() if key not in ('invalidation_candidates', 'unchanged_external_surfaces', 'added', 'changed', 'deleted')},
        'invalidation_candidates_count': len(update['invalidation_candidates']),
        'unchanged_external_surfaces_count': len(update['unchanged_external_surfaces']),
    }


def percentile(values, probability):
    return sorted(values)[math.ceil(len(values) * probability) - 1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--helper', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--samples', type=int, default=20)
    args = parser.parse_args()
    if args.output.exists() or not 5 <= args.samples <= 100:
        raise ValueError('new output directory and 5..100 samples required')
    helper = args.helper.resolve(strict=True)
    manifest_path = ROOT / 'eval/corpora/sourcemap-codec-history.json'
    manifest = json.loads(manifest_path.read_text())
    external = replay.fetch_snapshots(manifest)[4][1]
    workloads = {'sourcemap-projection-4': external, 'supported-chain-65': supported_chain()}
    results = []
    for name, base in workloads.items():
        for case in ('cold', 'no_op', 'body', 'export', 'config'):
            samples = []
            for sample in range(args.samples + 1):
                with tempfile.TemporaryDirectory(prefix='ripple-benchmark-') as directory:
                    temp = Path(directory)
                    incremental = temp / 'incremental'
                    incremental.mkdir()
                    if case != 'cold':
                        replay.compare(helper, incremental, base, temp)  # excluded setup; one prior generation
                    result = replay.compare(helper, incremental, changed(base, name, case), temp)
                    if case == 'no_op' and result['update']['parsed_files'] != 0:
                        raise ValueError('no-op failed parse-reuse contract')
                    if case == 'body' and (result['update']['parsed_files'] != 1 or result['update']['reused_files'] != len([p for p in base if p.endswith(('.ts', '.tsx'))]) - 1):
                        raise ValueError('body failed parse-reuse contract')
                    if case == 'config' and result['update']['rebuild_reason'] != 'global_inputs':
                        raise ValueError('config failed full-fallback contract')
                    if sample:  # discard one warm-up per workload/case
                        samples.append(compact(result))
            summaries = {field: {'p50': percentile([s[field] for s in samples], .5), 'p95': percentile([s[field] for s in samples], .95)}
                         for field in ('incremental_ms', 'cold_ms', 'helper_wall_ms')}
            results.append({'workload': name, 'case': case, 'samples': samples, 'summary_ms': summaries})
            print(name, case, json.dumps(summaries), flush=True)
    hardware = {'platform': platform.platform(), 'machine': platform.machine(), 'processor': platform.processor()}
    if platform.system() == 'Darwin':
        hardware.update(cpu=subprocess.check_output(['sysctl', '-n', 'machdep.cpu.brand_string'], text=True).strip(),
                        memory_bytes=int(subprocess.check_output(['sysctl', '-n', 'hw.memsize'], text=True)))
    artifact = {'format_version': 1, 'source_commit': subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip(),
                'command': ' '.join(__import__('sys').argv), 'helper_sha256': hashlib.sha256(helper.read_bytes()).hexdigest(),
                'manifest_sha256': hashlib.sha256(manifest_path.read_bytes()).hexdigest(), 'versions': result['versions'],
                'hardware': hardware, 'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(), 'profile': 'release (built by recorded reproduction command)', 'samples_per_case': args.samples, 'warmups_per_case': 1, 'percentile': 'nearest rank',
                'state': 'Release helper; warm OS/filesystem caches, no cache eviction. Fresh application trees each sample, exactly one prior generation for updates; cold has no index. Incremental measured before cold in same process; fixed order can bias caches. Library timing includes capture/reverification/analysis/publication, excludes startup/canonical reload; helper wall includes both builds and comparisons. Setup and network excluded. No user Git provenance subprocess cost (synthetic materialization provider).',
                'workloads': {name: {'files': len(files), 'bytes': sum(map(len, files.values())), 'input_sha256': hashlib.sha256(b''.join(path.encode() + b'\0' + raw for path, raw in sorted(files.items()))).hexdigest()} for name, files in workloads.items()},
                'canonical_mismatches': 0, 'measured_comparisons': len(results) * args.samples,
                'warmup_comparisons': len(results), 'results': results}
    args.output.mkdir(parents=True)
    (args.output / 'incremental.json').write_text(json.dumps(artifact, indent=2) + '\n')


if __name__ == '__main__':
    main()
