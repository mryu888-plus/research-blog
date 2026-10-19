"""Independent starter creation must not copy or overwrite private state."""
import importlib.util
import json
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import tomllib
from types import SimpleNamespace
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('init_site', ROOT / 'scripts/init_site.py')
init_site = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(init_site)


class InitSiteTests(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / '.tools'
        scratch.mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix='starter-test-', dir=scratch)
        self.folder = Path(self.temporary.name)
        self.destination = self.folder / 'workbench'

    def tearDown(self):
        self.temporary.cleanup()

    def initialize(self, **overrides):
        options = {'name': 'My Research Notes', 'url': 'https://example.org/notes/'}
        options.update(overrides)
        return init_site.initialize(self.destination, **options)

    def test_complete_starter_contains_shared_sources_and_only_anonymous_content(self):
        result = self.initialize(author='An Author')
        config = tomllib.loads((self.destination / 'site/config.toml').read_text(encoding='utf-8'))
        self.assertEqual(config['title'], 'My Research Notes')
        self.assertEqual(config['extra']['author'], 'An Author')
        self.assertFalse(config['extra']['profile_enabled'])
        self.assertEqual(config['base_url'], 'https://example.org/notes/')
        self.assertGreater(result['files'], 60)
        self.assertEqual(len(list(self.destination.rglob('ZEN.md'))), 1)
        for relative in ('scripts/editor.py', 'scripts/zotero.py', 'scripts/sync_zen.cjs',
                         'scripts/setup_tools.py', 'AGENTS.md', 'CLAUDE.md',
                         'extensions/research-blog/src/extension.cjs',
                         'extensions/research-notes-theme/scripts/build.cjs',
                         'Cargo.lock', 'site/content/posts/welcome/figures/style-example.typ'):
            self.assertTrue((self.destination / relative).is_file(), relative)
        for forbidden in ('profile', '.git', '.tools', 'output', 'target', 'node_modules',
                          'site/public', 'site/data/profile.json', 'site/static/resume.pdf',
                          '.env', 'site/content/posts/reusable-skills'):
            self.assertFalse((self.destination / forbidden).exists(), forbidden)
        self.assertFalse(list(self.destination.rglob('*.vsix')))
        self.assertFalse(list(self.destination.rglob('*.pyc')))
        self.assertFalse((self.destination / 'extensions/research-blog/assets/editorial-reference.png').exists())
        references = json.loads((self.destination / 'site/data/references.json').read_text())
        self.assertEqual(references['items'][0]['citekey'], 'zotero-guide')
        about = (self.destination / 'site/content/about.md').read_text(encoding='utf-8')
        self.assertNotIn('template = "about.html"', about)

    def test_dry_run_validates_without_creating_destination(self):
        result = self.initialize(dry_run=True, url='https://example.org')
        self.assertEqual(result['url'], 'https://example.org/')
        self.assertTrue(result['dry_run'])
        self.assertFalse(self.destination.exists())

    def test_generated_python_entrypoints_import_without_checkout_dependencies(self):
        self.initialize()
        command = (
            "import sys; sys.path.insert(0, 'scripts'); "
            "import build_site, build_profile, editor, check_site, prepare_site, "
            "references, math_markdown, site_features, zen, zotero, init_site, setup_tools"
        )
        result = subprocess.run([sys.executable, '-c', command],
                                cwd=self.destination, capture_output=True,
                                text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.destination / 'site/data/profile.json').exists())
        self.assertFalse((self.destination / 'site/public').exists())

    def test_generated_pages_workflow_uses_only_copied_build_entrypoints(self):
        self.initialize()
        workflow = (self.destination / '.github/workflows/pages.yml').read_text(encoding='utf-8')
        for tool in ('v0.23.4', 'v0.15.1', "python-version: '3.13'", "node-version: '22'",
                     'fonts-noto-cjk', 'actions/upload-pages-artifact@v3',
                     'actions/deploy-pages@v4', "if: github.event_name != 'pull_request'"):
            self.assertIn(tool, workflow)
        for entrypoint in ('scripts/build_site.py', 'scripts/check_site.py'):
            self.assertIn(f'run: python {entrypoint}', workflow)
            self.assertTrue((self.destination / entrypoint).is_file())
        self.assertNotIn('check_starter', workflow)
        self.assertNotIn('tests/', workflow)
        self.assertNotIn('pytest', workflow)

    def test_special_pages_are_not_filtered_by_article_date_sorting(self):
        self.initialize()
        content = self.destination / 'site/content'

        def metadata(relative):
            source = (content / relative).read_text(encoding='utf-8')
            return tomllib.loads(source.split('+++', 2)[1])

        root_section = metadata('_index.md')
        self.assertEqual(root_section['sort_by'], 'none',
                         'A date-sorted root ignores undated route pages in Zola')
        self.assertEqual(metadata('posts/_index.md')['sort_by'], 'date')
        required_routes = {'about.md': 'about', 'graph.md': 'graph',
                           'graph-data.md': 'graph-data', 'search.md': 'search-index',
                           'series.md': 'series', 'references.md': 'references'}
        for relative, expected in required_routes.items():
            page = metadata(relative)
            self.assertNotIn('date', page)
            self.assertEqual(page.get('path', Path(relative).stem), expected)
        for relative in ('search.md', 'graph-data.md'):
            page = metadata(relative)
            self.assertFalse(page['in_search_index'])
            self.assertFalse(page['include_in_feeds'])

    def test_existing_empty_directory_can_be_initialized(self):
        self.destination.mkdir()
        self.initialize()
        self.assertTrue((self.destination / 'site/config.toml').is_file())

    def test_existing_data_is_preserved(self):
        self.destination.mkdir()
        existing = self.destination / 'notes.txt'
        existing.write_text('Keep this', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, '不会覆盖'):
            self.initialize()
        self.assertEqual(existing.read_text(encoding='utf-8'), 'Keep this')
        self.assertEqual(list(self.destination.iterdir()), [existing])

    def test_existing_file_is_not_replaced(self):
        self.destination.write_text('Keep file', encoding='utf-8')
        with self.assertRaises(ValueError):
            self.initialize()
        self.assertEqual(self.destination.read_text(encoding='utf-8'), 'Keep file')

    def test_source_and_ancestor_are_rejected(self):
        for destination in (ROOT, ROOT.parent):
            with self.subTest(destination=destination), self.assertRaisesRegex(ValueError, '祖先'):
                init_site.initialize(destination, name='Example', url='https://example.org/')

    def test_invalid_urls_are_rejected_without_writes(self):
        for url in ('javascript:alert(1)', 'file:///tmp/site', 'https://user:pass@example.org/',
                    'https://example.org/?token=secret', 'https://example.org/#fragment',
                    'https://example.org\\path', 'https://example.org/../private/',
                    'https://example.org/%2e%2e/private/',
                    'https://example.org/%252e%252e/private/',
                    'https://example.org/%00', 'https://example.org:99999/',
                    'https://example .org/', 'https://-example.org/', 'https://example.org/\n'):
            with self.subTest(url=url), self.assertRaises(ValueError):
                self.initialize(url=url)
        self.assertFalse(self.destination.exists())

    def test_safe_urls_include_local_and_github_pages_roots(self):
        for value, expected in (
            ('http://127.0.0.1:1111', 'http://127.0.0.1:1111/'),
            ('http://[::1]:1111/notes', 'http://[::1]:1111/notes/'),
            ('https://USERNAME.github.io/REPO', 'https://USERNAME.github.io/REPO/'),
        ):
            self.assertEqual(init_site.normalize_url(value), expected)

    def test_toml_values_are_escaped_without_reinterpolation(self):
        name = 'Notes "quoted" __SITE_URL__ \\ path'
        author = 'Name __SITE_TITLE__'
        self.initialize(name=name, author=author)
        config = tomllib.loads((self.destination / 'site/config.toml').read_text(encoding='utf-8'))
        self.assertEqual(config['title'], name)
        self.assertEqual(config['extra']['author'], author)

    def test_missing_source_is_detected_before_destination_creation(self):
        empty = self.folder / 'source'
        empty.mkdir()
        with self.assertRaisesRegex(ValueError, '源文件'):
            self.initialize(source=empty)
        self.assertFalse(self.destination.exists())

    def test_windows_junction_attribute_is_detected(self):
        info = SimpleNamespace(st_mode=stat.S_IFDIR, st_file_attributes=0x400)
        with patch.object(Path, 'lstat', return_value=info):
            self.assertTrue(init_site._linked(self.destination))

    def test_linked_destination_parent_is_rejected(self):
        target = self.folder / 'real'
        target.mkdir()
        link = self.folder / 'linked'
        try:
            link.symlink_to(target, target_is_directory=True)
        except OSError as error:
            self.skipTest(f'Symlink creation unavailable: {error}')
        self.destination = link / 'new'
        with self.assertRaisesRegex(ValueError, 'junction'):
            self.initialize()
        self.assertFalse((target / 'new').exists())

    def test_allowlist_ignores_extra_private_template_files(self):
        source = self.folder / 'source'
        for relative, original in init_site._copy_plan(ROOT).items():
            target = source / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(original, target)
        # Reconstruct the dedicated template rather than copying any checkout.
        for relative in init_site.TEMPLATE_FILES:
            target = source / 'examples/starter' / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / 'examples/starter' / relative, target)
        private = source / 'examples/starter/.env'
        private.write_text('SECRET=private', encoding='utf-8')
        self.initialize(source=source)
        self.assertFalse((self.destination / '.env').exists())


if __name__ == '__main__':
    unittest.main()
