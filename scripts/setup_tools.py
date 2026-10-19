"""Install or check the pinned build tools: Zola 0.23.4 and Typst 0.15.1.

    python scripts/setup_tools.py            # report, then download what is missing into .tools/
    python scripts/setup_tools.py --check    # report only; never downloads
    python scripts/setup_tools.py --force    # download again even if the right version is present
    python scripts/setup_tools.py --json     # machine-readable summary on stdout

The build and preview scripts look for each tool in this order: the ZOLA or
TYPST environment variable, .tools/<tool>/ inside this project, then PATH. A
copy in .tools/ therefore takes precedence over a different version on PATH.
Downloads come from the official GitHub releases, are checked against the
SHA-256 hashes pinned below, and are unpacked only inside .tools/. Python,
Node.js, npm and a Chinese font are reported but never installed by this script.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from site_features import tool_command  # noqa: E402,F401

ROOT = Path(__file__).resolve().parents[1]
PYTHON_MIN = (3, 11)
NODE_MIN = 18
CHUNK = 1024 * 1024

# Official release assets, with SHA-256 hashes computed from them on 2026-10-13.
RELEASES = {
    'zola': {
        'version': '0.23.4',
        'base': 'https://github.com/getzola/zola/releases/download/v0.23.4/',
        'assets': {
            ('windows', 'x86_64'): ('zola-v0.23.4-x86_64-pc-windows-msvc.zip',
                                    'dc839498e27b631f60fd5e1527935198b7f002eb09b8446a1e2913a8fa386925'),
            # Zola has no Windows arm64 build; the x86_64 binary runs under emulation.
            ('windows', 'aarch64'): ('zola-v0.23.4-x86_64-pc-windows-msvc.zip',
                                     'dc839498e27b631f60fd5e1527935198b7f002eb09b8446a1e2913a8fa386925'),
            ('linux', 'x86_64'): ('zola-v0.23.4-x86_64-unknown-linux-gnu.tar.gz',
                                  '54d1a347781b2f32330914fcc02def81c7e3ddb6111b36d1cc89c06557aed1de'),
            ('linux-musl', 'x86_64'): ('zola-v0.23.4-x86_64-unknown-linux-musl.tar.gz',
                                       'd99c51302ebbf909a0d83d4319d4d745b56a93dc49c4a69878c0f0dcaa4c8531'),
            ('linux', 'aarch64'): ('zola-v0.23.4-aarch64-unknown-linux-gnu.tar.gz',
                                   '21bb37a4f3bbac663cf8f04df9b51ac6bc154acfe2cf2c3e9ea162b4951487b6'),
            ('darwin', 'x86_64'): ('zola-v0.23.4-x86_64-apple-darwin.tar.gz',
                                   'e79edcba2e8d03d22065c9cb8fa2e3abf07b823ef17f00abdc060188dceabba7'),
            ('darwin', 'aarch64'): ('zola-v0.23.4-aarch64-apple-darwin.tar.gz',
                                    '303b8e1f3251a6250e47f811eda143316f653c22201faa66777d48ac499c0ee3'),
        },
    },
    'typst': {
        'version': '0.15.1',
        'base': 'https://github.com/typst/typst/releases/download/v0.15.1/',
        'assets': {
            ('windows', 'x86_64'): ('typst-x86_64-pc-windows-msvc.zip',
                                    '19ce3551153c2fe7ee9fa2f95208310c8f4d3209fedb699e0333faf8913f6736'),
            ('windows', 'aarch64'): ('typst-aarch64-pc-windows-msvc.zip',
                                     '4ab28e1b71ec3184d38d580ab797f499b6770d952b6b19167be5cea5c2662e14'),
            # Typst ships static musl binaries for Linux; they run on glibc systems too.
            ('linux', 'x86_64'): ('typst-x86_64-unknown-linux-musl.tar.xz',
                                  'a6d077d0a95eed5a2eba715b2dae06be954f624ccbf85758a03f389ded33118c'),
            ('linux-musl', 'x86_64'): ('typst-x86_64-unknown-linux-musl.tar.xz',
                                       'a6d077d0a95eed5a2eba715b2dae06be954f624ccbf85758a03f389ded33118c'),
            ('linux', 'aarch64'): ('typst-aarch64-unknown-linux-musl.tar.xz',
                                   '5aa8d74a3d906e60ea12a66ac2f37f8eef1b14cbad7182a745e393a10c23dcee'),
            ('linux-musl', 'aarch64'): ('typst-aarch64-unknown-linux-musl.tar.xz',
                                        '5aa8d74a3d906e60ea12a66ac2f37f8eef1b14cbad7182a745e393a10c23dcee'),
            ('darwin', 'x86_64'): ('typst-x86_64-apple-darwin.tar.xz',
                                   '7f9fdd9584866245de9a79e0add8f9236fae6f40a8a45e2c4771ccc14db4e0fa'),
            ('darwin', 'aarch64'): ('typst-aarch64-apple-darwin.tar.xz',
                                    '48f62ed034aa3a7978309579ac6ca00045e2ef0da73114e8af27cfd8e74dc05a'),
        },
    },
}
CJK_FONT_FILES = ('msyh.ttc', 'msyh.ttf', 'msyhbd.ttc', 'simhei.ttf', 'simsun.ttc',
                  'NotoSansCJK-Regular.ttc', 'NotoSansSC-Regular.otf', 'SourceHanSansSC-Regular.otf')


def platform_key():
    """('windows' | 'linux' | 'linux-musl' | 'darwin', 'x86_64' | 'aarch64')."""
    system = platform.system().lower()
    machine = platform.machine().lower()
    arch = {'amd64': 'x86_64', 'x86_64': 'x86_64', 'x64': 'x86_64',
            'arm64': 'aarch64', 'aarch64': 'aarch64'}.get(machine)
    if system == 'linux' and platform.libc_ver()[0] != 'glibc':
        system = 'linux-musl'
    if system not in {'windows', 'linux', 'linux-musl', 'darwin'} or arch is None:
        raise SystemExit(f'没有 {platform.system()} {platform.machine()} 的预编译版本，请自己安装 Zola 和 Typst 并放进 PATH。')
    return system, arch


def asset_for(name, key=None):
    """The release asset and hash for a tool on this platform.

    A musl Linux falls back to the glibc build when no musl build is pinned.
    """
    key = key or platform_key()
    assets = RELEASES[name]['assets']
    if key not in assets and key[0] == 'linux-musl':
        key = ('linux', key[1])
    if key not in assets:
        raise SystemExit(f'没有 {name} 在 {key[0]} {key[1]} 上的预编译版本。')
    return assets[key]


def parse_version(output):
    match = re.search(r'(\d+\.\d+\.\d+)', output or '')
    return match.group(1) if match else None


def run_version(command, flag='--version'):
    try:
        result = subprocess.run([command, flag], capture_output=True, text=True, timeout=20)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return parse_version(result.stdout + result.stderr)


def binary_name(name):
    return name + ('.exe' if os.name == 'nt' else '')


def locate(name, root):
    """Where the build scripts would find the tool now, and which source wins."""
    override = os.environ.get(name.upper())
    if override:
        return override, 'env'
    bundled = Path(root) / '.tools' / name / binary_name(name)
    if bundled.is_file():
        return str(bundled), 'tools'
    found = shutil.which(name)
    return (found, 'path') if found else (None, None)


def tool_status(name, root):
    command, source = locate(name, root)
    required = RELEASES[name]['version']
    found = run_version(command) if command else None
    return {'tool': name, 'required': required, 'found': found, 'path': command,
            'source': source, 'ok': found == required}


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as handle:
        for chunk in iter(lambda: handle.read(CHUNK), b''):
            digest.update(chunk)
    return digest.hexdigest()


def download(url, target, quiet=False):
    target.parent.mkdir(parents=True, exist_ok=True)
    partial = target.with_name(target.name + '.part')
    request = urllib.request.Request(url, headers={'User-Agent': 'research-blog-setup'})
    with urllib.request.urlopen(request, timeout=120) as response, open(partial, 'wb') as handle:
        total = int(response.headers.get('Content-Length') or 0)
        done = 0
        while True:
            chunk = response.read(CHUNK)
            if not chunk:
                break
            handle.write(chunk)
            done += len(chunk)
            if not quiet and total:
                print(f'\r  下载 {target.name}  {done / 1e6:.0f} / {total / 1e6:.0f} MB',
                      end='', file=sys.stderr, flush=True)
    if not quiet:
        print(file=sys.stderr)
    partial.replace(target)


def check_members(archive, names):
    for name in names:
        path = Path(name.replace('\\', '/'))
        if path.drive or path.root or '..' in path.parts:
            raise SystemExit(f'拒绝解压带有越界路径的压缩包：{archive.name} 里的 {name}')


def extract(archive, destination):
    """Unpack into destination; archive members may not escape it or be links."""
    archive, destination = Path(archive), Path(destination)
    if archive.name.endswith('.zip'):
        with zipfile.ZipFile(archive) as bundle:
            check_members(archive, bundle.namelist())
            bundle.extractall(destination)
        return
    with tarfile.open(archive) as bundle:
        members = bundle.getmembers()
        check_members(archive, [member.name for member in members])
        regular = [member for member in members if member.isfile() or member.isdir()]
        try:
            bundle.extractall(destination, members=regular, filter='data')
        except TypeError:  # a Python without the extraction filter
            bundle.extractall(destination, members=regular)


def find_binary(folder, name):
    wanted = binary_name(name)
    matches = sorted(path for path in Path(folder).rglob(wanted) if path.is_file())
    if not matches:
        raise SystemExit(f'压缩包里没有 {wanted}。')
    return matches[0]


def install(name, root, *, proxy_prefix='', quiet=False):
    """Download, verify and unpack one tool into <root>/.tools/<name>/."""
    asset, digest = asset_for(name)
    tools = Path(root) / '.tools'
    archive = tools / 'downloads' / asset
    if not archive.is_file() or sha256(archive) != digest:
        download(proxy_prefix + RELEASES[name]['base'] + asset, archive, quiet=quiet)
    actual = sha256(archive)
    if actual != digest:
        archive.unlink(missing_ok=True)
        raise SystemExit(f'{asset} 的 SHA-256 和固定值不符，已删除下载文件。\n  期望 {digest}\n  实际 {actual}')
    target_dir = tools / name
    target_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=f'{name}-unpack-', dir=tools) as temporary:
        extract(archive, temporary)
        binary = find_binary(temporary, name)
        target = target_dir / binary.name
        shutil.copyfile(binary, target)
        for pattern in ('LICENSE*', 'NOTICE*'):
            for notice in binary.parent.glob(pattern):
                shutil.copyfile(notice, target_dir / notice.name)
    if os.name != 'nt':
        target.chmod(target.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    found = run_version(str(target))
    if found != RELEASES[name]['version']:
        raise SystemExit(f'{target} 运行后报告的版本是 {found}，不是 {RELEASES[name]["version"]}。')
    return str(target)


def runtime_status():
    python_ok = sys.version_info[:2] >= PYTHON_MIN
    node_path = shutil.which('node')
    node_version = run_version(node_path) if node_path else None
    node_ok = bool(node_version) and int(node_version.split('.')[0]) >= NODE_MIN
    npm_path = shutil.which('npm')
    return [
        {'tool': 'python', 'required': f'{PYTHON_MIN[0]}.{PYTHON_MIN[1]}+', 'found': platform.python_version(),
         'path': sys.executable, 'source': 'path', 'ok': python_ok},
        {'tool': 'node', 'required': f'{NODE_MIN}+', 'found': node_version, 'path': node_path,
         'source': 'path' if node_path else None, 'ok': node_ok},
        {'tool': 'npm', 'required': '任意', 'found': run_version(npm_path) if npm_path else None, 'path': npm_path,
         'source': 'path' if npm_path else None, 'ok': bool(npm_path)},
    ]


def cjk_font_present():
    """True or False when it can be checked, None when it cannot."""
    system = platform.system().lower()
    if system == 'windows':
        fonts = Path(os.environ.get('WINDIR', r'C:\Windows')) / 'Fonts'
        return any((fonts / name).exists() for name in CJK_FONT_FILES)
    fc_list = shutil.which('fc-list')
    if fc_list:
        try:
            output = subprocess.run([fc_list, ':lang=zh', 'family'], capture_output=True, text=True, timeout=20).stdout
        except (OSError, subprocess.TimeoutExpired):
            return None
        return bool(output.strip())
    if system == 'darwin':
        return Path('/System/Library/Fonts/PingFang.ttc').exists() or None
    return None


def describe(entry):
    mark = 'ok ' if entry['ok'] else '-- '
    found = entry['found'] or '未找到'
    where = {'env': '环境变量', 'tools': '.tools', 'path': 'PATH'}.get(entry['source'], '')
    return f"{mark}{entry['tool']:<7} 需要 {entry['required']:<8} 找到 {found:<10} {where} {entry['path'] or ''}".rstrip()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--check', action='store_true', help='只报告，不下载')
    parser.add_argument('--force', action='store_true', help='即使版本正确也重新下载到 .tools/')
    parser.add_argument('--json', action='store_true', help='在标准输出打印 JSON 摘要')
    parser.add_argument('--proxy-prefix', default=os.environ.get('RESEARCH_BLOG_DOWNLOAD_PREFIX', ''),
                        help='加在下载地址前面的镜像前缀，例如 https://mirror.example/ ；'
                             '也可以用环境变量 RESEARCH_BLOG_DOWNLOAD_PREFIX')
    parser.add_argument('--root', type=Path, default=ROOT, help='项目目录，默认是脚本所在的项目')
    args = parser.parse_args(argv)
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, 'reconfigure'):
            stream.reconfigure(encoding='utf-8')
    root = args.root.resolve()
    quiet = args.json

    report = {'root': str(root), 'platform': list(platform_key()), 'tools': runtime_status(),
              'installed': [], 'font': cjk_font_present()}
    for name in RELEASES:
        status = tool_status(name, root)
        if not args.check and (args.force or not status['ok']):
            if status['source'] == 'env' and not args.force:
                status['note'] = f'环境变量 {name.upper()} 指向了版本不对的程序，先改或删掉它。'
            else:
                if not quiet:
                    print(f"安装 {name} {RELEASES[name]['version']} 到 {root / '.tools' / name}", file=sys.stderr)
                install(name, root, proxy_prefix=args.proxy_prefix, quiet=quiet)
                report['installed'].append(name)
                status = tool_status(name, root)
        report['tools'].append(status)
    report['ok'] = all(entry['ok'] for entry in report['tools'])

    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        for entry in report['tools']:
            print(describe(entry))
            if entry.get('note'):
                print('   ' + entry['note'])
        font = report['font']
        if font:
            print('ok  中文字体 已找到')
        elif font is False:
            print('--  中文字体 没找到，Linux 可以装 fonts-noto-cjk')
        else:
            print('?   中文字体 这里没法检查，Typst 图里有中文时需要一种中文字体')
        if report['installed']:
            print('已安装到 .tools/：' + '、'.join(report['installed']) + '。构建和预览脚本会自动使用这里的版本。')
        if not report['ok']:
            missing = [entry['tool'] for entry in report['tools'] if not entry['ok']]
            hint = '。去掉 --check 可以下载 Zola 和 Typst。' if args.check else '。Python 和 Node.js 需要自己安装。'
            print('还缺：' + '、'.join(missing) + hint)
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
