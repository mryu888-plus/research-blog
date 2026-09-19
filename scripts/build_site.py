"""Build the public static blog; requires Zola 0.23.4 and Typst 0.15.1."""
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib
from build_profile import build_profile
from prepare_site import PreparedSite

ROOT = Path(__file__).resolve().parents[1]

def compile_figures(site):
    for article in sorted((site / 'content' / 'posts').rglob('index.md')):
        text = article.read_text(encoding='utf-8-sig')
        metadata = tomllib.loads(text.split('+++', 2)[1])
        if metadata.get('draft', False):
            continue
        for figure in re.findall(r'<fig\s+[^>]*?id="([a-zA-Z0-9_-]+)"', text):
            source = article.parent / 'figures' / f'{figure}.typ'
            subprocess.run([os.environ.get('TYPST', 'typst'), 'compile', str(source),
                            str(article.parent / f'{figure}.svg'), '--format', 'svg'], check=True)
def render_site(site, base_url=None):
    """Build protected input without ever rewriting an author's Markdown."""
    site = Path(site).resolve()
    scratch = ROOT / '.tools'
    scratch.mkdir(exist_ok=True)
    zola = os.environ.get('ZOLA') or shutil.which('zola') or str(scratch / 'zola' / 'zola.exe')
    with tempfile.TemporaryDirectory(prefix='math-build-', dir=scratch) as temporary:
        if not Path(temporary).resolve().is_relative_to(scratch.resolve()):
            raise RuntimeError('Prepared site escaped the build directory')
        prepared = PreparedSite(site, Path(temporary) / 'site')
        prepared.sync()
        command = [zola, '--root', str(prepared.target), 'build',
                   '--output-dir', str(site / 'public'), '--force']
        if base_url:
            command += ['--base-url', base_url]
        subprocess.run(command, check=True)
    # Article manifests contain local source paths and are only needed by the Agent.
    for manifest in (site / 'public').rglob('article-manifest.json'):
        manifest.unlink()
    (site / 'public' / '.nojekyll').touch()


def build(base_url=None, *, skip_figures=False):
    site = ROOT / 'site'
    build_profile(root=ROOT)
    if not skip_figures:
        compile_figures(site)
    render_site(site, base_url)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--base-url')
    parser.add_argument('--skip-figures', action='store_true',
                        help='Reuse figures already compiled by the Rust builder')
    args = parser.parse_args()
    build(args.base_url, skip_figures=args.skip_figures)
