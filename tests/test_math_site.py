"""Exercise protected equations through the real Zola build and live server."""
from html.parser import HTMLParser
import http.client
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


class RenderedPage(HTMLParser):
    def __init__(self, source):
        super().__init__()
        self.formulas = []
        self.code = []
        self.in_code = False
        self.cells = 0
        self.feed(source)

    def handle_starttag(self, tag, attributes):
        attributes = dict(attributes)
        if tag == 'span' and 'data-math-tex' in attributes:
            self.formulas.append(attributes['data-math-tex'])
        if tag == 'code':
            self.in_code = True
        if tag == 'td':
            self.cells += 1

    def handle_endtag(self, tag):
        if tag == 'code':
            self.in_code = False

    def handle_data(self, value):
        if self.in_code:
            self.code.append(value)


class MathSiteTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.zola = os.environ.get('ZOLA') or shutil.which('zola')
        bundled = ROOT / '.tools/zola/zola.exe'
        if not cls.zola and bundled.is_file():
            cls.zola = str(bundled)
        if not cls.zola:
            raise unittest.SkipTest('Zola is required for math integration checks')

    def setUp(self):
        scratch = ROOT / '.tools'
        scratch.mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix='math-test-', dir=scratch)
        self.directory = Path(self.temporary.name).resolve()
        if not self.directory.is_relative_to(scratch.resolve()):
            raise RuntimeError('Test directory escaped the workspace scratch directory')
        self.addCleanup(self.temporary.cleanup)
        self.source = self.directory / 'source'
        (self.source / 'content').mkdir(parents=True)
        (self.source / 'templates').mkdir()
        (self.source / 'config.toml').write_text('base_url = "https://example.test/blog/"\n', encoding='utf-8')
        (self.source / 'content/_index.md').write_text('+++\n+++\n', encoding='utf-8')
        (self.source / 'templates/index.html').write_text('{{ section.content | safe }}', encoding='utf-8')
        (self.source / 'templates/page.html').write_text('{{ page.content | safe }}', encoding='utf-8')
        self.article = self.source / 'content/math.md'
        self.header = '+++\ntitle = "Math"\n+++\n\n'
        self.prepared = PreparedSite(self.source, self.directory / 'prepared')

    def test_real_markdown_preserves_tex_code_and_table_columns(self):
        equation = r'\forall s \in S: Q^*_{\mathcal{M}}(s, \tau_1) = Q^*_{\mathcal{M}}(s, \tau_2)'
        matrix = r'\begin{aligned}x_1 &= \{a,b\} \\' + '\n' + r'x_2 &= {{c}}\end{aligned}'
        body = ('$f(x)$\n\n$$\n' + equation + '\n$$\n\n$$\n' + matrix + '\n$$\n\n'
                '| math | label |\n| --- | --- |\n| $a|b$ | ordinary |\n\n'
                '```latex\n$$ Q^*_{x} $$\n```\n\n`$inline_code$`\n\n'
                '- > ~~~latex\n  > $nested_code$\n  > ~~~\n\n'
                r'\(a_b\)' + '\n\n' + r'\[\{x\}\]' + '\n')
        original = (self.header + body).encode()
        self.article.write_bytes(original)
        self.prepared.sync()
        subprocess.run([self.zola, '--root', str(self.prepared.target), 'build'],
                       check=True, capture_output=True)
        rendered = RenderedPage((self.prepared.target / 'public/math/index.html').read_text(encoding='utf-8'))
        self.assertEqual(rendered.formulas,
                         ['f(x)', '\n' + equation + '\n', '\n' + matrix + '\n', 'a|b', 'a_b', r'\{x\}'])
        self.assertEqual(rendered.cells, 2)
        self.assertIn('$$ Q^*_{x} $$', ''.join(rendered.code))
        self.assertIn('$inline_code$', ''.join(rendered.code))
        self.assertIn('$nested_code$', ''.join(rendered.code))
        self.assertNotIn('math-source', ''.join(rendered.code))
        self.assertEqual(self.article.read_bytes(), original)

    def test_prepared_preview_updates_and_recovers_incomplete_math(self):
        self.article.write_text(self.header + '$x_1$', encoding='utf-8')
        self.prepared.sync()
        with socket.socket() as candidate:
            candidate.bind(('127.0.0.1', 0))
            port = candidate.getsockname()[1]
        log_path = self.directory / 'zola-preview.log'
        log = log_path.open('wb')
        self.addCleanup(log.close)
        process = subprocess.Popen(
            [self.zola, '--root', str(self.prepared.target), 'serve', '--port', str(port),
             '--interface', '127.0.0.1', '--base-url', 'http://127.0.0.1', '--debounce', '1', '--force'],
            stdout=log, stderr=subprocess.STDOUT)

        def await_page(predicate):
            deadline = time.monotonic() + 15
            html = ''
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    self.fail('Zola preview stopped unexpectedly')
                connection = http.client.HTTPConnection('127.0.0.1', port, timeout=1)
                try:
                    connection.request('GET', '/math/')
                    response = connection.getresponse()
                    html = response.read().decode('utf-8')
                    if response.status == 200 and predicate(html):
                        return html
                except (OSError, http.client.HTTPException):
                    pass
                finally:
                    connection.close()
                time.sleep(.05)
            self.fail('Timed out waiting for updated math in Zola preview\n'
                      + 'Last response: ' + repr(html) + '\n'
                      + log_path.read_text(encoding='utf-8', errors='replace'))

        try:
            await_page(lambda html: RenderedPage(html).formulas == ['x_1'])
            self.article.write_text(self.header + '$unfinished', encoding='utf-8')
            self.prepared.sync()
            await_page(lambda html: '$unfinished' in html and not RenderedPage(html).formulas)
            # Write exact bytes: write_text's platform newline translation
            # otherwise turns LF into CRLF on Windows. The staging layer
            # deliberately preserves those original TeX line endings.
            for newline, variable in (('\n', 'x'), ('\r\n', 'y')):
                equation = f'Q^*_{{{variable}}} = \\{{1\\}}'
                tex = newline + equation + newline
                source = self.header.replace('\n', newline) + '$$' + tex + '$$'
                self.article.write_bytes(source.encode('utf-8'))
                self.prepared.sync()
                await_page(lambda html: RenderedPage(html).formulas == [tex])
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == '__main__':
    unittest.main()
