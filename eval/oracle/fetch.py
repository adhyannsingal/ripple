#!/usr/bin/env python3
"""Fetch only manifest-approved raw files; never clone/install/execute corpus scripts."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import urllib.request


def fetch(manifest, destination):
    data = json.loads(Path(manifest).read_text())
    if not re.fullmatch(r'https://github.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', data['repository']):
        raise ValueError('expected GitHub repository URL')
    if not re.fullmatch('[0-9a-f]{40}', data['commit']):
        raise ValueError('expected immutable commit')
    destination = Path(destination)
    if destination.exists():
        raise ValueError('destination must not exist')
    inputs = []
    for entry in data['files']:
        path = entry['path']
        parts = PurePosixPath(path).parts
        if not parts or path.startswith('/') or any(p in ('.', '..') for p in parts) or '\\' in path or ':' in path:
            raise ValueError('invalid corpus path')
        url = 'https://raw.githubusercontent.com/' + data['repository'].removeprefix('https://github.com/') + '/' + data['commit'] + '/' + path
        with urllib.request.urlopen(url, timeout=30) as response:
            raw = response.read(4 * 1024 * 1024 + 1)
        if len(raw) != entry['bytes'] or hashlib.sha256(raw).hexdigest() != entry['sha256']:
            raise ValueError('corpus content mismatch: ' + path)
        inputs.append((path, raw))
    # All bytes checked before writing; use a new destination only.
    destination.mkdir(parents=True)
    for path, raw in inputs:
        output = destination / path
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(raw)
    print(json.dumps({'corpus': data['id'], 'files': len(inputs), 'verified': True}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest')
    parser.add_argument('destination')
    args = parser.parse_args()
    fetch(args.manifest, args.destination)
