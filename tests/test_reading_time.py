"""Match pinned Zola reading times without exposing private original bodies."""

import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from prepare_site import ORIGINAL_BODIES, PreparedSite, _markdown_body


class OriginalBodyTests(unittest.TestCase):
    def test_frontmatter_is_removed_without_normalizing_body(self):
        for delimiter in ('+++', '---'):
            text = '\ufeff' + delimiter + '\r\nkey = "value"\r\n' + delimiter + '\r\n\r\n$x$\r\n'
            self.assertEqual(_markdown_body(text), '\r\n$x$\r\n')
        self.assertEqual(_markdown_body('$x$\n'), '$x$\n')
        self.assertEqual(_markdown_body('+++\nunclosed'), '+++\nunclosed')


class ReadingTimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.zola = os.environ.get('ZOLA') or shutil.which('zola')
        bundled = ROOT / '.tools/zola/zola.exe'
        if not cls.zola and bundled.is_file():
            cls.zola = str(bundled)
        if not cls.zola:
            raise unittest.SkipTest('Zola is required for reading-time checks')

    def setUp(self):
        scratch = ROOT / '.tools'
        scratch.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix='reading-test-', dir=scratch)
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name).resolve()
        if not self.directory.is_relative_to(scratch.resolve()):
            raise RuntimeError('Test directory escaped the workspace scratch directory')
        self.source = self.directory / 'source'
        for directory in ('content', 'templates', 'data'):
            (self.source / directory).mkdir(parents=True)
        (self.source / 'config.toml').write_text(
            'base_url = "https://example.test/blog/"\ndefault_language = "zh"\n', encoding='utf-8')
        (self.source / 'content/_index.md').write_text('+++\n+++\n', encoding='utf-8')
        (self.source / 'templates/index.html').write_text('index', encoding='utf-8')
        (self.source / 'templates/page.html').write_text(
            '{% set reading_sources = load_data(path="data/original-bodies.json") %}'
            '{% set reading_page = page %}'
            'minutes={% include "reading-time.html" %};native={{ page.reading_time }};'
            'prepared={{ page.relative_path in reading_sources }};', encoding='utf-8')
        shutil.copyfile(ROOT / 'site/templates/reading-time.html',
                        self.source / 'templates/reading-time.html')
        (self.source / ORIGINAL_BODIES).write_text('{}', encoding='utf-8')
        self.prepared = PreparedSite(self.source, self.directory / 'prepared')

    def article(self, name, body):
        path = self.source / 'content' / (name + '.md')
        path.write_bytes(('+++\ntitle = "' + 'FrontMatter' * 100 + '"\n+++\n' + body).encode('utf-8'))
        return path

    def build(self, source):
        result = subprocess.run([self.zola, '--root', str(source), 'build'],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        values = {}
        for path in (source / 'public').rglob('index.html'):
            match = re.search(r'minutes=(\d+);native=(\d+);prepared=(true|false);',
                              path.read_text(encoding='utf-8'))
            if match:
                values[path.parent.name] = (int(match[1]), int(match[2]), match[3] == 'true')
        return values

    def test_prepared_zh_matches_native_original_for_unicode_and_fences(self):
        bodies = {
            'empty': '',
            'boundary': '汉' * 439,
            # U+0345 is Unicode Alphabetic but Python str.isalpha returns false.
            'combining': '\u0345' * 440,
            'math': ('$$\\forall x_i \\in S: f(x_i) = y_i$$\n\n' * 30),
            'fences': '汉' * 439 + '\n```python\n' + 'a' * 1000 + '\n```\n',
            'unclosed': '汉' * 440 + '\n```' + 'a' * 1000,
            'four-backticks': '汉' * 438 + '\n````\n' + 'a' * 1000 + '\n````\nxy',
        }
        originals = {name: self.article(name, body).read_bytes() for name, body in bodies.items()}
        baseline = self.build(self.source)
        self.prepared.sync()
        rendered = self.build(self.prepared.target)
        for name in bodies:
            with self.subTest(name=name):
                self.assertEqual(rendered[name][0], baseline[name][1])
                self.assertFalse(baseline[name][2])
                self.assertTrue(rendered[name][2])
                self.assertEqual((self.source / 'content' / (name + '.md')).read_bytes(), originals[name])
        self.assertEqual(rendered['combining'][0], 2)
        self.assertGreater(rendered['math'][1], rendered['math'][0])
        self.assertFalse((self.prepared.target / 'public' / ORIGINAL_BODIES).exists())
        self.assertEqual((self.source / ORIGINAL_BODIES).read_text(), '{}')
        sources = json.loads((self.prepared.target / ORIGINAL_BODIES).read_text(encoding='utf-8'))
        self.assertEqual(sources['math.md'], bodies['math'])

    def test_private_source_data_updates_and_removes_deleted_pages(self):
        article = self.article('math', '$x$')
        self.prepared.sync()
        data = self.prepared.target / ORIGINAL_BODIES
        before = data.stat().st_mtime_ns
        self.assertEqual(self.prepared.sync(), [])
        self.assertEqual(data.stat().st_mtime_ns, before)
        self.article('math', '$updated$')
        self.assertIn(ORIGINAL_BODIES, self.prepared.sync())
        self.assertEqual(json.loads(data.read_text())['math.md'], '$updated$')
        article.unlink()
        self.assertIn(ORIGINAL_BODIES, self.prepared.sync())
        self.assertNotIn('math.md', json.loads(data.read_text()))

    def test_other_languages_keep_native_fallback(self):
        (self.source / 'config.toml').write_text(
            'base_url = "https://example.test"\ndefault_language = "en"\n', encoding='utf-8')
        self.article('math', '$x$ ' * 50)
        self.prepared.sync()
        reading, native, prepared = self.build(self.prepared.target)['math']
        self.assertEqual(reading, native)
        self.assertTrue(prepared)


if __name__ == '__main__':
    unittest.main()
