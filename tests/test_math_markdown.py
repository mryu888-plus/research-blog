"""Regressions for TeX that Markdown would otherwise rewrite before KaTeX."""
from html.parser import HTMLParser
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from math_markdown import protect_math


class MathNodes(HTMLParser):
    def __init__(self, html):
        super().__init__()
        self.nodes = []
        self.feed(html)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'span' and attrs.get('class') == 'math-source':
            self.nodes.append((attrs['data-math-tex'], attrs['data-math-display'] == 'true'))


def nodes(source):
    return MathNodes(protect_math(source)).nodes


class MathMarkdownTests(unittest.TestCase):
    def test_user_equation_needs_no_html_or_markdown_escapes(self):
        tex = r'\forall s \in S: Q^*_{\mathcal{M}}(s, \tau_1) = Q^*_{\mathcal{M}}(s, \tau_2)'
        self.assertEqual(nodes('$$\n' + tex + '\n$$'), [('\n' + tex + '\n', True)])
        self.assertEqual(nodes('$f(x)$ and $x_i^2$'), [('f(x)', False), ('x_i^2', False)])

    def test_matrix_newlines_braces_and_comparison_are_preserved(self):
        tex = '\n' + r'\begin{aligned}x_1 &= \{a, b\} \\' + '\n' + r'x_2 &< {{c}} \end{aligned}' + '\n'
        protected = protect_math('$$' + tex + '$$')
        self.assertEqual(MathNodes(protected).nodes, [(tex, True)])
        self.assertNotIn('{{c}}', protected)
        self.assertNotIn('x_1', protected)
        self.assertNotIn('\\', protected)

    def test_bracket_delimiters_and_backslash_parity(self):
        self.assertEqual(nodes(r'\(a_b\) and \[\{x\mid x>0\}\]'),
                         [('a_b', False), (r'\{x\mid x>0\}', True)])
        self.assertEqual(nodes(r'\$5 and $x+\$2$'), [(r'x+\$2', False)])
        self.assertEqual(nodes(r'\\$x$'), [('x', False)])

    def test_currency_unclosed_math_and_whitespace_are_not_consumed(self):
        for source in ('$5 and $10, then $20', r'\$100', '$ unfinished', '$x', '$ x $', '$x $', '$x\ny$'):
            with self.subTest(source=source):
                self.assertEqual(protect_math(source), source)

    def test_frontmatter_and_code_examples_remain_unchanged(self):
        metadata = '\ufeff+++\r\ntitle = "$literal$"\r\n+++\r\n'
        body = '```latex\n$$ x_* $$\n```\n\n~~~text\n$x$\n~~~\n\n    $code$\n    $$ more $$\n\n`$x$` and `` `$y$` ``\n'
        self.assertEqual(protect_math(metadata + body), metadata + body)
        self.assertEqual(protect_math('+++\ntitle = "$x$"'), '+++\ntitle = "$x$"')

    def test_nested_fenced_code_is_not_math(self):
        for source in ('> ```latex\n> $x$\n> ```\n',
                       '- ```latex\n  $x$\n  ```\n',
                       '````\n```\n$x$\n```\n````\n'):
            self.assertEqual(protect_math(source), source)

    def test_code_in_mixed_quote_and_list_containers_is_unchanged(self):
        for source in (
            '- > ~~~latex\n  > $literal$\n  > ~~~\n',
            '> - ~~~latex\n>   $literal$\n>   ~~~\n',
            '- > ```latex\n  > $literal$\n  > ```\n',
            '> - ```latex\n>   $literal$\n>   ```\n',
            '- item\n\n  >     $literal$\n',
            '> - item\n>\n>       $literal$\n',
            '100. > ~~~latex\n     > $literal$\n     > ~~~\n',
            '- > - ~~~latex\n  >   $literal$\n  >   ~~~\n',
            '-     $literal$\n',
        ):
            with self.subTest(source=source):
                self.assertEqual(protect_math(source), source)

    def test_unclosed_fence_ends_with_its_container(self):
        for source in (
            '> ```latex\n> $literal$\n\n$real_math$\n',
            '- ~~~latex\n  $literal$\n\n$real_math$\n',
            '- ~~~latex\n  $literal$\n- $real_math$\n',
            '- > ~~~latex\n  > $literal$\n\n  $real_math$\n',
            '> - ~~~latex\n>   $literal$\n>\n> $real_math$\n',
        ):
            with self.subTest(source=source):
                self.assertEqual(nodes(source), [('real_math', False)])

    def test_fenced_code_does_not_open_nested_containers(self):
        for source in (
            '~~~\n> ~~~\n$literal$\n~~~\n',
            '> ~~~\n> - ~~~\n> $literal$\n> ~~~\n',
            '- ~~~\n  > ~~~\n  $literal$\n  ~~~\n',
        ):
            with self.subTest(source=source):
                self.assertEqual(protect_math(source), source)

    def test_display_math_in_mixed_containers_has_no_quote_markers(self):
        for source in (
            '- > $$\n  > x_*\n  > $$\n',
            '100. > $$\n     > x_*\n     > $$\n',
            '> - > $$\n>   > x_*\n>   > $$\n',
        ):
            with self.subTest(source=source):
                self.assertEqual(nodes(source), [('\nx_*\n', True)])

    def test_quote_and_list_math_keep_their_container(self):
        source = '> $$\n> x_* + y\n> $$\n\n- $a_b$\n\n  $$\n  x + y\n  $$\n'
        found = nodes(source)
        self.assertEqual(found, [('\nx_* + y\n', True), ('a_b', False), ('\n  x + y\n  ', True)])
        self.assertTrue(protect_math(source).startswith('> <span'))

    def test_tags_comments_code_and_urls_are_not_rewritten(self):
        source = ('<!-- $comment$ -->\n<pre>$code$</pre>\n<script>let x="$js$";</script>\n'
                  '<span title="$title$">word</span>\n![price $10$](image.png)\n'
                  '[link](https://example.test/$path$)\n<https://example.test/$path$>\n'
                  '[ref]: https://example.test/$path$\n{{ component(caption="$title$") }}')
        self.assertEqual(protect_math(source), source)

    def test_table_math_cannot_create_extra_columns(self):
        protected = protect_math('| $a|b$ | $c$ |')
        self.assertEqual(protected.count('|'), 3)
        self.assertEqual(MathNodes(protected).nodes, [('a|b', False), ('c', False)])

    def test_html_legacy_wrapper_works_and_transformation_is_idempotent(self):
        source = '<div class="math-block">\n$$\nQ^*_{x} = \\{1\\}\n$$\n</div>'
        protected = protect_math(source)
        self.assertEqual(MathNodes(protected).nodes, [('\nQ^*_{x} = \\{1\\}\n', True)])
        self.assertEqual(protect_math(protected), protected)

    def test_formula_cannot_inject_html_or_template_syntax(self):
        tex = r'\text{"</span><script>alert(1)</script>"} + {{x}} | {% y %}'
        protected = protect_math('$' + tex + '$')
        self.assertEqual(MathNodes(protected).nodes, [(tex, False)])
        self.assertNotIn('<script>', protected)
        self.assertNotIn('{{', protected)
        self.assertNotIn('{%', protected)


if __name__ == '__main__':
    unittest.main()
