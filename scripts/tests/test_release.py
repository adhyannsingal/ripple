import importlib.util
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile
from unittest import mock
import io
import os
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

    def test_vendored_notice_bytes_match_immutable_manifest(self):
        manifest=json.loads((r.ROOT/'docs/third-party-notices/manifest.json').read_text())
        for item in manifest['notices']:
            self.assertEqual(r.digest((r.ROOT/'docs/third-party-notices'/item['file']).read_bytes()),item['sha256'])

    def test_oversized_inputs_fail_before_zip_or_execution(self):
        with tempfile.TemporaryDirectory()as d:
            root=Path(d);self.package(root)
            for name,limit in [('manifest.json',r.MAX_MANIFEST_BYTES),('test.zip',r.MAX_ARCHIVE_BYTES)]:
                with self.subTest(name=name):
                    self.package(root)
                    with (root/name).open('wb')as f:f.truncate(limit+1)
                    with mock.patch.object(r.zipfile,'ZipFile',side_effect=AssertionError('ZIP opened')),mock.patch.object(r,'command',side_effect=AssertionError('executed')):
                        with self.assertRaisesRegex(ValueError,'byte limit'):r.verify(root,smoke=True)

    def test_stream_hash_is_chunked_and_detects_growth(self):
        class Chunked(io.BytesIO):
            def read(self,size=-1):
                if size<0 or size>r.HASH_CHUNK_BYTES:raise AssertionError('unbounded read')
                return super().read(size)
        raw=b'x'*(r.HASH_CHUNK_BYTES+17)
        self.assertEqual(r.stream_digest(Chunked(raw),len(raw)),r.digest(raw))
        with self.assertRaisesRegex(ValueError,'byte limit'):r.stream_digest(Chunked(raw),len(raw)-1)

    @unittest.skipUnless(hasattr(os,'symlink'),'symlink API unavailable')
    def test_manifest_archive_symlinks_and_directories_are_rejected(self):
        with tempfile.TemporaryDirectory()as d:
            root=Path(d);self.package(root)
            for name in ['manifest.json','test.zip']:
                for kind in ['directory','symlink']:
                    with self.subTest(name=name,kind=kind):
                        self.package(root)
                        target=root/name;saved=target.read_bytes();target.unlink()
                        if kind=='directory':target.mkdir()
                        else:
                            (root/'outside').write_bytes(saved)
                            try:target.symlink_to(root/'outside')
                            except OSError as error:self.skipTest(str(error))
                        try:
                            with self.assertRaisesRegex(ValueError,'regular'):r.verify(root)
                        finally:
                            if kind=='directory':target.rmdir()
                            else:target.unlink()

    def test_expansion_limit_rejects_before_execution(self):
        with tempfile.TemporaryDirectory()as d:
            root=Path(d);m=self.package(root)
            # The central-directory declaration is checked without allocating this expansion.
            info=zipfile.ZipInfo('ripple');info.file_size=129*1024*1024
            archive=mock.MagicMock();archive.infolist.return_value=[info];archive.namelist.return_value=['ripple']
            with mock.patch.object(r.zipfile,'ZipFile')as constructor,mock.patch.object(r,'command',side_effect=AssertionError('executed')):
                constructor.return_value.__enter__.return_value=archive
                with self.assertRaisesRegex(ValueError,'archive byte limit'):r.verify(root,smoke=True)
                archive.read.assert_not_called()
