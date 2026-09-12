"""Validate built links, anchors, search data and public-only assets."""
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urljoin, urlsplit, unquote
import json
import tomllib

ROOT = Path(__file__).resolve().parents[1]
PUBLIC = ROOT / 'site' / 'public'
config = tomllib.loads((ROOT / 'site' / 'config.toml').read_text(encoding='utf-8-sig'))
base = config['base_url'].rstrip('/') + '/'
base_parts = urlsplit(base)

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

json_routes = {'search-index/index.html', 'graph-data/index.html'}
pages = {p: Page(p) for p in PUBLIC.rglob('*.html') if p.relative_to(PUBLIC).as_posix() not in json_routes}
errors = []
checked = 0
for path, page in pages.items():
    relative = path.relative_to(PUBLIC).as_posix()
    page_url = urljoin(base, relative.removesuffix('index.html'))
    for link in page.links:
        parts = urlsplit(urljoin(page_url, link))
        if parts.netloc != base_parts.netloc or parts.scheme not in ('http', 'https'):
            continue
        if not parts.path.startswith(base_parts.path):
            errors.append(f'{relative}: escapes Pages prefix: {link}')
            continue
        local = PUBLIC / unquote(parts.path[len(base_parts.path):])
        if local.is_dir():
            local /= 'index.html'
        if not local.is_file():
            errors.append(f'{relative}: missing target: {link}')
        elif parts.fragment and local in pages and unquote(parts.fragment) not in pages[local].ids:
            errors.append(f'{relative}: missing anchor: {link}')
        checked += 1

index = json.loads((PUBLIC / 'search-index' / 'index.html').read_text(encoding='utf-8-sig'))
assert index and any(post['title'] == '从轨迹中提取可复用技能' for post in index)
assert '从轨迹中提取可复用技能' in (PUBLIC / 'index.html').read_text(encoding='utf-8')
assert (PUBLIC / 'atom.xml').is_file() and (PUBLIC / 'sitemap.xml').is_file()
assert (PUBLIC / '404.html').is_file()
assert (PUBLIC / 'graph' / 'index.html').is_file()
graph = json.loads((PUBLIC / 'graph-data' / 'index.html').read_text(encoding='utf-8-sig'))
assert isinstance(graph, list) and len({post['id'] for post in graph}) == len(graph)
for post in graph:
    assert not post['draft'], f"Draft leaked into graph: {post['id']}"
    parts = urlsplit(post['url'])
    assert parts.netloc == base_parts.netloc and parts.path.startswith(base_parts.path)
    target = PUBLIC / unquote(parts.path[len(base_parts.path):])
    if target.is_dir():
        target /= 'index.html'
    assert target.is_file(), f"Missing graph article: {post['url']}"
    assert isinstance(post['html'], str) and isinstance(post['headings'], list)
assert not list(PUBLIC.rglob('article-manifest.json'))
assert not list(PUBLIC.rglob('.env*'))
for path in pages:
    text = path.read_text(encoding='utf-8')
    assert 'http://localhost' not in text and 'http://127.0.0.1' not in text
    if not config['extra']['agent_endpoint']:
        assert 'id="agent-panel"' not in text
if errors:
    raise SystemExit('\n'.join(errors))
print(f'PASS: {len(pages)} pages, {checked} local links/anchors, {len(index)} searchable article(s), feed, sitemap and static-only output')
