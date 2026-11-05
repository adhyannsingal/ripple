"""Safety and evidence contracts for the immutable replay driver (no network)."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('replay', Path(__file__).with_name('run.py'))
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)


class ReplayContracts(unittest.TestCase):
    def test_unsafe_paths_cannot_escape_or_write_private_state(self):
        for path in ('../x', '/x', 'a/../x', '.ripple/x', 'src/.git/x', 'C:/x', 'src\\x', 'src//x', ''):
            with self.subTest(path=path), self.assertRaises(ValueError):
                replay.safe_path(path)
        self.assertEqual(replay.safe_path('src/λ.ts'), 'src/λ.ts')

    def test_materialization_preserves_only_private_generation_state(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / '.ripple').mkdir()
            (root / '.ripple/index.db').write_bytes(b'prior')
            (root / 'old.ts').write_bytes(b'old')
            replay.materialize(root, {'src/new.ts': b'new'})
            self.assertFalse((root / 'old.ts').exists())
            self.assertEqual((root / 'src/new.ts').read_bytes(), b'new')
            self.assertEqual((root / '.ripple/index.db').read_bytes(), b'prior')

    def test_hash_mismatch_stops_before_materialization(self):
        manifest = {'repository': 'https://github.com/a/b', 'snapshots': [{'commit': 'a' * 40,
                    'files': [{'path': 'src/a.ts', 'bytes': 1, 'sha256': '0' * 64}]}]}
        with patch.object(replay.urllib.request, 'urlopen') as fetch:
            fetch.return_value.__enter__.return_value.read.return_value = b'x'
            with self.assertRaisesRegex(ValueError, 'content mismatch'):
                replay.fetch_snapshots(manifest)

    def test_semantic_repeat_check_preserves_confidence_diagnostics_and_counters(self):
        result = {'canonical_sha256': 'x', 'update': {'parsed_files': 1}, 'incomplete': True,
                  'config_diagnostics': ['unsupported'], 'incremental_ms': 9, 'helper_wall_ms': 10}
        expected = replay.semantic(result)
        self.assertNotIn('incremental_ms', expected)
        self.assertEqual(expected['update'], {'parsed_files': 1})
        self.assertTrue(expected['incomplete'])
        self.assertEqual(expected['config_diagnostics'], ['unsupported'])


if __name__ == '__main__':
    unittest.main()
