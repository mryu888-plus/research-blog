"""Profile, download and launch boundary tests; never launch a real client."""
from email.message import Message
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch, MagicMock
import urllib.error
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import zotero


class Response:
    def __init__(self, headers):
        self.headers = headers

    def __enter__(self):
        return self

    def __exit__(self, *args):
        pass


class ZoteroHelperTests(unittest.TestCase):
    def setUp(self):
        scratch = ROOT / '.tools'
        scratch.mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix='zotero-helper-test-', dir=scratch)
        self.root = Path(self.temp.name).resolve()
        self.assertTrue(self.root.is_relative_to(scratch.resolve()))
        self.addCleanup(self.temp.cleanup)
        self.directory = self.root / '.tools/zotero'
        for mocked in (patch.object(zotero, 'ROOT', self.root), patch.object(zotero, 'DIRECTORY', self.directory)):
            mocked.start()
            self.addCleanup(mocked.stop)

    def app(self):
        app = self.directory / 'client/Zotero_win-x64/zotero.exe'
        app.parent.mkdir(parents=True, exist_ok=True)
        app.write_bytes(b'fixture executable; must not launch')
        return app

    def archive(self, members):
        self.directory.mkdir(parents=True, exist_ok=True)
        archive = self.directory / f'Zotero-{zotero.VERSION}_win-x64.zip'
        with zipfile.ZipFile(archive, 'w') as output:
            for name, content in members:
                output.writestr(name, content)
        return archive

    def test_private_path_rejects_outside_and_normalizes_inside(self):
        with self.assertRaises(ValueError):
            zotero.private_path(self.root / 'site/library')
        self.assertEqual(zotero.private_path(self.root / '.tools/one/../two'), self.root / '.tools/two')

    def test_profiles_named_differently_do_not_share_a_library(self):
        first = zotero.prepare_profile(self.root / '.tools/tests/first', 23129)
        second = zotero.prepare_profile(self.root / '.tools/tests/second', 23130)
        self.assertIn(json.dumps(str(first.parent / 'first-library')), (first / 'user.js').read_text())
        self.assertIn(json.dumps(str(second.parent / 'second-library')), (second / 'user.js').read_text())
        normal = zotero.prepare_profile(self.directory / 'profile', 23119)
        self.assertIn(json.dumps(str(self.directory / 'library')), (normal / 'user.js').read_text())

    def test_profile_symlink_and_user_js_symlink_cannot_redirect_writes(self):
        profile = self.directory / 'profile'
        profile.mkdir(parents=True)
        outside = self.root / 'outside.js'
        outside.write_text('preserve')
        try:
            (profile / 'user.js').symlink_to(outside)
        except OSError as error:
            self.skipTest(f'Windows symlink privilege unavailable: {error}')
        with self.assertRaises(ValueError):
            zotero.prepare_profile(profile, 23119)
        self.assertEqual(outside.read_text(), 'preserve')
        (profile / 'user.js').unlink()
        alternate = self.directory / 'alternate'
        alternate.mkdir()
        redirected = self.directory / 'redirected'
        redirected.symlink_to(alternate, target_is_directory=True)
        with self.assertRaises(ValueError):
            zotero.prepare_profile(redirected, 23119)

    def test_profile_junction_cannot_redirect_library_writes(self):
        if os.name != 'nt':
            self.skipTest('Windows junction test')
        self.directory.mkdir(parents=True)
        alternate = self.root / 'outside-profile'
        alternate.mkdir()
        redirected = self.directory / 'redirected-profile'
        quoted_path = str(redirected).replace("'", "''")
        quoted_target = str(alternate).replace("'", "''")
        shell = zotero.shutil.which('pwsh.exe') or zotero.shutil.which('powershell.exe')
        created = subprocess.run([shell, '-NoProfile', '-NonInteractive', '-Command',
                                  f"New-Item -ItemType Junction -Path '{quoted_path}' -Target '{quoted_target}' | Out-Null"],
                                 capture_output=True, timeout=15)
        if created.returncode:
            self.skipTest('This Windows filesystem does not permit junction creation')
        self.assertTrue(redirected.is_junction())
        with self.assertRaises(ValueError):
            zotero.prepare_profile(redirected, 23119)
        self.assertFalse((alternate / 'user.js').exists())

    def test_user_js_link_metadata_is_checked_before_overwriting_preferences(self):
        profile = self.directory / 'profile'
        profile.mkdir(parents=True)
        preferences = profile / 'user.js'
        preferences.write_text('must be preserved')
        original = Path.is_symlink
        with patch.object(Path, 'is_symlink', side_effect=lambda target: target == preferences or original(target), autospec=True), self.assertRaises(ValueError):
            zotero.prepare_profile(profile, 23119)
        self.assertEqual(preferences.read_text(), 'must be preserved')

    def test_zip_rejects_traversal_absolute_ads_and_windows_normalization(self):
        client = self.directory / 'client'
        client.mkdir(parents=True)
        for name in ['../escape.exe', '/absolute.exe', 'C:/absolute.exe', 'a\\..\\escape.exe', 'a/name.exe:stream', 'a/../escape.exe', 'a/NUL.txt', 'a/space /x.exe', 'a/dot./x.exe', './zotero.exe']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                zotero.check_zip_member(client, zipfile.ZipInfo(name))
        accepted = zotero.check_zip_member(client, zipfile.ZipInfo('Zotero_win-x64/app/omni.ja'))
        self.assertEqual(accepted, client / 'Zotero_win-x64/app/omni.ja')

    def test_zip_rejects_symlinks_and_validates_all_members_before_extracting(self):
        member = zipfile.ZipInfo('Zotero_win-x64/link')
        member.create_system = 3
        member.external_attr = 0o120777 << 16
        client = self.directory / 'client'
        client.mkdir(parents=True)
        with self.assertRaises(ValueError):
            zotero.check_zip_member(client, member)
        self.archive([('Zotero_win-x64/zotero.exe', b'fixture'), ('../escape', b'unsafe')])
        with self.assertRaises(ValueError), patch.object(zotero, 'verify_signature') as verify:
            zotero.setup()
        verify.assert_not_called()
        self.assertFalse((client / 'Zotero_win-x64/zotero.exe').exists())

    def test_setup_recognizes_actual_official_folder_and_verifies_signature(self):
        app = self.app()
        with patch.object(zotero, 'verify_signature', side_effect=lambda value: value) as verify, patch.object(zotero.urllib.request, 'urlopen') as download:
            self.assertEqual(zotero.setup(), app)
        verify.assert_called_once_with(app)
        download.assert_not_called()

    def test_setup_extracts_official_layout_and_verifies_before_returning(self):
        self.archive([('Zotero_win-x64/zotero.exe', b'fixture'), ('Zotero_win-x64/app/omni.ja', b'fixture')])
        with patch.object(zotero, 'verify_signature', side_effect=lambda value: value) as verify:
            app = zotero.setup()
        self.assertEqual(app, self.directory / 'client/Zotero_win-x64/zotero.exe')
        verify.assert_called_once_with(app)
        self.assertTrue((app.parent / 'app/omni.ja').is_file())

    def test_signature_requires_valid_status_and_the_official_publisher(self):
        app = self.app()
        for signature in [{'status': 'HashMismatch', 'subject': 'Corporation for Digital Scholarship'}, {'status': 'Valid', 'subject': 'Unrelated publisher'}]:
            output = subprocess.CompletedProcess([], 0, json.dumps(signature), '')
            with patch.object(zotero.subprocess, 'run', return_value=output), self.assertRaises(RuntimeError):
                zotero.verify_signature(app)
        output = subprocess.CompletedProcess([], 0, json.dumps({'status': 'Valid', 'subject': 'CN=Corporation for Digital Scholarship'}), '')
        with patch.object(zotero.subprocess, 'run', return_value=output):
            self.assertEqual(zotero.verify_signature(app), app)

    def test_status_requires_zotero_identity_and_disables_proxy_redirects(self):
        for headers, expected in [({}, False), ({'Zotero-API-Version': '3'}, False), ({'X-Zotero-Version': '10.0.6', 'Zotero-API-Version': '3'}, True)]:
            opener = MagicMock()
            opener.open.return_value = Response(headers)
            with patch.object(zotero.urllib.request, 'build_opener', return_value=opener) as build:
                self.assertEqual(zotero.status()['running'], expected)
            self.assertTrue(any(isinstance(arg, zotero.NoLocalRedirect) for arg in build.call_args.args))
            proxy = next(arg for arg in build.call_args.args if isinstance(arg, zotero.urllib.request.ProxyHandler))
            self.assertEqual(proxy.proxies, {})
        self.assertIsNone(zotero.NoLocalRedirect().redirect_request(None, None, 302, '', {}, 'https://example.test'))

    def test_disabled_api_is_distinguished_from_a_foreign_http_error(self):
        for identity, expected in [(False, False), (True, True)]:
            headers = Message()
            if identity:
                headers['X-Zotero-Version'] = '10.0.6'
                headers['Zotero-API-Version'] = '3'
            opener = MagicMock()
            opener.open.side_effect = urllib.error.HTTPError('http://127.0.0.1:23119/api/', 403, 'Forbidden', headers, None)
            with patch.object(zotero.urllib.request, 'build_opener', return_value=opener):
                state = zotero.status()
            self.assertEqual(state['running'], expected)
            self.assertFalse(state['apiEnabled'])

    def test_default_profile_launch_accepts_uri_remoting_and_hidden_window(self):
        app = self.app()
        uri = 'zotero://open-pdf/library/items/ABCD2345?page=1&annotation=BCDE3456'
        with patch.object(zotero, 'status', return_value={'running': False}), patch.object(zotero, 'setup', return_value=app), patch.object(zotero.subprocess, 'Popen') as spawn:
            spawn.return_value.pid = 123
            result = zotero.start(uri=uri, hidden=True)
        command = spawn.call_args.args[0]
        self.assertNotIn('-no-remote', command)
        self.assertEqual(command[command.index('-profile') + 1], str(self.directory / 'profile'))
        self.assertEqual(command[-2:], ['-url', uri])
        self.assertEqual(result['profile'], str(self.directory / 'profile'))
        if os.name == 'nt':
            self.assertEqual(spawn.call_args.kwargs['startupinfo'].wShowWindow, 0)

    def test_explicit_test_profile_uses_no_remote_and_refuses_an_occupied_port(self):
        app = self.app()
        profile = self.root / '.tools/test-instance/profile'
        with patch.object(zotero, 'status', return_value={'running': False}), patch.object(zotero, 'setup', return_value=app), patch.object(zotero.subprocess, 'Popen') as spawn:
            zotero.start(port=23129, profile=profile)
        self.assertIn('-no-remote', spawn.call_args.args[0])
        self.assertIn(str(profile), spawn.call_args.args[0])
        with patch.object(zotero, 'status', return_value={'running': True}), patch.object(zotero.subprocess, 'Popen') as spawn, self.assertRaises(RuntimeError):
            zotero.start(port=23129, profile=profile)
        spawn.assert_not_called()

    def test_running_default_instance_is_reused_without_touching_profile(self):
        with patch.object(zotero, 'status', return_value={'running': True, 'serverID': 'existing'}), patch.object(zotero, 'prepare_profile') as prepare, patch.object(zotero.subprocess, 'Popen') as spawn:
            self.assertEqual(zotero.start()['serverID'], 'existing')
        prepare.assert_not_called()
        spawn.assert_not_called()

    def test_invalid_uris_and_outside_profiles_are_rejected_before_any_network_or_launch(self):
        for uri in ['https://example.test', 'zotero://open-pdf/library/../../file', 'zotero://select/groups/12/items/invalid', 'zotero://open-pdf/library/items/ABCD2345?page=0', 'zotero://open-pdf/library/items/ABCD2345?file=C:/private', 'zotero://open-pdf/library/items/ABCD2345#fragment']:
            with self.subTest(uri=uri), patch.object(zotero, 'status') as status, patch.object(zotero.subprocess, 'Popen') as spawn, self.assertRaises(ValueError):
                zotero.start(uri=uri)
            status.assert_not_called()
            spawn.assert_not_called()
        with patch.object(zotero, 'status') as status, self.assertRaises(ValueError):
            zotero.start(profile=self.root / 'outside')
        status.assert_not_called()


if __name__ == '__main__':
    unittest.main()
