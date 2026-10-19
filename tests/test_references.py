"""Check source-preserving citation publication and the actual Zola renderer."""
from html.parser import HTMLParser
import http.client
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from prepare_site import PreparedSite
from references import (INDEX_PAGE, SNAPSHOT, _anchor, format_item, read_bibtex,
                        transform_citations, validate_snapshot)


def item(key='a', title='中文文献', **fields):
    return {'id': 'zotero-' + key, 'citekey': key, 'title': title,
            'author': [{'family': '陈', 'given': '明'}],
            'issued': {'date-parts': [[2026]]}, 'type': 'article-journal', **fields}


class RenderedPage(HTMLParser):
    def __init__(self, text):
        super().__init__()
        self.links, self.ids, self.math, self.codes = [], set(), [], []
        self.in_code = False
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if attrs.get('id'):
            self.ids.add(attrs['id'])
        if tag == 'a':
            self.links.append(attrs.get('href', ''))
        if attrs.get('data-math-tex'):
            self.math.append(attrs['data-math-tex'])
        if tag == 'code':
            self.in_code = True

    def handle_endtag(self, tag):
        if tag == 'code':
            self.in_code = False

    def handle_data(self, value):
        if self.in_code:
            self.codes.append(value)


class ReferenceUnitTests(unittest.TestCase):
    def test_reference_search_matches_chinese_and_multiple_terms_without_html_insertion(self):
        node = shutil.which('node')
        if not node:
            self.skipTest('Node is required for the small bibliography search check')
        script = r'''
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const rows = [{textContent:'中文论文 陈明 文章示例', hidden:false}, {textContent:'English Alice Tutorial', hidden:false}];
const status = {textContent:''};
const input = {value:'', addEventListener(name, callback) { this.change = callback; }};
vm.runInNewContext(fs.readFileSync(process.argv[1], 'utf8'), {
  document: {
    querySelector(selector) { return selector === '[data-reference-search]' ? input : status; },
    querySelectorAll() { return rows; }
  }
});
assert.equal(status.textContent, '共 2 篇文献');
input.value = '中文 陈明'; input.change();
assert.deepEqual(rows.map(row => row.hidden), [false, true]);
assert.equal(status.textContent, '找到 1 / 2 篇文献');
input.value = '<img src=x onerror=alert(1)>'; input.change();
assert.deepEqual(rows.map(row => row.hidden), [true, true]);
input.value = 'Ａｌｉｃｅ'; input.change();
assert.deepEqual(rows.map(row => row.hidden), [true, false]);
input.value = ''; input.change();
assert.deepEqual(rows.map(row => row.hidden), [false, false]);
'''
        result = subprocess.run([node, '-e', script, str(ROOT / 'site/static/references-page.js')], capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr.decode('utf-8', errors='replace'))

    def test_multiple_keys_locator_and_repeat_numbering(self):
        source = '中文正文 [@b; @a, p. 3]，再引 [@b]。'
        rendered, keys = transform_citations(source, {'a': item(), 'b': item('b', 'Second')})
        self.assertEqual(keys, ['b', 'a'])
        self.assertIn('>1</a>; ', rendered)
        self.assertIn('>2, p. 3</a>', rendered)
        self.assertEqual(rendered.count('id="' + _anchor('b') + '"'), 1)
        self.assertIn('↩1', rendered)
        self.assertIn('↩2', rendered)
        self.assertIn('陈', rendered)

    def test_code_math_metadata_links_and_email_are_preserved(self):
        source = ('+++\ntitle = "[@missing]"\n+++\n\n'
                  '`[@missing]`\n\n```md\n[@missing]\n```\n\n'
                  '    [@missing]\n\n'
                  '- > ~~~\n  > [@missing]\n  > ~~~\n\n'
                  '$x=[@missing]$\n\n$$\n[@missing]\n$$\n\n'
                  r'\([@missing]\) \[ [@missing] \]' + '\n\n'
                  '<code>[@missing]</code> <!-- [@missing] -->\n'
                  '<a title="[@missing]">普通</a>\n'
                  r'\[@missing] ![@missing](test.png) [@missing](https://example.org)' + '\n'
                  '<reader@example.org> reader@example.org [@reader@example.org]\n'
                  '[link]: https://example.org/[@missing]\n\n[@a]\n')
        rendered, keys = transform_citations(source, {'a': item()})
        self.assertEqual(keys, ['a'])
        before = source[:source.rfind('[@a]')]
        self.assertTrue(rendered.startswith(before))

    def test_missing_key_reports_source_and_line(self):
        with self.assertRaisesRegex(ValueError, r'paper.md:3: missing reference @absent'):
            transform_citations('first\n\n[@absent]', {}, 'paper.md')

    def test_html_is_escaped_and_only_safe_links_render(self):
        entry = item(title='<script>alert(1)</script> {{ danger }} $x$',
                     URL='javascript:alert(1)', DOI='10.1234/paper<x>',
                     publisher='<img src=x onerror=alert(1)>')
        rendered = format_item(entry)
        self.assertNotIn('<script>', rendered)
        self.assertNotIn('<img', rendered)
        self.assertNotIn('javascript:', rendered)
        self.assertNotIn('{{', rendered)
        self.assertNotIn('$x$', rendered)
        self.assertIn('https://doi.org/10.1234/paper%3Cx%3E', rendered)
        for url in ('file:///C:/private/paper.pdf', 'https://secret:token@example.org/', 'http://localhost/private', '//example.org'):
            self.assertNotIn('href=', format_item(item(URL=url)))

    def test_snapshot_rejects_private_fields_at_every_supported_level(self):
        for field in ('pdfPath', 'annotations', 'apiKey', 'attachments', 'abstract', 'note'):
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'forbidden fields'):
                validate_snapshot({'version': 1, 'items': [item(**{field: 'PRIVATE-SENTINEL'})]})
        with self.assertRaises(ValueError):
            validate_snapshot({'version': 1, 'items': [item(author=[{'family': 'Name', 'annotation': 'secret'}])]})
        with self.assertRaises(ValueError):
            validate_snapshot({'version': 1, 'items': [item(issued={'date-parts': [[2026]], 'private': 'secret'})]})
        with self.assertRaises(ValueError):
            validate_snapshot({'version': 1, 'items': [item(), item()]})
        with self.assertRaises(ValueError):
            validate_snapshot({'version': 1, 'items': [], 'apiKey': 'secret'})

    def test_bibtex_standard_values_crossref_names_and_private_field_exclusion(self):
        source = r'''@string{venue = "Journal " # "of Testing"}
@book{parent, title={Proceedings}, publisher={Test Press}, year=2024}
@article{paper,
 title={A {Nested} Title with G\"{o}del},
 author={Smith, Alice and {研究实验室} and Ludwig van Beethoven},
 journal=venue, crossref={parent}, volume=12, number={2}, pages={1--10},
 doi={10.1000/demo}, url={https://example.org/paper},
 file={PRIVATE-PDF-PATH}, note={PRIVATE-ANNOTATION}}
'''
        entry = read_bibtex(source)['paper']
        self.assertEqual(entry['title'], 'A Nested Title with Gödel')
        self.assertEqual(entry['container-title'], 'Journal of Testing')
        self.assertEqual(entry['publisher'], 'Test Press')
        self.assertEqual(entry['issued']['date-parts'], [[2024]])
        self.assertEqual(entry['author'], [{'family': 'Smith', 'given': 'Alice'},
                                           {'literal': '研究实验室'},
                                           {'family': 'van Beethoven', 'given': 'Ludwig'}])
        self.assertNotIn('PRIVATE', json.dumps(entry))
        existing = read_bibtex((ROOT / 'site/content/posts/reusable-skills/refs.bib').read_text(encoding='utf-8'))
        self.assertEqual(existing['smith2023']['page'], '1--28')
        self.assertEqual(existing['levine2018']['issued']['date-parts'], [[2018]])

    def test_unsupported_or_invalid_bibtex_fails_explicitly(self):
        for source in ('@article{a,title=unknown}', '@article{a,title={unclosed}',
                       '@article{a,title={A},crossref={a}}', r'@article{a,title={\unknown{X}}}'):
            with self.subTest(source=source), self.assertRaises(ValueError):
                read_bibtex(source)


class ReferenceStagingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        bundled = ROOT / '.tools/zola/zola.exe'
        cls.zola = os.environ.get('ZOLA') or shutil.which('zola') or (str(bundled) if bundled.exists() else None)

    def setUp(self):
        scratch = ROOT / '.tools'
        scratch.mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix='references-test-', dir=scratch)
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.source = self.directory / 'source'
        self.source.mkdir()
        self.write('config.toml', 'base_url = "https://example.test/blog/"\n')
        self.write('content/_index.md', '+++\n+++\n')
        self.write('templates/index.html', '{{ section.content | safe }}')
        self.write('templates/page.html', '{{ page.content | safe }}')
        self.write('templates/components.html', (ROOT / 'site/templates/components.html').read_text(encoding='utf-8'))
        self.prepared = PreparedSite(self.source, self.directory / 'prepared')

    def write(self, name, content):
        target = self.source / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content.encode('utf-8'))
        return target

    def snapshot(self, *items):
        return self.write(str(SNAPSHOT), json.dumps({'version': 1, 'items': items}, ensure_ascii=False))

    def article(self, name='paper', body='[@a]', draft=False, extra=''):
        return self.write(f'content/{name}.md', f'+++\ntitle = "文章 {name}"\ndraft = {str(draft).lower()}\n{extra}+++\n\n{body}')

    def build(self):
        if not self.zola:
            self.skipTest('Zola is required for bibliography rendering checks')
        self.prepared.sync()
        result = subprocess.run([self.zola, '--root', str(self.prepared.target), 'build', '--force'],
                                capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr.decode('utf-8', errors='replace'))
        return self.prepared.target / 'public'

    def test_real_render_chinese_multi_key_math_code_and_safe_html(self):
        self.snapshot(item('a', '中文 <em>文献</em>', DOI='10.1234/test'), item('b', 'Second'))
        article = self.article(body='正文 [@a, p. 3; @b]\n\n`[@missing]`\n\n$[@missing]$\n\n```\n[@missing]\n```')
        original = article.read_bytes()
        public = self.build()
        rendered_text = (public / 'paper/index.html').read_text(encoding='utf-8')
        rendered = RenderedPage(rendered_text)
        self.assertIn('#' + _anchor('a'), rendered.links)
        self.assertIn(_anchor('a'), rendered.ids)
        self.assertIn('https://doi.org/10.1234/test', rendered.links)
        self.assertIn('1, p. 3', rendered_text)
        self.assertIn('中文 &lt;em&gt;文献&lt;/em&gt;', rendered_text)
        self.assertIn('[@missing]', rendered.codes)
        self.assertEqual(rendered.math, ['[@missing]'])
        self.assertEqual(article.read_bytes(), original)
        index = (public / 'references/index.html').read_text(encoding='utf-8')
        self.assertIn('https://example.test/blog/paper/#' + _anchor('a'), index)
        self.assertIn('文章 paper', index)

    def test_only_published_references_and_backlinks_reach_index_or_staged_data(self):
        self.snapshot(item('a', 'PUBLIC-PAPER'), item('b', 'DRAFT-ONLY'), item('c', 'UNCITED-PAPER'))
        self.article()
        self.article('draft', '[@b]', draft=True)
        self.write('content/private/_index.md', '+++\ndraft = true\n+++\n')
        self.article('private/child', '[@c]')
        public = self.build()
        self.assertFalse((public / 'draft').exists())
        self.assertFalse((public / 'private/child').exists())
        index = (public / 'references/index.html').read_text(encoding='utf-8')
        self.assertIn('PUBLIC-PAPER', index)
        for private in ('DRAFT-ONLY', 'UNCITED-PAPER', '文章 draft', '文章 private/child'):
            self.assertNotIn(private, index)
        staged = json.loads((self.prepared.target / SNAPSHOT).read_text(encoding='utf-8'))
        self.assertEqual([entry['citekey'] for entry in staged['items']], ['a'])

    def test_bibtex_fallback_never_copies_private_original_to_public(self):
        self.article('paper/index', '[@smith]')
        bib = self.write('content/paper/refs.bib', '@article{smith,title={Example},author={Smith, A},year=2020,file={PRIVATE-PDF},note={PRIVATE-NOTE}}')
        original = bib.read_bytes()
        public = self.build()
        self.assertIn('Example', (public / 'paper/index.html').read_text(encoding='utf-8'))
        self.assertFalse((public / 'paper/refs.bib').exists())
        self.assertFalse((self.prepared.target / 'content/paper/refs.bib').exists())
        self.assertEqual(bib.read_bytes(), original)
        for path in public.rglob('*'):
            if path.is_file():
                self.assertNotIn(b'PRIVATE-', path.read_bytes())

    def test_snapshot_wins_and_ambiguous_local_key_fails(self):
        self.snapshot(item('a', 'Zotero source'))
        self.article('paper/index')
        self.write('content/paper/refs.bib', '@article{a,title={Local fallback}}')
        self.prepared.sync()
        rendered = (self.prepared.target / 'content/paper/index.md').read_text(encoding='utf-8')
        self.assertIn('Zotero source', rendered)
        self.assertNotIn('Local fallback', rendered)
        (self.source / SNAPSHOT).unlink()
        self.article('other/index')
        self.write('content/other/refs.bib', '@article{a,title={Different local paper}}')
        with self.assertRaisesRegex(ValueError, 'conflicting public reference'):
            self.prepared.sync()

    def test_privacy_or_missing_reference_failure_changes_no_staging_files(self):
        self.snapshot(item())
        article = self.article()
        self.prepared.sync()
        before = {path: path.read_bytes() for path in self.prepared.target.rglob('*') if path.is_file()}
        self.snapshot(item(apiKey='PRIVATE-TOKEN'))
        with self.assertRaisesRegex(ValueError, 'forbidden fields'):
            self.prepared.sync()
        self.assertEqual(before, {path: path.read_bytes() for path in before})
        self.snapshot(item())
        article.write_text('+++\ntitle = "Paper"\n+++\n[@missing]', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'missing reference'):
            self.prepared.sync()
        self.assertEqual(before, {path: path.read_bytes() for path in before})

    def test_snapshot_edits_rewrite_dependent_articles_and_removal_clears_exports(self):
        snapshot = self.snapshot(item(title='Before'))
        article = self.article()
        self.prepared.sync()
        self.assertEqual(self.prepared.sync(), [])
        self.snapshot(item(title='After'))
        changed = self.prepared.sync()
        self.assertIn(Path('content/paper.md'), changed)
        self.assertIn(INDEX_PAGE, changed)
        self.assertIn('After', (self.prepared.target / 'content/paper.md').read_text(encoding='utf-8'))
        article.unlink()
        self.prepared.sync()
        self.assertNotIn('After', (self.prepared.target / INDEX_PAGE).read_text(encoding='utf-8'))
        self.assertEqual(json.loads((self.prepared.target / SNAPSHOT).read_text(encoding='utf-8'))['items'], [])
        snapshot.unlink()
        changed = self.prepared.sync()
        self.assertIn(INDEX_PAGE, changed)
        self.assertIn(SNAPSHOT, changed)
        self.assertFalse((self.prepared.target / INDEX_PAGE).exists())
        self.assertFalse((self.prepared.target / SNAPSHOT).exists())

    def test_slug_and_path_backlinks_are_encoded_and_base_path_relative(self):
        self.snapshot(item())
        self.article('folder/paper', extra='path = "研究/文献示例"\n')
        public = self.build()
        index = (public / 'references/index.html').read_text(encoding='utf-8')
        self.assertIn('https://example.test/blog/研究/文献示例/#', index)

    def test_zola_resolves_unicode_and_space_routes_for_global_backlinks(self):
        self.snapshot(item())
        self.article('My Paper')
        self.article('研究 论文')
        self.article('Override Slug', extra='slug = "custom-note"\n')
        public = self.build()
        index = RenderedPage((public / 'references/index.html').read_text(encoding='utf-8'))
        for route in ('my-paper', 'yan-jiu-lun-wen', 'custom-note'):
            self.assertTrue((public / route / 'index.html').exists())
            self.assertIn('https://example.test/blog/' + route + '/#' + _anchor('a'), index.links)

    def test_live_preview_updates_when_only_reference_snapshot_changes(self):
        if not self.zola:
            self.skipTest('Zola is required for live preview checks')
        self.snapshot(item(title='Before update'))
        self.article()
        self.prepared.sync()
        with socket.socket() as candidate:
            candidate.bind(('127.0.0.1', 0))
            port = candidate.getsockname()[1]
        log_path = self.directory / 'preview.log'
        log = log_path.open('wb')
        self.addCleanup(log.close)
        process = subprocess.Popen([self.zola, '--root', str(self.prepared.target), 'serve', '--port', str(port),
                                    '--interface', '127.0.0.1', '--base-url', 'http://127.0.0.1', '--debounce', '1', '--force'],
                                   stdout=log, stderr=subprocess.STDOUT)

        def await_page(path, expected):
            deadline = time.monotonic() + 20
            latest = ''
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    self.fail('Zola preview exited: ' + log_path.read_text(encoding='utf-8', errors='replace'))
                connection = http.client.HTTPConnection('127.0.0.1', port, timeout=1)
                try:
                    connection.request('GET', path)
                    response = connection.getresponse()
                    latest = response.read().decode('utf-8')
                    if response.status == 200 and expected in latest:
                        return latest
                except (OSError, http.client.HTTPException):
                    pass
                finally:
                    connection.close()
                time.sleep(.05)
            self.fail(f'Preview did not update {path}: {latest}\n' + log_path.read_text(encoding='utf-8', errors='replace'))

        try:
            await_page('/paper/', 'Before update')
            self.snapshot(item(title='After update'))
            self.prepared.sync()
            await_page('/paper/', 'After update')
            await_page('/references/', 'After update')
            self.article(body='No citations now.')
            self.prepared.sync()
            self.assertNotIn('After update', await_page('/paper/', 'No citations now.'))
            self.assertNotIn('After update', await_page('/references/', '已发表文章中的文献引用'))
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == '__main__':
    unittest.main()
