"""VS Code task entrypoints for this blog (no shell interpolation)."""
import argparse
import json
import re
from datetime import date
from pathlib import Path
import shutil
import subprocess
import sys
import socket
import time
from build_site import ROOT, build


def zola():
    bundled = ROOT / '.tools' / 'zola' / 'zola.exe'
    return str(bundled) if bundled.exists() else (shutil.which('zola') or 'zola')


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
    def sync_figures():
        sources = set((ROOT / 'site/content/posts').glob('*/figures/*.typ'))
        for source in list(watchers):
            if source not in sources:
                watchers.pop(source).terminate()
        for source in sources - watchers.keys():
            output = source.parent.parent / (source.stem + '.svg')
            # Finish the first SVG before Zola renders its corresponding page.
            subprocess.run(['typst', 'compile', str(source), str(output), '--format', 'svg'], cwd=ROOT, check=False)
            watchers[source] = subprocess.Popen(['typst', 'watch', str(source), str(output), '--format', 'svg'], cwd=ROOT)
    try:
        sync_figures()
        print(f'网页实时预览：http://127.0.0.1:{port}/ （含草稿）', flush=True)
        print('保存 Markdown 或 Typst 后自动编译、刷新网页。Ctrl+C 停止。', flush=True)
        command = [zola(), '--root', str(ROOT / 'site'), 'serve', '--drafts', '--interface', '127.0.0.1', '--port', str(port), '--debounce', '1']
        if open_browser:
            command.append('--open')
        server = subprocess.Popen(command, cwd=ROOT)
        while server.poll() is None:
            sync_figures()
            time.sleep(1)
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


def main():
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=['new', 'preview', 'check'])
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
    else:
        preview()


if __name__ == '__main__':
    main()
