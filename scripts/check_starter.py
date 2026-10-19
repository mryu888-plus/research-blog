"""Build and exercise a newly initialized workstation, including live preview.

This uses a fresh destination, installs its locked editor dependencies and only
changes demonstration files in that destination. It never opens a browser or
connects to the user's Zotero library.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def run(command, root):
    subprocess.run([str(part) for part in command], cwd=root, check=True)


def fetch(url):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(url, timeout=2) as response:
        return response.read().decode('utf-8')


def wait_for(check, description, timeout=40):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            result = check()
            if result:
                return result
        except (OSError, ValueError, urllib.error.URLError):
            pass
        time.sleep(.2)
    raise RuntimeError(f'Timed out waiting for {description}')


def stop_preview(process):
    if process.poll() is not None:
        return
    if os.name == 'nt':
        # The PID belongs to the subprocess created immediately below. Stop its
        # child Zola/editor servers too; never select processes by app name.
        subprocess.run(['taskkill.exe', '/PID', str(process.pid), '/T', '/F'],
                       capture_output=True, check=False)
    else:
        os.killpg(process.pid, signal.SIGINT)
    try:
        process.wait(timeout=8)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=3)


def check_starter(destination, source=ROOT):
    source = Path(source).resolve()
    destination = Path(destination).absolute()
    run([sys.executable, source / 'scripts/init_site.py', destination,
         '--name', 'Research Workbench', '--url', 'https://example.invalid/notes/'], source)
    npm = shutil.which('npm.cmd' if os.name == 'nt' else 'npm')
    if not npm:
        raise RuntimeError('Node.js and npm are required for the editor preview')
    run([npm, '--prefix', 'extensions/research-blog', 'ci', '--ignore-scripts', '--no-audit', '--no-fund'], destination)
    run([npm, '--prefix', 'extensions/research-blog', 'run', 'build'], destination)
    run([sys.executable, 'scripts/build_site.py'], destination)
    run([sys.executable, 'scripts/check_site.py'], destination)
    public = destination / 'site/public'
    assert not (destination / 'profile').exists(), 'Personal CV must not be initialized'
    assert not (public / 'resume.pdf').exists(), 'Personal resume must not be published'
    assert not (public / 'preview-editor.json').exists(), 'Preview credentials must stay private'
    workflow = destination / '.github/workflows/pages.yml'
    assert workflow.is_file(), 'Independent sites need their own deployment workflow'
    assert 'check_starter.py' not in workflow.read_text(encoding='utf-8'), 'Starter deployment must not recursively initialize another starter'
    run(['node', 'scripts/sync_zen.cjs', '--check'], destination)

    log = destination / '.tools/starter-preview.log'
    log.parent.mkdir(exist_ok=True)
    options = {'creationflags': subprocess.CREATE_NEW_PROCESS_GROUP} if os.name == 'nt' else {'start_new_session': True}
    with log.open('w', encoding='utf-8') as output:
        preview = subprocess.Popen([sys.executable, 'scripts/editor.py', 'preview', '--no-open'],
                                   cwd=destination, stdout=output, stderr=subprocess.STDOUT, **options)
        try:
            def preview_url():
                if preview.poll() is not None:
                    raise RuntimeError('Starter preview exited; see its private log')
                match = re.search(r'网页实时预览：(http://127\.0\.0\.1:\d+/)', log.read_text(encoding='utf-8'))
                return match.group(1) if match else None
            base = wait_for(preview_url, 'preview startup')
            wait_for(lambda: 'Research Workbench' in fetch(base), 'starter homepage')
            assert 'preview.js' in fetch(base)
            # A fresh draft proves preview includes unpublished work while the
            # earlier public build remains unchanged.
            draft = destination / 'site/content/posts/preview-probe/index.md'
            draft.parent.mkdir(parents=True)
            draft.write_text('+++\ntitle = "Live preview probe"\ndate = 2026-10-12\ndraft = true\n+++\n\nFIRST_PREVIEW_MARKER\n', encoding='utf-8')
            wait_for(lambda: 'FIRST_PREVIEW_MARKER' in fetch(base + 'posts/preview-probe/'), 'draft preview')
            draft.write_text(draft.read_text(encoding='utf-8').replace('FIRST_PREVIEW_MARKER', 'UPDATED_PREVIEW_MARKER'), encoding='utf-8')
            wait_for(lambda: 'UPDATED_PREVIEW_MARKER' in fetch(base + 'posts/preview-probe/'), 'article hot update')
            zen_path = destination / 'ZEN.md'
            original_zen = zen_path.read_text(encoding='utf-8')
            block = re.search(r'(<!-- zen:tokens -->\s*```json\s*)([\s\S]*?)(\s*```\s*<!-- /zen:tokens -->)', original_zen)
            if not block:
                raise RuntimeError('ZEN tokens are missing')
            tokens = json.loads(block[2])
            original_accent = tokens['palettes']['paper']['accent']
            probe_accent = '#7049a8' if original_accent.lower() != '#7049a8' else '#476a92'
            tokens['palettes']['paper']['accent'] = probe_accent
            changed_zen = original_zen[:block.start(2)] + json.dumps(tokens, ensure_ascii=False, indent=2) + original_zen[block.end(2):]
            assert changed_zen != original_zen
            zen_path.write_text(changed_zen, encoding='utf-8')
            wait_for(lambda: f'--accent: {probe_accent};' in fetch(base + 'zen.css'), 'ZEN hot update')
            theme = destination / 'extensions/research-notes-theme/themes/research-notes-paper-color-theme.json'
            wait_for(lambda: json.loads(theme.read_text(encoding='utf-8'))['colors']['editorCursor.foreground'] == probe_accent, 'editor style synchronization')
            zen_path.write_text(original_zen, encoding='utf-8')
            wait_for(lambda: f'--accent: {original_accent};' in fetch(base + 'zen.css'), 'ZEN restoration')
        finally:
            stop_preview(preview)
    run([sys.executable, 'scripts/build_site.py'], destination)
    run([sys.executable, 'scripts/check_site.py'], destination)
    assert not (public / 'posts/preview-probe').exists(), 'Preview draft leaked into public output'
    report = {'ok': True, 'checks': ['fresh initialization without personal data', 'locked editor bundle',
              'public build and links', 'draft preview', 'article hot update', 'ZEN hot update and editor synchronization',
              'private preview credentials excluded', 'draft excluded from final public output']}
    (destination / '.tools/starter-result.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print('PASS: fresh starter, build, live preview, ZEN and public boundary')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination', type=Path, required=True, help='New, empty workstation directory')
    args = parser.parse_args()
    check_starter(args.destination)
