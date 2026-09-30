'use strict';
// Run only in an isolated VS Code Extension Development Host workspace/profile.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { once } = require('node:events');
const vscode = require('vscode');
const library = require('../src/collections-core.cjs');
const output = path.resolve(__dirname, '../.test-work/features-host-result.json');
async function run() {
  const checks = []; let server;
  try {
    const root = vscode.workspace.workspaceFolders[0].uri.fsPath;
    assert(root.includes('.test-work'), 'Refuse to modify a normal workspace');
    const contentRoot = path.join(root, 'site/content');
    fs.mkdirSync(path.join(contentRoot, 'posts'), { recursive: true });
    const series = library.createSeries(contentRoot, '宿主验证系列');
    const group = library.createGroup(contentRoot, series.id, '基础组');
    const article = library.createArticle(contentRoot, '宿主配图文章', group.id);
    const extension = vscode.extensions.getExtension('mryu888-plus.research-blog');
    assert(extension); await extension.activate();
    const commands = await vscode.commands.getCommands(true);
    for (const name of ['newSeries', 'newGroup', 'newArticle', 'assignGroup', 'preview', 'check', 'images.generate', 'images.configure']) assert(commands.includes(`researchBlog.${name}`), name);
    assert(extension.packageJSON.contributes.views.explorer.some(v => v.id === 'researchBlog.library'));
    await vscode.commands.executeCommand('researchBlog.refreshLibrary');
    checks.push('extension activates; eight writing/image commands and library view registered');
    const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=';
    let request;
    server = http.createServer(async (req, res) => {
      let body = ''; for await (const chunk of req) body += chunk;
      request = { path: req.url, body: JSON.parse(body) };
      res.setHeader('Content-Type', 'application/json'); res.end(JSON.stringify({ data: [{ b64_json: PNG }] }));
    });
    server.listen(0, '127.0.0.1'); await once(server, 'listening');
    const config = vscode.workspace.getConfiguration('researchBlog.images');
    await config.update('baseUrl', `http://127.0.0.1:${server.address().port}/v1`, vscode.ConfigurationTarget.Global);
    await config.update('model', 'isolated-test-model', vscode.ConfigurationTarget.Global);
    const document = await vscode.workspace.openTextDocument(article.file);
    const editor = await vscode.window.showTextDocument(document);
    const start = document.getText().indexOf('在这里开始写作。');
    editor.selection = new vscode.Selection(document.positionAt(start), document.positionAt(start + '在这里开始写作。'.length));
    await vscode.commands.executeCommand('researchBlog.images.generate');
    assert(request, 'Image command did not reach the test API');
    assert.equal(request.path, '/v1/images/generations');
    assert.equal(request.body.model, 'isolated-test-model');
    assert(request.body.prompt.includes('#9d4941'));
    const updated = fs.readFileSync(article.file, 'utf8');
    assert(updated.includes('在这里开始写作。\n\n!['));
    const asset = updated.match(/\]\((assets\/[^)]+\.png)\)/)[1];
    assert.equal(fs.readFileSync(path.join(path.dirname(article.file), asset)).toString('base64'), PNG);
    checks.push('real VS Code selection calls local HTTP image fixture, writes asset, inserts Markdown and saves');
    assert.equal(library.readCatalog(contentRoot).series[0].groups[0].articles[0], article.key);
    checks.push('series/group membership persisted beside actual editor document');
    fs.writeFileSync(output, JSON.stringify({ ok: true, checks }, null, 2));
  } catch (error) {
    fs.writeFileSync(output, JSON.stringify({ ok: false, checks, error: error.stack }, null, 2));
    throw error;
  } finally { if (server) { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); } }
}
module.exports = { run };
