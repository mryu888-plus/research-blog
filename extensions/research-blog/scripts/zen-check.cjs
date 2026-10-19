'use strict';
// `blogctl zen` generates every ZEN consumer from ZEN.md: site/static/zen.css, design/zen.generated.cjs,
// the blogdiagram snippet, both editor themes and the theme preview page. The source is
// crates/blog-site/src/zen.rs; the VS Code colour roles live only in crates/blog-site/src/zen/theme.json.
// Extension builds check that the generated files are current and never write them.
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const REPO = path.resolve(__dirname, '../../..');
const STALE = '请在仓库根目录运行 blogctl zen（源码仓库里是 cargo run -p blogctl -- zen），再重新构建。';

function checkZen() {
  const attempts = [[process.env.BLOGCTL || 'blogctl', ['--root', REPO, 'zen', '--check']]];
  if (fs.existsSync(path.join(REPO, 'Cargo.toml'))) {
    attempts.push(['cargo', ['run', '-q', '-p', 'blogctl', '--', '--root', REPO, 'zen', '--check']]);
  }
  for (const [command, args] of attempts) {
    const result = spawnSync(command, args, { cwd: REPO, encoding: 'utf8', windowsHide: true, shell: false });
    if (result.error?.code === 'ENOENT') continue;
    if (result.status === 0) return;
    const output = `${result.stderr || ''}${result.stdout || ''}`.trim();
    throw new Error(`ZEN 生成文件检查没有通过。${STALE}${output ? `\n${output}` : ''}`);
  }
  throw new Error('没有找到 blogctl，也没有找到 cargo。请安装 blogctl 并放进 PATH，或用环境变量 BLOGCTL 指定它的路径。');
}

module.exports = { checkZen };
