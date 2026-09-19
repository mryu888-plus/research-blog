"""Build the public static blog; requires Zola 0.23.4 and Typst 0.15.1."""
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import tomllib
from build_profile import build_profile

ROOT = Path(__file__).resolve().parents[1]

def build(base_url=None):
    site = ROOT / 'site'
    build_profile(root=ROOT)
    for article in sorted((site / 'content' / 'posts').rglob('index.md')):
        text = article.read_text(encoding='utf-8-sig')
        metadata = tomllib.loads(text.split('+++', 2)[1])
        if metadata.get('draft', False):
            continue
        for figure in re.findall(r'<fig\s+[^>]*?id="([a-zA-Z0-9_-]+)"', text):
            source = article.parent / 'figures' / f'{figure}.typ'
            subprocess.run([os.environ.get('TYPST', 'typst'), 'compile', str(source),
                            str(article.parent / f'{figure}.svg'), '--format', 'svg'], check=True)
    zola = os.environ.get('ZOLA') or shutil.which('zola') or str(ROOT / '.tools' / 'zola' / 'zola.exe')
    command = [zola, '--root', str(site), 'build']
    if base_url:
        command += ['--base-url', base_url]
    subprocess.run(command, check=True)
    # Article manifests contain local source paths and are only needed by the Agent.
    for manifest in (site / 'public').rglob('article-manifest.json'):
        manifest.unlink()
    (site / 'public' / '.nojekyll').touch()

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--base-url')
    build(parser.parse_args().base_url)
