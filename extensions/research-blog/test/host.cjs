'use strict';
// Executed by the actual VS Code Extension Development Host, not node --test.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');
const output = path.resolve(__dirname, '../.test-work/host-result.json');
async function run() {
  const checks = [];
  try {
    const folder = vscode.workspace.workspaceFolders[0].uri.fsPath;
    const target = path.join(folder, 'site/content/posts/target/index.md');
    const source = path.join(folder, 'site/content/posts/source/index.md');
    const figure = path.join(folder, 'site/content/posts/target/figures/test.typ');
    fs.mkdirSync(path.dirname(target), { recursive: true }); fs.mkdirSync(path.dirname(source), { recursive: true }); fs.mkdirSync(path.dirname(figure), { recursive: true });
    fs.writeFileSync(target, '+++\ntitle = "宿主目标"\n[taxonomies]\ntags = ["agent"]\n+++\n## 章节\n');
    fs.writeFileSync(source, '+++\ntitle = "宿主来源"\n[taxonomies]\ntags = ["ag"]\n+++\n[[宿主目标#章节]]\n[[宿主]]\n[[不存在]]\n');
    fs.writeFileSync(figure, '#let square(x) = x * x\n#square(2)\n');
    const extension = vscode.extensions.getExtension('mryu888-plus.research-blog');
    assert(extension); await extension.activate(); checks.push('extension activation');
    const doc = await vscode.workspace.openTextDocument(source);
    await vscode.window.showTextDocument(doc);
    const until = Date.now() + 20000;
    let completions;
    while (Date.now() < until) {
      completions = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, new vscode.Position(6, 4));
      if (completions?.items.some(item => item.label === '宿主目标')) break;
      await new Promise(r => setTimeout(r, 100));
    }
    assert(completions?.items.some(item => item.label === '宿主目标')); checks.push('real editor link completion');
    const tags = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', doc.uri, new vscode.Position(3, 11));
    assert(tags.items.some(item => item.label === 'agent')); checks.push('real editor tag completion');
    const defs = await vscode.commands.executeCommand('vscode.executeDefinitionProvider', doc.uri, new vscode.Position(5, 4));
    assert(defs.some(def => (def.uri || def.targetUri)?.fsPath === target)); checks.push('real editor go to definition');
    const hover = await vscode.commands.executeCommand('vscode.executeHoverProvider', doc.uri, new vscode.Position(5, 4));
    assert(hover.length); checks.push('real editor hover');
    await new Promise(r => setTimeout(r, 300));
    assert(vscode.languages.getDiagnostics(doc.uri).some(d => d.source === 'research-blog' && d.code === 'link-missing')); checks.push('real editor diagnostics');
    const tinymist = vscode.extensions.getExtension('myriad-dreamin.tinymist');
    if (tinymist) {
      await tinymist.activate();
      const typ = await vscode.workspace.openTextDocument(figure); await vscode.window.showTextDocument(typ);
      assert.equal(typ.languageId, 'typst');
      let results;
      const deadline = Date.now() + 20000;
      while (Date.now() < deadline) {
        results = await vscode.commands.executeCommand('vscode.executeDefinitionProvider', typ.uri, new vscode.Position(1, 3));
        if (results?.length) break;
        await new Promise(r => setTimeout(r, 150));
      }
      assert(results?.length); checks.push(`Tinymist ${tinymist.packageJSON.version} real Typst definition`);
    }
    fs.writeFileSync(output, JSON.stringify({ ok: true, checks }, null, 2));
  } catch (error) { fs.writeFileSync(output, JSON.stringify({ ok: false, checks, error: error.stack }, null, 2)); throw error; }
}
module.exports = { run };
