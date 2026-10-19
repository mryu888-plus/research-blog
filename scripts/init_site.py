"""Create an independent research workbench from an explicit public allowlist."""
from __future__ import annotations

import argparse
import ipaddress
import json
from pathlib import Path
import re
import shutil
import stat
import tempfile
from urllib.parse import unquote, urlsplit, urlunsplit


ROOT = Path(__file__).absolute().parents[1]

# These are source components, never a recursive copy of the author's checkout.
SHARED_FILES = (
    'Cargo.toml', 'Cargo.lock', 'ZEN.md',
    'scripts/build_site.py', 'scripts/build_profile.py', 'scripts/check_site.py',
    'scripts/editor.py', 'scripts/math_markdown.py', 'scripts/prepare_site.py',
    'scripts/references.py', 'scripts/sync_zen.cjs', 'scripts/zen.py',
    'scripts/zotero.py', 'scripts/site_features.py', 'scripts/init_site.py',
    'scripts/setup_tools.py',
    'design/visual-system/typst/blog-diagram.typ',
    'extensions/research-blog/package.json',
    'extensions/research-blog/package-lock.json',
    'extensions/research-blog/LICENSE',
    'extensions/research-blog/README.md',
    'extensions/research-blog/CHANGELOG.md',
    'extensions/research-blog/scripts/build.cjs',
    'extensions/research-blog/snippets/blog.code-snippets',
    'extensions/research-notes-theme/package.json',
    'extensions/research-notes-theme/package-lock.json',
    'extensions/research-notes-theme/LICENSE',
    'extensions/research-notes-theme/README.md',
    'extensions/research-notes-theme/CHANGELOG.md',
    'extensions/research-notes-theme/scripts/build.cjs',
    'extensions/research-notes-theme/scripts/preview.template.html',
)
SHARED_PATTERNS = (
    'crates/*/Cargo.toml', 'crates/*/src/*.rs', 'crates/*/tests/*.rs',
    'site/templates/*.html', 'site/templates/tags/*.html',
    'site/templates/shortcodes/*.html',
    'extensions/research-blog/src/*.cjs',
    'extensions/research-blog/test/*.cjs',
)
STATIC_FILES = (
    'zen.css', 'style.css', 'apollo.css', 'main.js', 'preview.js',
    'graph.css', 'graph.js', 'knowledge-core.js', 'knowledge.js',
    'topics.css', 'topics.js', 'series.css', 'series-editor.js',
    'references.css', 'references-page.js', 'about.css', 'about.js', 'favicon.svg',
)
TEMPLATE_FILES = (
    'README.md', 'AGENTS.md', 'CLAUDE.md', '.gitignore', '.vscode/settings.json',
    '.vscode/extensions.json', '.vscode/tasks.json',
    '.github/workflows/pages.yml',
    'site/config.toml', 'site/content/_index.md',
    'site/content/posts/_index.md', 'site/content/about.md',
    'site/content/graph.md', 'site/content/graph-data.md',
    'site/content/search.md', 'site/content/series.md',
    'site/content/references.md', 'site/data/collections.json',
    'site/data/references.json', 'site/content/posts/welcome/index.md',
    'site/content/posts/welcome/figures/style-example.typ',
    'site/content/posts/style-notes/index.md',
    'site/content/posts/draft-note/index.md',
)


def _linked(path: Path) -> bool:
    """Treat Windows junctions and other reparse points like symlinks."""
    info = path.lstat()
    return stat.S_ISLNK(info.st_mode) or bool(
        getattr(info, 'st_file_attributes', 0)
        & getattr(stat, 'FILE_ATTRIBUTE_REPARSE_POINT', 0x400)
    )


def _check_path(path: Path) -> None:
    for component in (path, *path.parents):
        try:
            if _linked(component):
                raise ValueError(f'符号链接或 junction 不能用于初始化：{component}')
        except FileNotFoundError:
            continue


def _text(value: str, label: str) -> str:
    value = value.strip()
    if not value or any(ord(character) < 32 for character in value):
        raise ValueError(f'{label}不能为空或包含控制字符。')
    if len(value) > 200:
        raise ValueError(f'{label}不能超过 200 个字符。')
    return value


def normalize_url(value: str) -> str:
    """Accept public HTTP(S) site roots, including a GitHub Pages subpath."""
    if not value or any(character.isspace() or ord(character) < 32 for character in value):
        raise ValueError('网站 URL 不能为空或包含空格、控制字符。')
    if '\\' in value:
        raise ValueError('网站 URL 不能包含反斜杠。')
    try:
        parsed = urlsplit(value)
        hostname, port = parsed.hostname, parsed.port
    except ValueError as error:
        raise ValueError('网站 URL 格式不正确。') from error
    if (parsed.scheme not in {'https', 'http'} or not hostname
            or parsed.username is not None or parsed.password is not None
            or parsed.query or parsed.fragment):
        raise ValueError('网站 URL 必须是无凭据、查询或片段的 HTTP(S) 地址。')
    try:
        ipaddress.ip_address(hostname)
    except ValueError:
        labels = hostname.encode('idna').decode('ascii').split('.')
        if any(not re.fullmatch(r'[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?', label)
               for label in labels):
            raise ValueError('网站 URL 的域名格式不正确。')
    decoded = parsed.path
    for _ in range(5):
        previous, decoded = decoded, unquote(decoded)
        if previous == decoded:
            break
    if decoded != unquote(decoded):
        raise ValueError('网站 URL 路径包含过多嵌套转义。')
    if ('\\' in decoded or any(ord(character) < 32 for character in decoded)
            or any(segment in {'.', '..'} for segment in decoded.split('/'))):
        raise ValueError('网站 URL 路径不能包含目录跳转或控制字符。')
    # Accessing .port above also verifies the range. Preserve explicit ports.
    del port
    return urlunsplit((parsed.scheme, parsed.netloc, parsed.path.rstrip('/') + '/', '', ''))


def _copy_plan(source: Path) -> dict[str, Path]:
    _check_path(source)
    source = source.resolve()
    plan: dict[str, Path] = {}
    for relative in (*SHARED_FILES, *(f'site/static/{name}' for name in STATIC_FILES)):
        filename = source / relative
        _check_path(filename)
        if not filename.is_file():
            raise ValueError(f'模板缺少必需的源文件：{relative}')
        plan[relative] = filename
    for pattern in SHARED_PATTERNS:
        filenames = sorted(source.glob(pattern))
        if not filenames and '*' in pattern and pattern not in {
            'crates/*/tests/*.rs', 'site/templates/shortcodes/*.html',
        }:
            raise ValueError(f'模板缺少必需的源组件：{pattern}')
        for filename in filenames:
            _check_path(filename)
            if filename.is_file():
                plan[filename.relative_to(source).as_posix()] = filename
    # Only this dedicated public template supplies content and local settings.
    template = source / 'examples/starter'
    _check_path(template)
    if not template.is_dir():
        raise ValueError('缺少 examples/starter 模板目录。')
    for relative in TEMPLATE_FILES:
        filename = template / relative
        _check_path(filename)
        if not filename.is_file():
            raise ValueError(f'模板缺少必需的示例文件：{relative}')
        plan[relative] = filename
        # Keep the tiny public template resources so the generated workbench
        # can initialize another clean workspace with the same command.
        plan[f'examples/starter/{relative}'] = filename
    for relative in ('LICENSE', 'CONTENT_LICENSE.md', 'CONTRIBUTING.md',
                     'THIRD_PARTY_NOTICES.md', 'SECURITY.md',
                     'scripts/check_starter.py',
                     'docs/getting-started.md', 'docs/style-guide.md',
                     'docs/architecture.md', 'docs/releasing.md'):
        filename = source / relative
        _check_path(filename)
        if filename.is_file():
            plan[relative] = filename
    return plan


def initialize(destination: Path | str, *, name: str, url: str,
               author: str = 'Researcher', source: Path = ROOT,
               dry_run: bool = False) -> dict:
    name, author = _text(name, '网站标题'), _text(author, '作者名称')
    url = normalize_url(url)
    source = Path(source).absolute()
    destination = Path(destination).absolute()
    _check_path(source)
    _check_path(destination)
    source, destination = source.resolve(), destination.resolve()
    if source == destination or source.is_relative_to(destination):
        raise ValueError('目标不能是源项目或源项目的祖先目录。')
    if destination.exists() and (not destination.is_dir() or any(destination.iterdir())):
        raise ValueError(f'目标必须不存在或为空目录，不会覆盖已有文件：{destination}')
    plan = _copy_plan(source)
    result = {'destination': str(destination), 'name': name, 'url': url,
              'author': author, 'files': len(plan), 'dry_run': dry_run}
    if dry_run:
        return result

    destination.parent.mkdir(parents=True, exist_ok=True)
    _check_path(destination.parent)
    stage = Path(tempfile.mkdtemp(prefix='.research-blog-init-', dir=destination.parent))
    try:
        for relative, filename in sorted(plan.items()):
            _check_path(filename)
            target = stage / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(filename, target)
        config = stage / 'site/config.toml'
        content = config.read_text(encoding='utf-8')
        replacements = {
            '__SITE_TITLE__': json.dumps(name, ensure_ascii=False),
            '__SITE_URL__': json.dumps(url, ensure_ascii=False),
            '__SITE_AUTHOR__': json.dumps(author, ensure_ascii=False),
        }
        # The template placeholders occupy complete TOML values. Do one pass so
        # a user's literal placeholder-looking title is never interpolated.
        content = re.sub('|'.join(re.escape(key) for key in replacements),
                         lambda match: replacements[match[0]], content)
        config.write_text(content, encoding='utf-8')
        _check_path(destination)
        if destination.exists():
            if not destination.is_dir() or any(destination.iterdir()):
                raise ValueError('初始化期间目标目录发生变化；没有覆盖已有文件。')
            destination.rmdir()  # Remove only the verified empty directory.
        stage.rename(destination)
    finally:
        if stage.exists():
            if stage.parent.resolve() != destination.parent.resolve() or _linked(stage):
                raise RuntimeError('拒绝清理不属于此次初始化的路径。')
            shutil.rmtree(stage)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path, help='新项目路径；必须不存在或为空')
    parser.add_argument('--name', required=True, help='网站标题')
    parser.add_argument('--url', required=True, help='公开网站根地址，支持子路径')
    parser.add_argument('--author', default='Researcher', help='公开显示的作者名称')
    parser.add_argument('--dry-run', action='store_true', help='检查路径并列出摘要，不写文件')
    args = parser.parse_args()
    try:
        result = initialize(args.destination, name=args.name, url=args.url,
                            author=args.author, dry_run=args.dry_run)
    except (OSError, ValueError, RuntimeError) as error:
        parser.exit(1, f'初始化失败：{error}\n')
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
