"""Staging must protect formulas without changing the author's source tree."""

import json
import os
from pathlib import Path
import stat
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from prepare_site import ORIGINAL_BODIES, PreparedSite, _reject_link


class PreparedSiteTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='prepared-site-test-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / 'site'
        self.target = self.root / 'staged'
        self.source.mkdir()
        self.write('config.toml', 'base_url = "https://example.org"\n')
        self.transform = patch('prepare_site.protect_math', side_effect=lambda text: 'PROTECTED\n' + text)
        self.protect = self.transform.start()
        self.addCleanup(self.transform.stop)

    def write(self, relative, text):
        path = self.source / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8')
        return path

    def test_sources_untouched_and_only_content_markdown_transformed(self):
        originals = {
            'content/posts/a/index.md': '$x_i$\n',
            'content/posts/a/asset.svg': '<svg/>',
            'content/posts/a/data.txt': '$x_i$',
            'templates/example.md': '$x_i$',
            'static/example.md': '$x_i$',
            'sass/main.scss': 'body {}',
            'data/profile.json': '{"name":"User"}',
            'themes/example/templates/index.html': '<main/>',
        }
        for relative, text in originals.items():
            self.write(relative, text)
        snapshot = {relative: (self.source / relative).read_bytes() for relative in originals}
        stage = PreparedSite(self.source, self.target)
        changed = stage.sync()
        self.assertIn(Path('data/profile.json'), changed)
        for relative, content in snapshot.items():
            self.assertEqual((self.source / relative).read_bytes(), content)
            expected = b'PROTECTED\n' + content if relative.endswith('index.md') else content
            self.assertEqual((self.target / relative).read_bytes(), expected)
        self.protect.assert_called_once()

    def test_incremental_noop_edit_add_and_delete(self):
        article = self.write('content/posts/a/index.md', '$x_i$')
        self.write('data/profile.json', '{"name":"A"}')
        stage = PreparedSite(self.source, self.target)
        stage.sync()
        output = self.target / 'content/posts/a/index.md'
        before = output.stat().st_mtime_ns
        self.protect.reset_mock()
        self.assertEqual(stage.sync(), [])
        self.assertEqual(output.stat().st_mtime_ns, before)
        self.protect.assert_not_called()
        article.write_text('$x_{updated}$', encoding='utf-8')
        self.write('static/new.txt', 'added')
        self.write('data/profile.json', '{"name":"Updated"}')
        self.assertEqual(stage.sync(), [Path('content/posts/a/index.md'),
                                        ORIGINAL_BODIES, Path('data/profile.json'), Path('static/new.txt')])
        # Generated output is not owned by the mirror.
        generated = self.target / 'public/keep.html'
        generated.parent.mkdir()
        generated.write_text('generated', encoding='utf-8')
        article.unlink()
        article.parent.rmdir()
        self.assertEqual(stage.sync(), [Path('content/posts/a/index.md'), ORIGINAL_BODIES])
        self.assertFalse(output.exists())
        self.assertFalse(output.parent.exists())
        self.assertTrue(generated.exists())

    def test_output_manifests_and_unapproved_root_files_are_excluded(self):
        for relative in ('public/index.html', '.git/config', 'secret.env',
                         'article-manifest.json', 'content/posts/a/article-manifest.json',
                         'static/article-manifest.json', 'themes/example/.git/config',
                         'content/public/old.html'):
            self.write(relative, 'local secret')
        stage = PreparedSite(self.source, self.target)
        self.assertEqual(stage.sync(), [Path('config.toml'), ORIGINAL_BODIES])
        self.assertEqual({path.relative_to(self.target) for path in self.target.rglob('*') if path.is_file()},
                         {Path('config.toml'), ORIGINAL_BODIES})
        self.assertEqual(json.loads((self.target / ORIGINAL_BODIES).read_text(encoding='utf-8')), {})

    def test_reading_data_generated_without_placeholder_and_tracks_edits_and_deletes(self):
        original = '+++\ntitle = "Fresh note"\n+++\n\nRaw $x_i$ and 汉字.\n'
        article = self.write('content/posts/fresh/index.md', original)
        article.write_bytes(original.encode('utf-8'))
        self.assertFalse((self.source / 'data').exists())
        stage = PreparedSite(self.source, self.target)
        self.assertIn(ORIGINAL_BODIES, stage.sync())
        data = self.target / ORIGINAL_BODIES
        self.assertEqual(json.loads(data.read_text(encoding='utf-8')),
                         {'posts/fresh/index.md': '\nRaw $x_i$ and 汉字.\n'})
        self.assertEqual(article.read_text(encoding='utf-8'), original)
        self.assertFalse((self.source / 'data').exists())
        before = data.stat().st_mtime_ns
        with patch('prepare_site._markdown_body', side_effect=AssertionError('Unchanged body should use cache')):
            self.assertEqual(stage.sync(), [])
        self.assertEqual(data.stat().st_mtime_ns, before)
        article.write_bytes(b'+++\ntitle = "Fresh note"\n+++\nChanged $y$\n')
        self.assertIn(ORIGINAL_BODIES, stage.sync())
        self.assertEqual(json.loads(data.read_text(encoding='utf-8')),
                         {'posts/fresh/index.md': 'Changed $y$\n'})
        article.unlink()
        self.assertIn(ORIGINAL_BODIES, stage.sync())
        self.assertEqual(json.loads(data.read_text(encoding='utf-8')), {})
        self.assertIn(Path('data'), stage._directories)
        self.assertEqual(stage.sync(), [])

    def test_overlapping_and_nonempty_targets_are_rejected(self):
        for target in (self.source, self.source / 'stage', self.root):
            with self.subTest(target=target), self.assertRaises(ValueError):
                PreparedSite(self.source, target)
        self.target.mkdir()
        sentinel = self.target / 'keep.txt'
        sentinel.write_text('keep', encoding='utf-8')
        with self.assertRaises(ValueError):
            PreparedSite(self.source, self.target)
        self.assertEqual(sentinel.read_text(encoding='utf-8'), 'keep')

    def test_file_directory_transitions(self):
        path = self.write('static/item', 'first file')
        stage = PreparedSite(self.source, self.target)
        stage.sync()
        path.unlink()
        child = self.write('static/item/child.txt', 'child')
        stage.sync()
        self.assertTrue((self.target / 'static/item/child.txt').is_file())
        child.unlink()
        path.rmdir()
        self.write('static/item', 'file again')
        stage.sync()
        self.assertEqual((self.target / 'static/item').read_text(encoding='utf-8'), 'file again')

    def test_unowned_staging_collision_is_not_overwritten(self):
        stage = PreparedSite(self.source, self.target)
        stage.sync()
        unowned = self.target / 'static/new.txt'
        unowned.parent.mkdir()
        unowned.write_text('keep', encoding='utf-8')
        self.write('static/new.txt', 'replacement')
        with self.assertRaises(ValueError):
            stage.sync()
        self.assertEqual(unowned.read_text(encoding='utf-8'), 'keep')

    def test_sync_recovers_after_a_transform_error(self):
        self.write('content/article.md', '$x_i$')
        stage = PreparedSite(self.source, self.target)
        self.protect.side_effect = ValueError('temporary input error')
        with self.assertRaises(ValueError):
            stage.sync()
        # The generated reading data and config precede the failing article; they must
        # remain owned and must not prevent a retry after the article is fixed.
        self.protect.side_effect = lambda text: 'PROTECTED\n' + text
        self.assertEqual(stage.sync(), [Path('content/article.md')])
        self.assertEqual((self.target / 'content/article.md').read_text(encoding='utf-8'),
                         'PROTECTED\n$x_i$')

    def test_windows_reparse_points_are_rejected(self):
        info = SimpleNamespace(st_mode=stat.S_IFDIR,
                               st_file_attributes=getattr(stat, 'FILE_ATTRIBUTE_REPARSE_POINT', 0x400))
        with patch.object(Path, 'lstat', return_value=info), self.assertRaises(ValueError):
            _reject_link(self.source)

    def make_symlink(self, target, link, directory=False):
        try:
            os.symlink(target, link, target_is_directory=directory)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f'Symlinks are not available: {error}')

    def test_source_symlink_is_rejected_before_copying(self):
        outside = self.root / 'outside'
        outside.mkdir()
        (outside / 'secret.md').write_text('secret', encoding='utf-8')
        self.make_symlink(outside, self.source / 'content', directory=True)
        stage = PreparedSite(self.source, self.target)
        with self.assertRaises(ValueError):
            stage.sync()
        self.assertFalse(any(self.target.iterdir()))

    def test_target_symlink_is_rejected_before_deletion(self):
        article = self.write('content/article.md', 'article')
        stage = PreparedSite(self.source, self.target)
        stage.sync()
        output = self.target / 'content/article.md'
        output.unlink()
        outside = self.root / 'keep.md'
        outside.write_text('keep', encoding='utf-8')
        self.make_symlink(outside, output)
        article.unlink()
        with self.assertRaises(ValueError):
            stage.sync()
        self.assertEqual(outside.read_text(encoding='utf-8'), 'keep')


if __name__ == '__main__':
    unittest.main()
