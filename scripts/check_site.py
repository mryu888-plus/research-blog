"""Validate any Research Blog build: links, search data and public-only assets."""
import argparse
from html.parser import HTMLParser
from html import unescape
from pathlib import Path
from urllib.parse import urljoin, urlsplit, unquote
import json

from site_features import profile_enabled, read_config

ROOT = Path(__file__).resolve().parents[1]


class Page(HTMLParser):
    def __init__(self, path):
        super().__init__()
        self.links = []
        self.ids = set()
        self.feed(path.read_text(encoding='utf-8'))

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        if 'id' in values:
            self.ids.add(values['id'])
        for attribute in ('href', 'src'):
            if values.get(attribute):
                self.links.append(values[attribute])


def validate_site(site=None, output=None, base_url=None):
    """Return build counts, or raise ValueError with actionable failures.

    Site inputs, output directory and the build's base URL are explicit so a
    generated starter or a preview-prefix build can be checked independently.
    """
    site = Path(site or ROOT / 'site').resolve()
    public = Path(output or site / 'public').resolve()
    config = read_config(site)
    enabled = profile_enabled(config)
    base = (base_url or config['base_url']).rstrip('/') + '/'
    base_parts = urlsplit(base)
    if base_parts.scheme not in ('http', 'https') or not base_parts.netloc:
        raise ValueError('The validation base URL must be an absolute http(s) URL')
    if not public.is_dir():
        raise ValueError(f'Build output is missing: {public}')
    json_routes = {'search-index/index.html', 'graph-data/index.html'}
    pages = {p: Page(p) for p in public.rglob('*.html')
             if p.relative_to(public).as_posix() not in json_routes}
    errors = []
    checked = 0

    def require(condition, message):
        if not condition:
            errors.append(message)

    def local_target(url, context):
        parts = urlsplit(url)
        if parts.netloc != base_parts.netloc or parts.scheme not in ('http', 'https'):
            return None
        if not parts.path.startswith(base_parts.path):
            errors.append(f'{context}: escapes site prefix: {url}')
            return None
        local = (public / unquote(parts.path[len(base_parts.path):])).resolve()
        if not local.is_relative_to(public):
            errors.append(f'{context}: escapes output directory: {url}')
            return None
        if local.is_dir():
            local /= 'index.html'
        require(local.is_file(), f'{context}: missing target: {url}')
        return local

    for path, page in pages.items():
        relative = path.relative_to(public).as_posix()
        page_url = urljoin(base, relative.removesuffix('index.html'))
        for link in page.links:
            absolute = urljoin(page_url, link)
            parts = urlsplit(absolute)
            if parts.netloc != base_parts.netloc or parts.scheme not in ('http', 'https'):
                continue
            local = local_target(absolute, relative)
            if parts.fragment and local in pages:
                require(unquote(parts.fragment) in pages[local].ids,
                        f'{relative}: missing anchor: {link}')
            checked += 1

    def read_json(relative):
        path = public / relative
        try:
            return json.loads(path.read_text(encoding='utf-8-sig'))
        except (OSError, ValueError) as error:
            errors.append(f'{relative}: cannot read JSON: {error}')
            return []

    index = read_json('search-index/index.html')
    graph = read_json('graph-data/index.html')
    require(isinstance(index, list), 'Search index must contain an array')
    require(isinstance(graph, list), 'Graph data must contain an array')
    index = index if isinstance(index, list) else []
    graph = graph if isinstance(graph, list) else []
    for post in index:
        if not isinstance(post, dict) or not isinstance(post.get('title'), str) or not isinstance(post.get('url'), str):
            errors.append('Search index contains an invalid article')
            continue
        target = local_target(post['url'], f'Search article {post["title"]}')
        require(target is not None, f'Search article URL must belong to the site: {post["url"]}')
    graph_ids = set()
    for post in graph:
        if not isinstance(post, dict) or not isinstance(post.get('id'), str) or not isinstance(post.get('url'), str):
            errors.append('Graph data contains an invalid article')
            continue
        require(post['id'] not in graph_ids, f'Duplicate graph article: {post["id"]}')
        graph_ids.add(post['id'])
        require(post.get('draft') is False, f'Draft leaked into graph: {post["id"]}')
        require(local_target(post['url'], f'Graph article {post["id"]}') is not None,
                f'Graph article URL must belong to the site: {post["url"]}')
        require(isinstance(post.get('html'), str) and isinstance(post.get('headings'), list),
                f'Graph article body/headings are invalid: {post["id"]}')

    for relative in ('index.html', 'atom.xml', 'sitemap.xml', '404.html',
                     'graph/index.html', 'references/index.html'):
        require((public / relative).is_file(), f'Required public route is missing: {relative}')
    if enabled:
        profile_path = site / 'data/profile.json'
        try:
            profile = json.loads(profile_path.read_text(encoding='utf-8-sig'))
        except (OSError, ValueError) as error:
            profile = {}
            errors.append(f'Profile is enabled but generated metadata is missing/invalid: {error}')
        about_path = public / 'about/index.html'
        require(about_path in pages, 'Profile is enabled but /about/ is missing')
        if about_path in pages:
            require(any(urlsplit(link).path.endswith('/resume.pdf') for link in pages[about_path].links),
                    'Profile must link to the public resume')
            require(bool(profile.get('display_name')) and profile['display_name'] in unescape(about_path.read_text(encoding='utf-8')),
                    'About page must display the generated profile name')
        resume = public / 'resume.pdf'
        require(resume.is_file() and resume.read_bytes().startswith(b'%PDF-'),
                'Profile is enabled but the public resume is missing/invalid')

    private_names = {'contact.local.json', 'resume-public.pdf', 'preview-editor.json',
                     'references.json', 'original-bodies.json', 'article-manifest.json', 'localapikeys.json',
                     'zotero.sqlite', 'zotero.sqlite-journal', 'zotero.sqlite-wal'}
    for path in public.rglob('*'):
        name = path.name.lower()
        require(name not in private_names and not name.startswith('.env'),
                f'Private build input leaked into public output: {path.relative_to(public)}')
    require(not (public / 'profile').exists(), 'Resume source and private contacts must stay outside published files')
    for path in pages:
        text = path.read_text(encoding='utf-8')
        relative = path.relative_to(public)
        require('id="series-editor-script"' not in text, f'{relative}: local editing is enabled on the public site')
        require('http://localhost' not in text and 'http://127.0.0.1' not in text,
                f'{relative}: local preview URL leaked into public output')
        require('Zotero-API-Key' not in text and '"annotationPosition"' not in text,
                f'{relative}: private Zotero records leaked into public output')
        if not config.get('extra', {}).get('agent_endpoint'):
            require('id="agent-panel"' not in text, f'{relative}: an unconfigured Agent panel is enabled')
    if errors:
        raise ValueError('\n'.join(errors))
    return {'pages': len(pages), 'links': checked, 'articles': len(index)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--site', type=Path, default=ROOT / 'site', help='Site input directory containing config.toml')
    parser.add_argument('--output', type=Path, help='Built output directory (default: SITE/public)')
    parser.add_argument('--base-url', help='Base URL used when building (overrides config base_url)')
    args = parser.parse_args()
    try:
        result = validate_site(args.site, args.output, args.base_url)
    except (OSError, ValueError) as error:
        raise SystemExit(str(error)) from None
    print(f'PASS: {result["pages"]} pages, {result["links"]} local links/anchors, '
          f'{result["articles"]} searchable article(s), feed, sitemap and static-only output')


if __name__ == '__main__':
    main()
