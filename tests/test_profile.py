"""Integration checks for the single-source profile, using the real Typst CLI."""
import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from build_profile import build_profile, validate_profile


class ProfileBuildTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not shutil.which(os.environ.get('TYPST', 'typst')):
            raise unittest.SkipTest('Typst is required for profile integration checks')

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='profile-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        shutil.copytree(ROOT / 'profile', self.root / 'profile',
                        ignore=shutil.ignore_patterns('contact.local.json', '*.pdf'))
        self.source = self.root / 'profile/cv.typ'
        self.outputs = ('site/data/profile.json', 'site/static/resume.pdf',
                        'output/pdf/resume.pdf', 'output/pdf/resume-public.pdf')

    def snapshot(self):
        return {name: (self.root / name).read_bytes() for name in self.outputs}

    def test_typst_expression_edits_update_data_and_pdf_and_recover_after_errors(self):
        original = self.source.read_text(encoding='utf-8')
        first = build_profile(self.root)
        baseline = self.snapshot()
        # Imported/computed data is evaluated by Typst rather than source regexes.
        (self.root / 'profile/name.typ').write_text('#let alias = "Pipeline Probe"\n', encoding='utf-8')
        changed = '#import "name.typ": alias\n' + original.replace(
            f'display_name: "{first["display_name"]}"', 'display_name: alias + " Updated"', 1)
        self.assertNotEqual(changed, original)
        self.source.write_text(changed, encoding='utf-8')
        updated = build_profile(self.root)
        self.assertEqual(updated['display_name'], 'Pipeline Probe Updated')
        data = json.loads((self.root / 'site/data/profile.json').read_text(encoding='utf-8'))
        self.assertEqual(data['display_name'], updated['display_name'])
        latest = self.snapshot()
        self.assertNotEqual(baseline['site/static/resume.pdf'], latest['site/static/resume.pdf'])
        self.source.write_text(changed + '\n#let broken = (', encoding='utf-8')
        with self.assertRaises(RuntimeError):
            build_profile(self.root)
        self.assertEqual(latest, self.snapshot(), 'A Typst error must preserve all last-good outputs')
        self.source.write_text(original, encoding='utf-8')
        self.assertEqual(build_profile(self.root)['display_name'], first['display_name'])

    def test_local_contact_never_changes_public_outputs(self):
        # Freeze document timestamps so byte equality is meaningful.
        with patch.dict(os.environ, {'SOURCE_DATE_EPOCH': '1789776000'}):
            data = build_profile(self.root)
            baseline = self.snapshot()
            (self.root / 'profile/contact.local.json').write_text('{"phone": "+00 123 4567"}', encoding='utf-8')
            self.assertEqual(build_profile(self.root), data)
            full = self.snapshot()
            for name in ('site/data/profile.json', 'site/static/resume.pdf', 'output/pdf/resume-public.pdf'):
                self.assertEqual(baseline[name], full[name], name)
            self.assertNotEqual(baseline['output/pdf/resume.pdf'], full['output/pdf/resume.pdf'])
            self.assertNotIn('phone', json.loads(full['site/data/profile.json']))
            build_profile(self.root, include_private=False)
            self.assertEqual(full['output/pdf/resume.pdf'], (self.root / 'output/pdf/resume.pdf').read_bytes())

    def test_public_schema_rejects_private_fields_and_unsafe_links(self):
        data = build_profile(self.root, include_private=False)
        private = copy.deepcopy(data)
        private['phone'] = 'must not publish'
        with self.assertRaises(ValueError):
            validate_profile(private)
        unsafe = copy.deepcopy(data)
        unsafe['experiences'][0]['paper_url'] = 'javascript:alert(1)'
        with self.assertRaises(ValueError):
            validate_profile(unsafe)


if __name__ == '__main__':
    unittest.main()
