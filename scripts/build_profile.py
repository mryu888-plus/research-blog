"""Build the website profile and resume from profile/cv.typ using Typst itself.

Generated data and PDFs are staged first: a compiler or schema error never
replaces the last successful website/profile outputs. No regex parsing of Typst.
"""
import argparse
from functools import lru_cache
import json
import os
from pathlib import Path
import subprocess
import tempfile
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]


def _strings(value, required, optional=(), *, where):
    if not isinstance(value, dict):
        raise ValueError(f'{where}: expected a dictionary')
    missing = set(required) - value.keys()
    unknown = value.keys() - set(required) - set(optional)
    if missing or unknown:
        raise ValueError(f'{where}: missing fields {sorted(missing)}, unknown fields {sorted(unknown)}')
    for key, item in value.items():
        if not isinstance(item, str):
            raise ValueError(f'{where}.{key}: use a quoted Typst string')
    return dict(value)


def _web_url(value, where):
    parts = urlsplit(value)
    if parts.scheme not in ('https', 'http') or not parts.netloc or any(c.isspace() for c in value):
        raise ValueError(f'{where}: expected an absolute http(s) URL')


def validate_profile(value):
    """Validate the public schema; never pass unknown/private fields to Zola."""
    scalar = ('display_name', 'english_name', 'intro', 'current_research', 'research_interests',
              'github', 'email', 'location', 'tagline')
    arrays = ('education', 'experiences', 'honors')
    if not isinstance(value, dict) or value.get('schema_version') != 1:
        raise ValueError('Expected profile metadata with schema_version: 1')
    unknown = value.keys() - set(scalar) - set(arrays) - {'schema_version'}
    if unknown:
        raise ValueError(f'Unknown public profile fields: {sorted(unknown)}')
    _strings({k: value[k] for k in scalar if k in value}, scalar, where='profile')
    for key in arrays:
        if not isinstance(value.get(key), list):
            raise ValueError(f'profile.{key}: expected an array')
    _web_url(value['github'], 'profile.github')
    if '@' not in value['email'] or any(c.isspace() for c in value['email']) or any(c in value['email'] for c in '?&#:'):
        raise ValueError('profile.email: expected a plain email address')
    for i, entry in enumerate(value['education']):
        _strings(entry, ('degree', 'school', 'date', 'detail'), where=f'education[{i}]')
    for i, entry in enumerate(value['experiences']):
        if not isinstance(entry, dict):
            raise ValueError(f'experiences[{i}]: expected a dictionary')
        _strings({k: v for k, v in entry.items() if k != 'bullets'},
                 ('kind', 'org', 'role', 'date', 'venue', 'status', 'paper_title', 'web_title', 'summary'),
                 ('paper_url',), where=f'experiences[{i}]')
        if entry['kind'] not in ('internship', 'research'):
            raise ValueError(f'experiences[{i}].kind: use internship or research')
        if not isinstance(entry.get('bullets'), list) or not all(isinstance(x, str) for x in entry['bullets']):
            raise ValueError(f'experiences[{i}].bullets: expected an array of strings')
        if entry.get('paper_url'):
            _web_url(entry['paper_url'], f'experiences[{i}].paper_url')
    for i, entry in enumerate(value['honors']):
        _strings(entry, ('year', 'title'), where=f'honors[{i}]')
    return value


def _run(command, **kwargs):
    result = subprocess.run(command, capture_output=True, text=True, encoding='utf-8', **kwargs)
    if result.returncode:
        # Do not include CLI inputs (which may contain a local phone number).
        raise RuntimeError(f'Typst {command[1]} failed:\n{result.stderr.strip()}')
    if result.stderr.strip():
        print(result.stderr.strip(), flush=True)
    return result.stdout


@lru_cache(maxsize=8)
def _font_inputs(typst, font_path, inherited_paths):
    """Keep original Noto fonts when installed, with explicit Windows fallbacks."""
    args = ['--font-path', font_path]
    available = set(_run([typst, 'fonts', *args]).splitlines())
    for key, candidates in (
        ('body-font', ('Noto Sans CJK SC', 'Source Han Sans SC', 'Microsoft YaHei', 'DengXian')),
        ('serif-font', ('Noto Serif CJK SC', 'Source Han Serif SC', 'SimSun', 'Microsoft YaHei')),
    ):
        selected = next((font for font in candidates if font in available), None)
        if not selected:
            raise RuntimeError('No Chinese resume font found. Install Noto Sans/Serif CJK SC (CI: fonts-noto-cjk).')
        args += ['--input', f'{key}={selected}']
    return args


def _replace_if_changed(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and path.read_bytes() == content:
        return
    with tempfile.NamedTemporaryFile(dir=path.parent, suffix='.tmp', delete=False) as file:
        temporary = Path(file.name)
        file.write(content)
    try:
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def build_profile(root=ROOT, include_private=True):
    root = Path(root).resolve()
    source = root / 'profile/cv.typ'
    if not source.is_file():
        raise FileNotFoundError(f'Missing resume source: {source}')
    typst = os.environ.get('TYPST', 'typst')
    options = ['--root', str(source.parent), *_font_inputs(typst, str(source.parent), os.environ.get('TYPST_FONT_PATHS', ''))]
    # Public extraction and compilation NEVER receive local contact inputs.
    raw = _run([typst, 'eval', 'query(<profile>).map(it => it.value)', '--in', str(source), *options])
    values = json.loads(raw)
    if not isinstance(values, list) or len(values) != 1:
        raise ValueError('cv.typ must emit exactly one #metadata(profile) <profile>')
    profile = validate_profile(values[0])
    phone = ''
    local_contact = root / 'profile/contact.local.json'
    if include_private and local_contact.exists():
        contact = json.loads(local_contact.read_text(encoding='utf-8-sig'))
        if not isinstance(contact, dict) or set(contact) - {'phone'} or not isinstance(contact.get('phone', ''), str):
            raise ValueError('contact.local.json must contain only a phone string')
        phone = contact.get('phone', '')
    output = root / 'output/pdf'
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='profile-build-', dir=output) as directory:
        public_pdf = Path(directory) / 'public.pdf'
        _run([typst, 'compile', str(source), str(public_pdf), *options])
        public_bytes = public_pdf.read_bytes()
        if not public_bytes.startswith(b'%PDF-'):
            raise ValueError('Typst did not produce a PDF')
        private_bytes = public_bytes
        if include_private and phone:
            private_pdf = Path(directory) / 'private.pdf'
            _run([typst, 'compile', str(source), str(private_pdf), *options, '--input', f'phone={phone}'])
            private_bytes = private_pdf.read_bytes()
        # All validation and compiles have succeeded before changing served data.
        _replace_if_changed(root / 'site/static/resume.pdf', public_bytes)
        _replace_if_changed(output / 'resume-public.pdf', public_bytes)
        if include_private:
            _replace_if_changed(output / 'resume.pdf', private_bytes)
        _replace_if_changed(root / 'site/data/profile.json', (json.dumps(profile, ensure_ascii=False, indent=2) + '\n').encode('utf-8'))
    return profile


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--public-only', action='store_true', help='Skip the local full resume')
    args = parser.parse_args()
    build_profile(include_private=not args.public_only)
    print('Updated site/data/profile.json and site/static/resume.pdf; PDF copies in output/pdf/.')
