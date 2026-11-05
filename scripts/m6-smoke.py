#!/usr/bin/env python3
"""Installed v0.3 candidate: prior commands plus adversarial incremental transitions."""
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


prior = json.loads(run([sys.executable, Path(__file__).with_name('m5-smoke.py'), '--binary', binary], Path.cwd()).stdout)
with tempfile.TemporaryDirectory(prefix='ripple-m6-') as directory:
    root = Path(directory) / 'project Ω'
    root.mkdir()
    template = Path(directory) / 'template'
    template.mkdir()
    git = ['git', '-c', 'core.hooksPath=', '-c', 'core.fsmonitor=false', '-c', 'core.autocrlf=false']
    run([*git, 'init', '-q', '--initial-branch=main', f'--template={template}'], root)
    for key, value in [('user.name', 'Ripple Fixture'), ('user.email', 'fixture@ripple.invalid'), ('commit.gpgsign', 'false')]:
        run([*git, 'config', '--local', key, value], root)

    def write(path, text):
        (root / path).write_bytes(text.encode('utf-8'))

    def command(*argv):
        return json.loads(run([binary, *argv, '--format', 'json'], root).stdout)['data']

    write('.gitignore', '.ripple/\n')
    write('lib.ts', 'export function f(){return 1;}\n')
    run([*git, 'add', '--all'], root)
    run([*git, 'commit', '-qm', 'fixture'], root)
    original_index = (root / '.git/index').read_bytes()
    command('index')
    write('a.ts', "// café 🦀\r\nexport * from './b';\r\nexport function a(){return 1;}\r\n")
    write('b.ts', "export * from './a';\n")
    added = command('index')
    assert added['counts']['files'] == 3 and added['update']['parsed_files'] == 2
    assert command('status')['freshness'] == 'verified'
    assert command('index')['update']['parsed_files'] == 0
    write('.ignore', 'b.ts\n')
    ignored = command('index')
    assert ignored['counts']['files'] == 2 and ignored['update']['rebuild_reason'] == 'global_inputs'
    write('.ignore', '# restored\n')
    write('tsconfig.json', '{"compilerOptions":{"baseUrl":".","paths":{"@lib":["lib"]}}}')
    restored = command('index')
    assert restored['counts']['files'] == 3 and restored['update']['rebuild_reason'] == 'global_inputs'
    assert command('index', '--full')['update']['parsed_files'] == 3
    assert command('status')['freshness'] == 'verified'
    assert original_index == (root / '.git/index').read_bytes()
print(json.dumps({'m6_smoke': 'passed', 'prior_capabilities': prior, 'unicode_crlf_cycles': True, 'ignore_config_refresh': True, 'git_index_preserved': True}))
