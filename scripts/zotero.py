"""Run the official Zotero desktop client with a private, project-local profile.

No Zotero database or PDF is placed under site/. A system installation is also
usable through the workbench; this helper offers a reproducible local client.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
import zipfile
from urllib.parse import urlsplit, parse_qs

ROOT = Path(__file__).resolve().parents[1]
VERSION = '10.0.6'
DIRECTORY = ROOT / '.tools' / 'zotero'
DOWNLOAD = f'https://download.zotero.org/client/release/{VERSION}/Zotero-{VERSION}_win-x64.zip'


def private_path(path):
    path = Path(path).absolute()
    tools = (ROOT / '.tools').resolve()
    for parent in (path, *path.parents):
        if parent.is_symlink() or parent.is_junction():
            raise ValueError(f'Zotero private storage must not use links: {parent}')
    if not path.resolve().is_relative_to(tools):
        raise ValueError('Zotero managed files must stay within the project .tools directory')
    return path.resolve()


def executable():
    direct = DIRECTORY / 'client' / 'zotero.exe'
    if direct.is_file():
        return direct
    found = sorted((DIRECTORY / 'client').glob('*/zotero.exe'))
    return found[0] if found else direct


def verify_signature(app):
    """Authenticate the official Windows executable before returning or launching it."""
    app = private_path(app)
    quoted = str(app).replace("'", "''")
    shell = shutil.which('pwsh.exe') or shutil.which('powershell.exe')
    if not shell:
        raise RuntimeError('PowerShell is required to verify the official Zotero signature')
    # VS Code may pass PowerShell 7's module path to Windows PowerShell 5. Load
    # the chosen shell's own signed modules explicitly rather than autoloading.
    command = ('Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Security/Microsoft.PowerShell.Security.psd1" -ErrorAction Stop; '
               'Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1" -ErrorAction Stop; '
               f"$s = Get-AuthenticodeSignature -LiteralPath '{quoted}'; "
               "@{status=$s.Status.ToString(); subject=$s.SignerCertificate.Subject} | ConvertTo-Json -Compress")
    checked = subprocess.run([shell, '-NoProfile', '-NonInteractive', '-Command', command],
                             capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30,
                             creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0)
    try:
        signature = json.loads(checked.stdout.strip())
    except (ValueError, TypeError) as error:
        raise RuntimeError('Could not verify the downloaded Zotero executable signature') from error
    if checked.returncode or not isinstance(signature, dict) or signature.get('status') != 'Valid' or 'Corporation for Digital Scholarship' not in (signature.get('subject') or ''):
        raise RuntimeError('Zotero executable must have a valid Corporation for Digital Scholarship signature')
    return app


def check_zip_member(client, member):
    name = member.filename
    parts = name.rstrip('/').split('/')
    # Reject Windows normalization, drive paths, ADS and device names before
    # zipfile's extraction normalizes them into a different destination.
    if (not name or '\\' in name or ':' in name or '\x00' in member.orig_filename or name.startswith('/')
            or any(part in {'', '.', '..'} or part.rstrip(' .') != part for part in parts)):
        raise ValueError('Unsafe path in the Zotero ZIP')
    devices = {'CON', 'PRN', 'AUX', 'NUL', *(f'COM{i}' for i in range(1, 10)), *(f'LPT{i}' for i in range(1, 10))}
    if any(part.split('.')[0].upper() in devices for part in parts):
        raise ValueError('Unsafe Windows device path in the Zotero ZIP')
    destination = private_path(client / name)
    if not destination.is_relative_to(client.resolve()):
        raise ValueError('Unsafe path in the Zotero ZIP')
    if (member.external_attr >> 16) & 0o170000 == 0o120000:
        raise ValueError('Links are not accepted in the Zotero ZIP')
    return destination


def setup():
    if os.name != 'nt':
        raise RuntimeError('On macOS/Linux, install Zotero from https://www.zotero.org/download/ and enable its local API.')
    private_path(DIRECTORY).mkdir(parents=True, exist_ok=True)
    app = executable()
    if app.is_file():
        return verify_signature(app)
    archive = private_path(DIRECTORY / f'Zotero-{VERSION}_win-x64.zip')
    if not archive.is_file():
        temporary = private_path(archive.with_suffix('.download'))
        print(f'Downloading official Zotero {VERSION}...', flush=True)
        try:
            with urllib.request.urlopen(DOWNLOAD, timeout=60) as response, temporary.open('wb') as output:
                shutil.copyfileobj(response, output)
            temporary.replace(archive)
        finally:
            temporary.unlink(missing_ok=True)
    client = private_path(DIRECTORY / 'client')
    client.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive) as source:
        members = source.infolist()
        for member in members:
            check_zip_member(client, member)
        source.extractall(client)
    app = executable()
    if not app.is_file():
        raise RuntimeError('The official ZIP did not contain zotero.exe')
    return verify_signature(app)


class NoLocalRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        return None


def status(port=23119):
    try:
        request = urllib.request.Request(f'http://127.0.0.1:{port}/api/', headers={'Zotero-API-Version': '3'})
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoLocalRedirect())
        with opener.open(request, timeout=2) as response:
            identified = bool(response.headers.get('X-Zotero-Version') and response.headers.get('Zotero-API-Version'))
            return {'running': identified, 'apiEnabled': identified, 'apiVersion': response.headers.get('Zotero-API-Version'),
                    'serverID': response.headers.get('Zotero-Server-ID'), 'port': port}
    except urllib.error.HTTPError as error:
        identified = bool(error.headers.get('X-Zotero-Version') and error.headers.get('Zotero-API-Version'))
        return {'running': identified, 'apiEnabled': False, 'status': error.code, 'port': port}
    except (urllib.error.URLError, TimeoutError, OSError):
        return {'running': False, 'apiEnabled': False, 'port': port}


def prepare_profile(profile, port):
    profile = private_path(profile)
    profile.mkdir(parents=True, exist_ok=True)
    library_name = 'library' if profile.name == 'profile' else profile.name + '-library'
    library = private_path(profile.parent / library_name)
    library.mkdir(parents=True, exist_ok=True)
    preferences = {
        'extensions.zotero.httpServer.enabled': True,
        'extensions.zotero.httpServer.localAPI.enabled': True,
        'extensions.zotero.httpServer.port': port,
        'extensions.zotero.useDataDir': True,
        'extensions.zotero.dataDir': str(library),
        'extensions.zotero.firstRun2': False,
    }
    # Only our managed profile is configured. Existing user profiles are untouched.
    private_path(profile / 'user.js').write_text(''.join(
        f'user_pref({json.dumps(name)}, {json.dumps(value)});\n' for name, value in preferences.items()), encoding='utf-8')
    return profile


def start(port=23119, profile=None, uri=None, hidden=False):
    if not 1024 <= port <= 65535:
        raise ValueError('Port must be between 1024 and 65535')
    if uri:
        parsed = urlsplit(uri)
        components = parsed.path.strip('/').split('/')
        key = lambda value: len(value) == 8 and all(char in '23456789ABCDEFGHIJKLMNPQRSTUVWXYZ' for char in value)
        group_prefix = len(components) >= 2 and components[0] == 'groups' and components[1].isdigit() and int(components[1]) > 0
        library_path = components == ['library'] or len(components) == 2 and group_prefix
        item_path = ((len(components) == 3 and components[0] == 'library' and components[1] == 'items' and key(components[2]))
                     or (len(components) == 4 and group_prefix and components[2] == 'items' and key(components[3])))
        parameters = parse_qs(parsed.query, keep_blank_values=True)
        valid_parameters = (set(parameters) <= {'page', 'annotation'} and all(len(values) == 1 for values in parameters.values())
                            and ('page' not in parameters or parameters['page'][0].isdigit() and int(parameters['page'][0]) > 0)
                            and ('annotation' not in parameters or key(parameters['annotation'][0])))
        if (parsed.scheme != 'zotero' or parsed.netloc not in {'select', 'open-pdf'} or parsed.fragment
                or not (item_path or parsed.netloc == 'select' and library_path) or not valid_parameters
                or any(ord(char) < 32 for char in uri)):
            raise ValueError('Only Zotero library and PDF links with valid local item keys are accepted')
    isolated = profile is not None
    if isolated:
        profile = private_path(profile)
    current = status(port)
    if isolated and current['running']:
        raise RuntimeError('The isolated test port is already used by Zotero; use another port or the running test instance directly')
    if current['running'] and not uri:
        return current
    app = setup()
    profile = prepare_profile(profile or DIRECTORY / 'profile', port)
    command = [str(app), *(['-no-remote'] if isolated else []), '-profile', str(profile)]
    if uri:
        command += ['-url', uri]
    startup = None
    if os.name == 'nt' and hidden:
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
    child = subprocess.Popen(command, cwd=ROOT, stdin=subprocess.DEVNULL,
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                             startupinfo=startup)
    return {'started': True, 'pid': child.pid, 'profile': str(profile), 'port': port}


def main():
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(encoding='utf-8')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['setup', 'start', 'status'])
    parser.add_argument('--port', type=int, default=23119)
    parser.add_argument('--profile', type=Path)
    parser.add_argument('--uri')
    parser.add_argument('--hidden', action='store_true')
    args = parser.parse_args()
    if not 1024 <= args.port <= 65535:
        parser.error('Port must be between 1024 and 65535')
    if args.action == 'setup':
        print(json.dumps({'executable': str(setup()), 'version': VERSION}))
    elif args.action == 'status':
        print(json.dumps(status(args.port)))
    else:
        print(json.dumps(start(args.port, args.profile, args.uri, args.hidden)))


if __name__ == '__main__':
    main()
