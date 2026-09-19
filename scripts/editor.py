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


def zola():
    bundled = ROOT / '.tools' / 'zola' / 'zola.exe'
    return os.environ.get('ZOLA') or (str(bundled) if bundled.exists() else (shutil.which('zola') or 'zola'))


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


def generate_resume():
    build_profile(root=ROOT)
    print(f'个人资料源文件：{ROOT / "profile/cv.typ"}', flush=True)
    print(f'本地简历：{ROOT / "output/pdf/resume.pdf"}', flush=True)
    print(f'公开简历：{ROOT / "output/pdf/resume-public.pdf"}', flush=True)
    print(f'网站简历：{ROOT / "site/static/resume.pdf"}', flush=True)
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


def preview(open_browser=True):
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
    prepared_directory = None
    typst = os.environ.get('TYPST', 'typst')

    def refresh_profile():
        try:
            build_profile(root=ROOT)
            # Zola watches content, but may not watch files read by load_data.
            # Updating only mtime triggers a rebuild without rewriting the page.
            (ROOT / 'site/content/about.md').touch()
            print('个人介绍与简历已更新。', flush=True)
            return True
        except Exception as error:
            print(f'个人资料编译失败，保留上一版产物：{error}', file=sys.stderr, flush=True)
            print('修正 profile/ 中的文件并保存后会自动重试。', file=sys.stderr, flush=True)
            return False

    def sync_figures():
        sources = set((ROOT / 'site/content/posts').glob('*/figures/*.typ'))
        for source in list(watchers):
            if source not in sources:
                watchers.pop(source).terminate()
        for source in sources - watchers.keys():
            output = source.parent.parent / (source.stem + '.svg')
            # Finish the first SVG before Zola renders its corresponding page.
            subprocess.run([typst, 'compile', str(source), str(output), '--format', 'svg'], cwd=ROOT, check=False)
            watchers[source] = subprocess.Popen([typst, 'watch', str(source), str(output), '--format', 'svg'], cwd=ROOT)
    try:
        initial_profile = refresh_profile()
        if not initial_profile and not (ROOT / 'site/data/profile.json').exists():
            raise RuntimeError('首次生成个人资料失败，请修正上面的错误后重新运行预览。')
        previous_profile = profile_snapshot()
        profile_changed_at = None
        sync_figures()
        print(f'网页实时预览：http://127.0.0.1:{port}/ （含草稿）', flush=True)
        print('保存 Markdown 或 Typst 后自动编译、刷新网页。Ctrl+C 停止。', flush=True)
        preview_output = (ROOT / '.tools/site-preview').resolve()
        if not preview_output.is_relative_to((ROOT / '.tools').resolve()):
            raise RuntimeError('预览输出目录必须位于博客的 .tools 目录内。')
        scratch = ROOT / '.tools'
        scratch.mkdir(exist_ok=True)
        prepared_directory = tempfile.TemporaryDirectory(prefix='math-preview-', dir=scratch)
        if not Path(prepared_directory.name).resolve().is_relative_to(scratch.resolve()):
            raise RuntimeError('预览输入目录必须位于博客的 .tools 目录内。')
        prepared = PreparedSite(ROOT / 'site', Path(prepared_directory.name) / 'site')
        prepared.sync()
        command = [zola(), '--root', str(prepared.target), 'serve', '--drafts', '--output-dir', str(preview_output), '--force', '--interface', '127.0.0.1', '--port', str(port), '--debounce', '1']
        if open_browser:
            command.append('--open')
        server = subprocess.Popen(command, cwd=ROOT)
        last_sync_error = None
        while server.poll() is None:
            sync_figures()
            current_profile = profile_snapshot()
            if current_profile != previous_profile:
                previous_profile = current_profile
                profile_changed_at = time.monotonic()
            elif profile_changed_at is not None and time.monotonic() - profile_changed_at >= 0.4:
                refresh_profile()
                profile_changed_at = None
            try:
                changed = prepared.sync()
                # load_data inputs may not be watched by Zola itself.
                if any(path.parts[0] == 'data' for path in changed):
                    (prepared.target / 'content/about.md').touch()
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
        children = list(watchers.values()) + ([server] if server else [])
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
    args = parser.parse_args()
    if args.action == 'new':
        try:
            article = create_article(args.slug or '')
        except ValueError as error:
            parser.error(str(error))
        print(f"已创建草稿：{article}", flush=True)
    elif args.action == 'check':
        build()
        subprocess.run([sys.executable, str(ROOT / 'scripts/check_site.py')], cwd=ROOT, check=True)
    elif args.action == 'resume':
        generate_resume()
    else:
        preview()


if __name__ == '__main__':
    main()
