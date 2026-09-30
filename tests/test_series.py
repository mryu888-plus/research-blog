"""Series reading order and draft exclusion through the pinned Zola renderer."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class SeriesRenderingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.zola = os.environ.get('ZOLA') or shutil.which('zola')
        bundled = ROOT / '.tools/zola/zola.exe'
        if not cls.zola and bundled.is_file():
            cls.zola = str(bundled)
        if not cls.zola:
            raise unittest.SkipTest('Zola is required for series integration tests')

    def setUp(self):
        scratch = ROOT / '.tools'
        scratch.mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix='series-test-', dir=scratch)
        self.addCleanup(self.temp.cleanup)
        self.site = Path(self.temp.name)
        self.write('config.toml', 'base_url = "https://example.test/blog/"\ndefault_language = "zh"\n[extra]\n')
        self.write('content/_index.md', '+++\ntitle = "首页"\n+++\n')
        self.write('content/posts/_index.md', '+++\ntitle = "文章"\nsort_by = "date"\n+++\n')
        self.write('templates/base.html', '<!doctype html><html><head><title>{% block title %}{% endblock title %}</title></head><body><main>{% block content %}{% endblock content %}</main></body></html>')
        self.write('templates/index.html', '首页')
        self.write('templates/section.html', '文章')
        self.write('templates/page.html', '<h1>{{ page.title }}</h1>{{ page.content | safe }}{% include "article-series.html" %}')
        for template in ('series.html', 'article-series.html'):
            self.write('templates/' + template, (ROOT / 'site/templates' / template).read_text(encoding='utf-8'))
        self.write('content/series.md', (ROOT / 'site/content/series.md').read_text(encoding='utf-8'))
        for slug, title, draft in [('a', '第一篇', False), ('b', '第二篇', False), ('draft', '不可泄露的草稿标题', True), ('c', '另一组文章', False)]:
            self.write(f'content/posts/{slug}/index.md', f'+++\ntitle = "{title}"\ndate = 2026-09-28\ndescription = "摘要"\ndraft = {str(draft).lower()}\n+++\n正文\n')
        self.catalog = {'version': 1, 'series': [{'id': 's-test', 'title': '研究 <script>系列', 'description': '按组阅读', 'groups': [
            {'id': 'g-first', 'title': '基础', 'description': '', 'articles': ['posts/a/index.md', 'posts/draft/index.md', 'posts/missing/index.md', 'posts/b/index.md']},
            {'id': 'g-second', 'title': '拓展', 'description': '', 'articles': ['posts/c/index.md']},
            {'id': 'g-empty', 'title': '尚未开始', 'description': '', 'articles': []},
        ]}]}
        self.write('data/collections.json', json.dumps(self.catalog, ensure_ascii=False))

    def write(self, relative, text):
        file = self.site / relative
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(text, encoding='utf-8')

    def build(self, drafts=False):
        command = [self.zola, '--root', str(self.site), 'build']
        if drafts:
            command.append('--drafts')
        result = subprocess.run(command, text=True, capture_output=True, encoding='utf-8')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return self.site / 'public'

    def test_public_series_order_counts_escaping_and_previous_next_skip_drafts(self):
        output = self.build()
        index = (output / 'series/index.html').read_text(encoding='utf-8')
        self.assertLess(index.index('第一篇'), index.index('第二篇'))
        self.assertNotIn('不可泄露', index)
        self.assertNotIn('/draft/', index)
        self.assertNotIn('/missing/', index)
        self.assertIn('2 篇', index)
        self.assertIn('1 篇', index)
        self.assertIn('0 篇', index)
        self.assertNotIn('<script>', index)
        self.assertIn('&lt;script&gt;', index)
        first = (output / 'posts/a/index.html').read_text(encoding='utf-8')
        second = (output / 'posts/b/index.html').read_text(encoding='utf-8')
        self.assertIn('rel="next" href="https://example.test/blog/posts/b/"', first)
        self.assertIn('rel="prev" href="https://example.test/blog/posts/a/"', second)
        self.assertNotIn('另一组文章', first)
        self.assertIn('aria-current="page">第一篇', first)

    def test_draft_preview_keeps_draft_in_reading_order(self):
        output = self.build(drafts=True)
        index = (output / 'series/index.html').read_text(encoding='utf-8')
        self.assertIn('不可泄露的草稿标题', index)
        self.assertIn('series-draft', index)
        self.assertIn('3 篇', index)
        first = (output / 'posts/a/index.html').read_text(encoding='utf-8')
        self.assertIn('rel="next" href="https://example.test/blog/posts/draft/"', first)

    def test_empty_or_absent_catalog_has_a_readable_fallback(self):
        self.write('data/collections.json', '{"version":1,"series":[]}')
        output = self.build()
        self.assertIn('系列正在整理中', (output / 'series/index.html').read_text(encoding='utf-8'))
        (self.site / 'data/collections.json').unlink()
        self.build()

    def test_direct_series_articles_render_without_group_and_skip_drafts_in_navigation(self):
        series = self.catalog['series'][0]
        series['articles'] = series['groups'][0]['articles']
        series['groups'] = [series['groups'][1]]
        self.write('data/collections.json', json.dumps(self.catalog, ensure_ascii=False))
        output = self.build()
        index = (output / 'series/index.html').read_text(encoding='utf-8')
        self.assertLess(index.index('第一篇'), index.index('第二篇'))
        self.assertNotIn('不可泄露', index)
        first = (output / 'posts/a/index.html').read_text(encoding='utf-8')
        self.assertIn('本系列目录 · 2 篇', first)
        self.assertNotIn('本组目录', first)
        self.assertIn('rel="next" href="https://example.test/blog/posts/b/"', first)
        self.assertNotIn('另一组文章', first)
        series['groups'] = []
        self.write('data/collections.json', json.dumps(self.catalog, ensure_ascii=False))
        output = self.build(drafts=True)
        first = (output / 'posts/a/index.html').read_text(encoding='utf-8')
        self.assertIn('本系列目录 · 3 篇', first)
        self.assertIn('rel="next" href="https://example.test/blog/posts/draft/"', first)


if __name__ == '__main__':
    unittest.main()
