import importlib.util
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile
spec=importlib.util.spec_from_file_location('release',Path(__file__).resolve().parents[1]/'release.py')
r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
class ReleaseContracts(unittest.TestCase):
    def package(self,root,name='ripple'):
        raw=b'owned fixture'
        with zipfile.ZipFile(root/'test.zip','w')as z:z.writestr(name,raw)
        m={'archive':'test.zip','archive_sha256':r.digest((root/'test.zip').read_bytes()),'binary':'ripple','files':{name:r.digest(raw)},'source_commit':'a'*40}
        (root/'manifest.json').write_text(json.dumps(m));return m
    def test_exact_inventory_hashes_verify_without_execution(self):
        with tempfile.TemporaryDirectory()as d:
            root=Path(d);self.package(root);self.assertEqual(r.verify(root)['binary'],'ripple')
    def test_tamper_is_rejected_before_execution(self):
        with tempfile.TemporaryDirectory()as d:
            root=Path(d);self.package(root);(root/'test.zip').write_bytes(b'tampered')
            with self.assertRaisesRegex(ValueError,'checksum'):r.verify(root)
    def test_path_escape_and_symlink_are_rejected(self):
        for name in ['../ripple','/ripple','a/../ripple','C:/ripple','ripple\n']:
            with self.subTest(name=name),self.assertRaises(ValueError):r.safe(name)
        with tempfile.TemporaryDirectory()as d:
            root=Path(d);m=self.package(root)
            with zipfile.ZipFile(root/'test.zip','w')as z:
                i=zipfile.ZipInfo('ripple');i.external_attr=0o120777<<16;z.writestr(i,b'owned fixture')
            m['archive_sha256']=r.digest((root/'test.zip').read_bytes());(root/'manifest.json').write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError,'unsafe member'):r.verify(root)
