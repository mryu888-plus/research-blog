"""VS Code task entrypoints for this blog (no shell interpolation)."""
import argparse
import json
import os
import re
from datetime import date
from pathlib import Path
import shutil
import subprocess
import sys
import socket
import time
import tempfile
from build_site import ROOT, build
from build_profile import build_profile
from prepare_site import PreparedSite
from zen import sync_design
from site_features import profile_enabled, read_config, require_profile_source, tool_command


def zola(root=ROOT):
    return tool_command('zola', root)


def profile_snapshot(root=ROOT):
    """Watch imports and assets as well as the main Typst source."""
    folder = root / 'profile'
    snapshot = {}
    for path in folder.rglob('*'):
        relative = path.relative_to(folder)
        if any(part in {'.git', '__pycache__', '.cache', 'output', 'build'} for part in relative.parts):
            continue
        # Tinymist may emit this beside cv.typ; our outputs live outside profile/.
        if relative.as_posix() == 'cv.pdf':
            continue
        try:
            if path.is_file():
                info = path.stat()
                snapshot[relative.as_posix()] = (info.st_mtime_ns, info.st_size)
        except FileNotFoundError:
            # Editors may replace a file atomically while we scan it.
            continue
    return snapshot


def generate_resume(root=ROOT):
    root = Path(root).resolve()
    if not profile_enabled(read_config(root / 'site')):
        raise ValueError('个人资料功能未启用。请先添加 profile/cv.typ，并在 site/config.toml 中设置 extra.profile_enabled = true。')
    require_profile_source(root)
    build_profile(root=root)
    print(f'个人资料源文件：{root / "profile/cv.typ"}', flush=True)
    print(f'本地简历：{root / "output/pdf/resume.pdf"}', flush=True)
    print(f'公开简历：{root / "output/pdf/resume-public.pdf"}', flush=True)
    print(f'网站简历：{root / "site/static/resume.pdf"}', flush=True)
    print('个人介绍数据已更新；运行预览或构建网站即可查看 /about/。', flush=True)


def create_article(title, posts_dir=None):
    title = title.strip()
    if not title:
        raise ValueError("请输入文章标题，支持中文。")
    # Preserve readable titles, replacing only characters unsafe in Windows paths.
    slug = re.sub(r'[<>:"/\\|?*\x00-\x1f]', '-', title).strip(' .-')
    slug = slug[:80].rstrip(' .')
    if not slug:
        raise ValueError("标题需要包含可用于目录名的文字或数字。")
    if re.fullmatch(r"(?i)(con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])", slug.split('.')[0]):
        slug = "文章-" + slug
    posts = Path(posts_dir) if posts_dir is not None else ROOT / 'site/content/posts'
    folder = posts / slug
    posts.mkdir(parents=True, exist_ok=True)
    try:
        folder.mkdir()
    except FileExistsError:
        raise ValueError(f"这篇文章已经存在，请直接打开：{folder / 'index.md'}") from None
    # JSON string escaping is valid for TOML basic strings. Escape '+' so a
    # title containing '+++' cannot terminate the existing frontmatter parser.
    quoted = json.dumps(title, ensure_ascii=False).replace('+', r"\u002b")
    text = f'''+++
title = {quoted}
date = {date.today().isoformat()}
description = ""
draft = true

[taxonomies]
tags = []
+++

## 问题与背景

在这里开始写作。

## 我的思考


## 总结

'''
    (folder / 'figures').mkdir()
    article = folder / 'index.md'
    article.write_text(text, encoding='utf-8')
    return article


def trigger_content_reload(site):
    """Prompt a Zola rebuild after load_data changes without creating a page."""
    content = Path(site) / 'content'
    entry = content / '_index.md'
    if not entry.is_file():
        entry = next(content.rglob('*.md'), None)
    if entry is not None:
        entry.touch()


def preview(open_browser=True, *, root=ROOT):
    root = Path(root).resolve()
    site = root / 'site'
    enabled = profile_enabled(read_config(site))
    if enabled:
        require_profile_source(root)
    sync_design(root=root)
    previous_zen = (root / 'ZEN.md').stat().st_mtime_ns
    port = None
    for candidate in range(1111, 1121):
        with socket.socket() as probe:
            try:
                probe.bind(('127.0.0.1', candidate))
                port = candidate
                break
            except OSError:
                continue
    if port is None:
        raise RuntimeError('本地预览端口 1111–1120 均被占用，请停止之前的预览任务。')
    watchers = {}
    server = None
    editor_server = None
    prepared_directory = None
    typst = tool_command('typst', root)

    def refresh_profile():
        try:
            require_profile_source(root)
            build_profile(root=root)
            # Zola watches content, but may not watch files read by load_data.
            # Updating only mtime triggers a rebuild without rewriting the page.
            trigger_content_reload(site)
            print('个人介绍与简历已更新。', flush=True)
            return True
        except Exception as error:
            print(f'个人资料编译失败，保留上一版产物：{error}', file=sys.stderr, flush=True)
            print('修正 profile/ 中的文件并保存后会自动重试。', file=sys.stderr, flush=True)
            return False

    def sync_figures():
        sources = set((site / 'content/posts').rglob('figures/*.typ'))
        for source in list(watchers):
            if source not in sources:
                watchers.pop(source).terminate()
        for source in sources - watchers.keys():
            output = source.parent.parent / (source.stem + '.svg')
            # Finish the first SVG before Zola renders its corresponding page.
            subprocess.run([typst, 'compile', str(source), str(output), '--root', str(root), '--format', 'svg'], cwd=root, check=False)
            watchers[source] = subprocess.Popen([typst, 'watch', str(source), str(output), '--root', str(root), '--format', 'svg'], cwd=root)
    try:
        initial_profile = refresh_profile() if enabled else True
        if not initial_profile and not (site / 'data/profile.json').exists():
            raise RuntimeError('首次生成个人资料失败，请修正上面的错误后重新运行预览。')
        previous_profile = profile_snapshot(root)
        profile_changed_at = None
        sync_figures()
        print(f'网页实时预览：http://127.0.0.1:{port}/ （含草稿）', flush=True)
        print('保存 Markdown 或 Typst 后自动编译、刷新网页。Ctrl+C 停止。', flush=True)
        preview_output = (root / '.tools/site-preview').resolve()
        if not preview_output.is_relative_to((root / '.tools').resolve()):
            raise RuntimeError('预览输出目录必须位于博客的 .tools 目录内。')
        scratch = root / '.tools'
        scratch.mkdir(exist_ok=True)
        prepared_directory = tempfile.TemporaryDirectory(prefix='math-preview-', dir=scratch)
        if not Path(prepared_directory.name).resolve().is_relative_to(scratch.resolve()):
            raise RuntimeError('预览输入目录必须位于博客的 .tools 目录内。')
        prepared = PreparedSite(site, Path(prepared_directory.name) / 'site')
        prepared.sync()
        editor_entry = root / 'extensions/research-blog/dist/preview-server.cjs'
        if not editor_entry.is_file():
            raise RuntimeError('请先运行 npm --prefix extensions/research-blog run build，生成预览编辑服务。')
        editor_server = subprocess.Popen(
            [os.environ.get('NODE') or shutil.which('node') or 'node', str(editor_entry),
             str(site / 'content'), f'http://127.0.0.1:{port}'],
            cwd=root, stdout=subprocess.PIPE, text=True, encoding='utf-8')
        editor_config = editor_server.stdout.readline()
        if not editor_config:
            raise RuntimeError('本地系列编辑服务未能启动，请查看上面的错误。')
        # Session credentials exist only in the private serve input, never in
        # site/static or the public build. PreparedSite leaves unmanaged files alone.
        (prepared.target / 'static/preview-editor.json').write_text(
            json.dumps(json.loads(editor_config)), encoding='utf-8')
        command = [zola(root), '--root', str(prepared.target), 'serve', '--drafts', '--output-dir', str(preview_output), '--force', '--interface', '127.0.0.1', '--port', str(port), '--debounce', '1']
        if open_browser:
            command.append('--open')
        server = subprocess.Popen(command, cwd=root)
        last_sync_error = None
        while server.poll() is None:
            current_zen = (root / 'ZEN.md').stat().st_mtime_ns
            if current_zen != previous_zen:
                previous_zen = current_zen
                try:
                    sync_design(root=root)
                except (OSError, subprocess.CalledProcessError, RuntimeError) as error:
                    print(f'ZEN 更新失败，请修正 ZEN.md 后保存重试：{error}', file=sys.stderr, flush=True)
            sync_figures()
            try:
                current_enabled = profile_enabled(read_config(site))
            except (OSError, ValueError):
                # A config edit may briefly be incomplete; PreparedSite reports
                # the render error while we retain the previous feature state.
                current_enabled = enabled
            if current_enabled and not enabled:
                refresh_profile()
            enabled = current_enabled
            current_profile = profile_snapshot(root)
            if current_profile != previous_profile:
                previous_profile = current_profile
                profile_changed_at = time.monotonic()
            elif profile_changed_at is not None and time.monotonic() - profile_changed_at >= 0.4:
                if enabled:
                    refresh_profile()
                profile_changed_at = None
            try:
                changed = prepared.sync()
                # load_data inputs may not be watched by Zola itself.
                if any(path.parts[0] == 'data' for path in changed):
                    trigger_content_reload(prepared.target)
                last_sync_error = None
            except (OSError, ValueError) as error:
                # An editor can atomically replace a file during our scan.
                # Retry next tick, retaining the last valid preview meanwhile.
                if str(error) != last_sync_error:
                    print(f'预览更新暂缓：{error}', file=sys.stderr, flush=True)
                    last_sync_error = str(error)
            time.sleep(0.1)
        if server.returncode:
            raise RuntimeError(f'网页预览退出，错误码 {server.returncode}，请查看上面的编译错误。')
    except KeyboardInterrupt:
        pass
    finally:
        children = list(watchers.values()) + ([server] if server else []) + ([editor_server] if editor_server else [])
        for child in children:
            if child.poll() is None:
                child.terminate()
        for child in children:
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
        if prepared_directory is not None:
            prepared_directory.cleanup()


def main():
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=['new', 'preview', 'check', 'resume'])
    parser.add_argument('slug', nargs='?', help='文章标题，支持中文、空格和标点')
    parser.add_argument('--root', type=Path, default=ROOT,
                        help='Project root containing site/, scripts/ and ZEN.md')
    parser.add_argument('--no-open', action='store_true', help='Do not open a browser for preview')
    args = parser.parse_args()
    root = args.root.resolve()
    if args.action == 'new':
        try:
            article = create_article(args.slug or '', posts_dir=root / 'site/content/posts')
        except ValueError as error:
            parser.error(str(error))
        print(f"已创建草稿：{article}", flush=True)
    elif args.action == 'check':
        build(root=root)
        subprocess.run([sys.executable, str(root / 'scripts/check_site.py'), '--site', str(root / 'site')], cwd=root, check=True)
    elif args.action == 'resume':
        try:
            generate_resume(root)
        except ValueError as error:
            parser.error(str(error))
    else:
        preview(open_browser=not args.no_open, root=root)


if __name__ == '__main__':
    main()
