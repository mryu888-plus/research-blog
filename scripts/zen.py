"""Synchronize the ZEN design source before site builds and previews."""
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def sync_design(root=ROOT):
    root = Path(root).resolve()
    node = os.environ.get('NODE') or shutil.which('node')
    if not node:
        raise RuntimeError('ZEN 参数同步需要 Node.js 18+，请安装 Node.js 或设置 NODE。')
    subprocess.run([node, str(root / 'scripts/sync_zen.cjs')], cwd=root, check=True)
