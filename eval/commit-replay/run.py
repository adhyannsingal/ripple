#!/usr/bin/env python3
"""Replay verified raw GitHub blobs; never execute corpus scripts or checkout user work."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]


def safe_path(path):
    if not isinstance(path, str) or not path or '\\' in path or ':' in path:
        raise ValueError('invalid corpus path')
    p = PurePosixPath(path)
    if p.is_absolute() or any(part in ('.', '..', '.ripple', '.git') for part in path.split('/')) or str(p) != path:
        raise ValueError('invalid corpus path: ' + path)
    return path


def fetch_snapshots(manifest):
    repo = manifest['repository']
    if not re.fullmatch(r'https://github.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repo):
        raise ValueError('invalid repository URL')
    snapshots = []
    for entry in manifest['snapshots']:
        sha = entry['commit']
        if not re.fullmatch('[0-9a-f]{40}', sha):
            raise ValueError('immutable commit required')
        files = {}
        for item in entry['files']:
            path = safe_path(item['path'])
            if path in files or not 0 <= item['bytes'] <= 4 * 1024 * 1024:
                raise ValueError('duplicate path or oversized blob')
            url = 'https://raw.githubusercontent.com/' + repo.removeprefix('https://github.com/') + '/' + sha + '/' + path
            with urllib.request.urlopen(url, timeout=30) as response:
                raw = response.read(4 * 1024 * 1024 + 1)
            if len(raw) != item['bytes'] or hashlib.sha256(raw).hexdigest() != item['sha256']:
                raise ValueError('corpus content mismatch: ' + path)
            files[path] = raw
        if manifest['license']['path'] not in files:
            raise ValueError('license missing from snapshot')
        snapshots.append((sha, files))
    files = dict(snapshots[-1][1])
    for edit in manifest['supplemental']:
        path = safe_path(edit['path'])
        if edit['text'] is None:
            del files[path]
        else:
            files[path] = edit['text'].encode()
        snapshots.append(('curated:' + edit['id'], dict(files)))
    return snapshots


def materialize(root, files):
    # Callers own this fresh temporary directory. Preserve only its private generation state.
    for child in root.iterdir():
        if child.name == '.ripple':
            continue
        if child.is_dir():
            shutil.rmtree(child)
        else:
            child.unlink()
    for path, raw in files.items():
        output = root / safe_path(path)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(raw)


def compare(helper, incremental, files, temporary):
    materialize(incremental, files)
    with tempfile.TemporaryDirectory(dir=temporary) as directory:
        cold = Path(directory)
        materialize(cold, files)
        start = time.perf_counter()
        run = subprocess.run([str(helper), str(incremental), str(cold)], capture_output=True, text=True, timeout=120, check=True)
        result = json.loads(run.stdout)
        result['helper_wall_ms'] = (time.perf_counter() - start) * 1000
        return result


def semantic(result):
    return {key: value for key, value in result.items() if not key.endswith('_ms')}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, default=ROOT / 'eval/corpora/sourcemap-codec-history.json')
    parser.add_argument('--helper', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError('output must not exist')
    helper = args.helper.resolve(strict=True)
    manifest = json.loads(args.manifest.read_text())
    snapshots = fetch_snapshots(manifest)
    campaigns = []
    for repeat in range(2):
        steps = []
        with tempfile.TemporaryDirectory(prefix='ripple-history-') as directory:
            temp = Path(directory)
            incremental = temp / 'incremental'
            incremental.mkdir()
            for identity, files in snapshots:
                result = compare(helper, incremental, files, temp)
                steps.append(dict(identity=identity, **result))
                print(f'repeat={repeat + 1} step={identity}: canonical equal', flush=True)
        campaigns.append(steps)
    if [semantic(s) for s in campaigns[0]] != [semantic(s) for s in campaigns[1]]:
        raise ValueError('clean repeat semantic results differ')
    source = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
    evidence = dict(format_version=1, corpus=manifest['id'], manifest_sha256=hashlib.sha256(args.manifest.read_bytes()).hexdigest(), source_commit=source,
                    helper_sha256=hashlib.sha256(helper.read_bytes()).hexdigest(), command=' '.join(os.sys.argv),
                    timing_scope='library refresh/cold: capture, verification, analysis and publication; excludes process startup and canonical reload; helper_wall_ms includes both and comparison',
                    repeats=campaigns, semantic_reproducible=True, canonical_mismatches=0)
    args.output.mkdir(parents=True)
    (args.output / 'replay.json').write_text(json.dumps(evidence, indent=2) + '\n')


if __name__ == '__main__':
    main()
