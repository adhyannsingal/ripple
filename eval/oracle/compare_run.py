#!/usr/bin/env python3
"""Build an isolated source projection, run trusted Ripple/compiler, write sorted artifacts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile
import urllib.parse
from compare import compare, markdown

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent


def run(args, cwd=None):
    env = {k: v for k, v in os.environ.items() if not k.upper().startswith('GIT_')}
    env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull, GIT_TERMINAL_PROMPT='0')
    result = subprocess.run(list(map(str, args)), cwd=cwd, env=env, capture_output=True, timeout=120, check=True)
    return result.stdout.decode('utf-8')


def write(path, data):
    path.write_text(json.dumps(data, indent=2, sort_keys=True, ensure_ascii=False) + '\n', encoding='utf-8')


def evaluate(source, files, binary, output, corpus):
    output = Path(output)
    if output.exists():
        raise ValueError('output directory must not exist')
    binary = Path(binary).resolve(strict=True)
    if run(['node', '--version']).strip() != 'v24.22.1':
        raise ValueError('use pinned Node 24.22.1 on PATH')
    with tempfile.TemporaryDirectory(prefix='ripple-oracle-') as temp:
        root = Path(temp)
        input_hashes = {}
        for f in files:
            p = Path(f)
            if p.is_absolute() or '..' in p.parts or '\\' in f or ':' in f or p.suffix not in ('.ts', '.tsx'):
                raise ValueError('invalid source path')
            raw = (source / p).read_bytes()
            if len(raw) > 4 * 1024 * 1024:
                raise ValueError('source too large')
            input_hashes[f] = hashlib.sha256(raw).hexdigest()
            dest = root / p
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(raw)
        if corpus.get('files'):
            expected = {i['path']: i['sha256'] for i in corpus['files'] if i['path'].endswith(('.ts', '.tsx'))}
            if input_hashes != expected:
                raise ValueError('projection differs from pinned corpus')
        (root / '.gitignore').write_text('.ripple/\n')
        run(['git', '-c', 'init.templateDir=', '-c', 'core.hooksPath=' + os.devnull, 'init', '-q', root])
        listing = root / 'files.json'
        write(listing, files)
        oracle = json.loads(run(['node', HERE / 'adapter.mjs', root, listing]))
        index = json.loads(run([binary, 'index', root, '--format', 'json']))
        status = json.loads(run([binary, 'status', '--root', root, '--format', 'json']))
        # Private disposable index already validated by CLI status. Export using one read transaction.
        uri = 'file:' + urllib.parse.quote(str(root / '.ripple/index.db')) + '?mode=ro'
        with sqlite3.connect(uri, uri=True) as db:
            db.execute('BEGIN')
            generation = db.execute('SELECT generation_id FROM current_generation WHERE singleton=1').fetchone()[0]
            records = lambda table, col: [json.loads(r[0]) for r in db.execute(f'SELECT {col} FROM {table} WHERE generation_id=? ORDER BY 1', (generation,))]
            ripple = {'symbols': records('symbols', 'record_json'), 'edges': records('edges', 'record_json'), 'unresolved': records('unresolved_references', 'record_json'), 'files': records('files', 'facts_json')}
        if oracle['input_hashes'] != input_hashes:
            raise ValueError('oracle input mismatch')
        report = compare(oracle, ripple)
        report['metadata'] = {'analyzer_commit': run(['git', '-C', REPO, 'rev-parse', 'HEAD']).strip(),
            'rust_source_tree': run(['git', '-C', REPO, 'rev-parse', 'HEAD:crates']).strip(),
            'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'node': oracle['node'], 'typescript': oracle['typescript'],
            'compiler_options': oracle['options'], 'corpus': corpus, 'input_hashes': input_hashes,
            'analyzer_versions': status['data']['metadata']['versions']}
        output.mkdir(parents=True)
        for name, data in [('oracle', oracle), ('ripple', ripple), ('comparison', report)]:
            write(output / (name + '.json'), data)
        (output / 'report.md').write_text(markdown(report), encoding='utf-8')
        print(json.dumps({'corpus': corpus['id'], 'metrics': report['metrics'], 'counts': report['counts'], 'categories': report['ranked_categories']}, ensure_ascii=False))
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True, type=Path)
    parser.add_argument('--manifest', type=Path)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    corpus = json.loads(args.manifest.read_text()) if args.manifest else {'id': 'mini', 'projection': 'owned hand-checked mini corpus'}
    files = [i['path'] for i in corpus['files'] if i['path'].endswith(('.ts', '.tsx'))] if args.manifest else json.loads((args.source / 'files.json').read_text())
    evaluate(args.source.resolve(), sorted(files), args.binary, args.output, corpus)
