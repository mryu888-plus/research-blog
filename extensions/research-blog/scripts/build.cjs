'use strict';
const esbuild = require('esbuild');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
async function build() {
  await esbuild.build({
    absWorkingDir: root,
    entryPoints: { extension: 'src/extension.cjs', server: 'src/server.cjs' },
    outdir: 'dist', outExtension: { '.js': '.cjs' },
    bundle: true, platform: 'node', format: 'cjs', target: 'node18',
    external: ['vscode'], legalComments: 'linked', metafile: true,
  }).then(result => {
    // Include dependency license text in the offline VSIX.
    const licenses = new Map();
    for (const input of Object.keys(result.metafile.inputs)) {
      const match = input.match(/node_modules\/((?:@[^/]+\/)?[^/]+)/);
      if (!match || licenses.has(match[1])) continue;
      const dir = path.join(root, 'node_modules', match[1]);
      const file = fs.readdirSync(dir).find(name => /^licen[cs]e(?:\..*)?$/i.test(name));
      if (file) licenses.set(match[1], fs.readFileSync(path.join(dir, file), 'utf8'));
    }
    fs.writeFileSync(path.join(root, 'dist', 'THIRD_PARTY_LICENSES.txt'),
      [...licenses].map(([name, license]) => `${name}\n${license}`).join('\n\n'));
  });
}
build().catch(error => { console.error(error); process.exitCode = 1; });
