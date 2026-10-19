'use strict';
// Preview, build checks and the portable Zotero client all run through the blogctl program.
const path = require('node:path');
const { execFile } = require('node:child_process');

const INSTALL_HINT = '无法运行 blogctl。请从 https://github.com/mryu888-plus/research-blog/releases 下载 blogctl 并放进 PATH，'
  + '或在源码仓库里运行 cargo install --path crates/blogctl；也可以在设置 researchBlog.blogctlPath 中填写它的完整路径。';

function blogctlPath(vscode, folderUri) {
  const configured = vscode.workspace.getConfiguration('researchBlog', folderUri).get('blogctlPath', 'blogctl');
  return typeof configured === 'string' && configured.trim() ? configured.trim() : 'blogctl';
}

// The blog root holds site/config.toml; contentRoot is <root>/site/content.
function blogRoot(contentRoot) {
  return path.dirname(path.dirname(contentRoot));
}

function ensureBlogctl(binary, run = execFile) {
  return new Promise((resolve, reject) => {
    run(binary, ['--version'], { timeout: 10000, windowsHide: true, shell: false }, error => error ? reject(new Error(INSTALL_HINT)) : resolve(binary));
  });
}

module.exports = { INSTALL_HINT, blogctlPath, blogRoot, ensureBlogctl };
