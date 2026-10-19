"""setup_tools: platform mapping, version parsing, safe extraction and the tool lookup order."""
import io
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import setup_tools  # noqa: E402
from site_features import tool_command  # noqa: E402


class SetupToolsTests(unittest.TestCase):
    def test_every_supported_platform_has_both_tools_pinned(self):
        for key in (('windows', 'x86_64'), ('windows', 'aarch64'), ('linux', 'x86_64'), ('linux', 'aarch64'),
                    ('linux-musl', 'x86_64'), ('linux-musl', 'aarch64'), ('darwin', 'x86_64'), ('darwin', 'aarch64')):
            for name in ('zola', 'typst'):
                asset, digest = setup_tools.asset_for(name, key)
                self.assertTrue(asset.startswith(name), asset)
                self.assertIn(setup_tools.RELEASES[name]['version'] if name == 'zola' else '', asset)
                self.assertRegex(digest, r'^[0-9a-f]{64}$')

    def test_musl_linux_falls_back_to_the_glibc_zola_only_when_no_musl_build_is_pinned(self):
        self.assertEqual(setup_tools.asset_for('zola', ('linux-musl', 'aarch64'))[0],
                         'zola-v0.23.4-aarch64-unknown-linux-gnu.tar.gz')
        self.assertEqual(setup_tools.asset_for('zola', ('linux-musl', 'x86_64'))[0],
                         'zola-v0.23.4-x86_64-unknown-linux-musl.tar.gz')

    def test_version_parsing(self):
        self.assertEqual(setup_tools.parse_version('zola 0.23.4'), '0.23.4')
        self.assertEqual(setup_tools.parse_version('typst 0.15.1 (unknown commit)'), '0.15.1')
        self.assertEqual(setup_tools.parse_version('v22.19.0\n'), '22.19.0')
        self.assertIsNone(setup_tools.parse_version(''))

    def test_lookup_prefers_env_then_tools_then_path(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            with patch.dict(os.environ, {'ZOLA': 'custom-zola'}):
                self.assertEqual(tool_command('zola', root), 'custom-zola')
                self.assertEqual(setup_tools.locate('zola', root), ('custom-zola', 'env'))
            with patch.dict(os.environ, {}):
                os.environ.pop('ZOLA', None)
                bundled = root / '.tools' / 'zola' / setup_tools.binary_name('zola')
                bundled.parent.mkdir(parents=True)
                bundled.write_bytes(b'')
                self.assertEqual(tool_command('zola', root), str(bundled))
                self.assertEqual(setup_tools.locate('zola', root)[1], 'tools')
                bundled.unlink()
                with patch('shutil.which', return_value=None):
                    self.assertEqual(tool_command('zola', root), 'zola')
                    self.assertEqual(setup_tools.locate('zola', root), (None, None))

    def test_archives_that_escape_the_destination_are_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            bad_zip = Path(folder) / 'bad.zip'
            with zipfile.ZipFile(bad_zip, 'w') as bundle:
                bundle.writestr('../escape.txt', 'x')
            with self.assertRaises(SystemExit):
                setup_tools.extract(bad_zip, Path(folder) / 'out')
            bad_tar = Path(folder) / 'bad.tar.gz'
            with tarfile.open(bad_tar, 'w:gz') as bundle:
                info = tarfile.TarInfo('/abs.txt')
                info.size = 1
                bundle.addfile(info, io.BytesIO(b'x'))
            with self.assertRaises(SystemExit):
                setup_tools.extract(bad_tar, Path(folder) / 'out2')
            self.assertFalse((Path(folder) / 'escape.txt').exists())

    def test_extract_finds_the_binary_inside_a_nested_folder(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = Path(folder) / 'typst.tar.xz'
            with tarfile.open(archive, 'w:xz') as bundle:
                for name, text in (('typst-demo/README.md', 'readme'),
                                   (f'typst-demo/{setup_tools.binary_name("typst")}', 'bin')):
                    info = tarfile.TarInfo(name)
                    data = text.encode()
                    info.size = len(data)
                    bundle.addfile(info, io.BytesIO(data))
            out = Path(folder) / 'out'
            setup_tools.extract(archive, out)
            self.assertEqual(setup_tools.find_binary(out, 'typst').read_bytes(), b'bin')
            with self.assertRaises(SystemExit):
                setup_tools.find_binary(out, 'zola')


if __name__ == '__main__':
    unittest.main()
