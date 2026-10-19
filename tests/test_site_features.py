"""A fresh site builds and validates without a personal profile or fixed article."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from build_site import build
from check_site import validate_site
from editor import trigger_content_reload


class GenericSiteTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='generic-site-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.site = self.root / 'site'
        self.site.mkdir()
        self.config(False)

    def config(self, enabled):
        self.site.joinpath('config.toml').write_text(
            'base_url = "https://example.org/research/"\n'
            f'[extra]\nprofile_enabled = {str(enabled).lower()}\nagent_endpoint = ""\n',
            encoding='utf-8')

    def write(self, relative, text):
        path = self.site / 'public' / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8')
        return path

    def output(self, *, base='https://example.org/research/', title='A completely different paper'):
        self.write('index.html', f'<a href="{base}posts/new/">Read</a>')
        self.write('posts/new/index.html', '<h1 id="result">Result</h1><a href="#result">Section</a>')
        for relative in ('404.html', 'graph/index.html', 'references/index.html'):
            self.write(relative, '<main>Research</main>')
        self.write('atom.xml', '<feed/>')
        self.write('sitemap.xml', '<urlset/>')
        self.write('search-index/index.html', json.dumps([{'title': title, 'url': base + 'posts/new/'}]))
        self.write('graph-data/index.html', json.dumps([{
            'id': 'posts/new/index.md', 'url': base + 'posts/new/', 'draft': False,
            'html': '<h1>Result</h1>', 'headings': []}]))

    def test_disabled_profile_build_does_not_invoke_resume_compiler(self):
        with patch('build_site.sync_design') as design, patch('build_site.build_profile') as profile, \
                patch('build_site.compile_figures') as figures, patch('build_site.render_site') as render:
            build(root=self.root)
        profile.assert_not_called()
        design.assert_called_once_with(root=self.root)
        figures.assert_called_once_with(self.site, root=self.root)
        render.assert_called_once_with(self.site, None, root=self.root)
        self.assertFalse((self.root / 'profile').exists())

    def test_enabled_profile_missing_source_fails_before_any_build_mutation(self):
        self.config(True)
        with patch('build_site.sync_design') as design, patch('build_site.build_profile') as profile:
            with self.assertRaisesRegex(ValueError, 'profile/cv.typ is missing'):
                build(root=self.root)
        design.assert_not_called()
        profile.assert_not_called()

    def test_generic_article_and_missing_about_profile_pass(self):
        self.output()
        result = validate_site(self.site)
        self.assertEqual(result['articles'], 1)
        self.assertEqual(result['pages'], 5)
        self.assertFalse((self.site / 'public/about').exists())
        self.assertFalse((self.site / 'data/profile.json').exists())

    def test_empty_library_is_valid(self):
        self.output()
        self.write('search-index/index.html', '[]')
        self.write('graph-data/index.html', '[]')
        self.assertEqual(validate_site(self.site)['articles'], 0)

    def test_prefix_build_can_be_checked_with_explicit_base_url(self):
        base = 'https://preview.example.org/another-prefix/'
        self.output(base=base)
        with self.assertRaisesRegex(ValueError, 'must belong to the site'):
            validate_site(self.site)
        self.assertEqual(validate_site(self.site, base_url=base)['articles'], 1)

    def test_missing_link_anchor_and_graph_draft_are_reported(self):
        self.output()
        self.write('index.html', '<a href="missing/">Gone</a><a href="posts/new/#wrong">Anchor</a>')
        graph = self.site / 'public/graph-data/index.html'
        data = json.loads(graph.read_text(encoding='utf-8'))
        data[0]['draft'] = True
        graph.write_text(json.dumps(data), encoding='utf-8')
        with self.assertRaises(ValueError) as result:
            validate_site(self.site)
        self.assertIn('missing target', str(result.exception))
        self.assertIn('missing anchor', str(result.exception))
        self.assertIn('Draft leaked', str(result.exception))

    def test_encoded_link_cannot_escape_output(self):
        self.output()
        self.write('index.html', '<a href="%2e%2e/secret.txt">Secret</a>')
        (self.site / 'secret.txt').write_text('outside', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'escapes output directory'):
            validate_site(self.site)

    def test_private_credentials_are_rejected_without_profile_enabled(self):
        self.output()
        self.write('nested/preview-editor.json', '{"token":"private"}')
        self.write('index.html', '<script id="series-editor-script"></script>')
        with self.assertRaises(ValueError) as result:
            validate_site(self.site)
        self.assertIn('Private build input leaked', str(result.exception))
        self.assertIn('local editing is enabled', str(result.exception))

    def test_enabled_profile_checks_generated_outputs(self):
        self.output()
        self.config(True)
        with self.assertRaises(ValueError) as result:
            validate_site(self.site)
        self.assertIn('generated metadata is missing/invalid', str(result.exception))
        self.assertIn('/about/ is missing', str(result.exception))
        self.assertIn('public resume is missing/invalid', str(result.exception))

    def test_load_data_refresh_touches_existing_content_without_creating_about(self):
        content = self.site / 'content'
        content.mkdir()
        index = content / '_index.md'
        index.write_text('+++\ntitle = "Library"\n+++\n', encoding='utf-8')
        original = index.read_bytes()
        trigger_content_reload(self.site)
        self.assertEqual(index.read_bytes(), original)
        self.assertFalse((content / 'about.md').exists())


if __name__ == '__main__':
    unittest.main()
