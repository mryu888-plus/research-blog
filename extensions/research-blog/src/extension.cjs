'use strict';
const vscode = require('vscode');
const path = require('node:path');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');
const { registerAI } = require('./ai.cjs');
const clients = new Map();
let extensionContext;
async function start(folder) {
  if (folder.uri.scheme !== 'file' || clients.has(folder.uri.toString())) return;
  const contentPath = vscode.workspace.getConfiguration('researchBlog', folder.uri).get('contentPath', 'site/content');
  const root = path.resolve(folder.uri.fsPath, contentPath);
  const relative = path.relative(folder.uri.fsPath, root);
  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    vscode.window.showErrorMessage('Research Blog: contentPath 必须位于当前工作区内。');
    return;
  }
  const watcher = vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(folder, `${relative.replace(/\\/g, '/')}/**/*`));
  const configWatcher = vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(path.dirname(root), 'config.toml'));
  const client = new LanguageClient('researchBlog', `Research Blog (${folder.name})`, {
    module: extensionContext.asAbsolutePath('dist/server.cjs'), transport: TransportKind.stdio,
  }, {
    documentSelector: [{ scheme: 'file', language: 'markdown', pattern: `${root.replace(/\\/g, '/')}/**/*.md` }],
    workspaceFolder: folder,
    initializationOptions: { contentRoot: root },
    synchronize: { fileEvents: [watcher, configWatcher] },
  });
  clients.set(folder.uri.toString(), { client, watcher, configWatcher });
  try { await client.start(); }
  catch (error) {
    clients.delete(folder.uri.toString()); watcher.dispose(); configWatcher.dispose();
    await client.dispose();
    vscode.window.showErrorMessage(`Research Blog 启动失败：${error.message}`);
  }
}
async function stop(key) {
  const entry = clients.get(key);
  if (!entry) return;
  clients.delete(key); entry.watcher.dispose(); entry.configWatcher.dispose(); await entry.client.dispose();
}
async function activate(context) {
  extensionContext = context;
  registerAI(context);
  context.subscriptions.push(vscode.commands.registerCommand('researchBlog.typst', () =>
    vscode.commands.executeCommand('extension.open', 'myriad-dreamin.tinymist')));
  context.subscriptions.push(vscode.commands.registerCommand('researchBlog.restartServer', async () => {
    await deactivate();
    await Promise.all((vscode.workspace.workspaceFolders || []).map(start));
  }));
  context.subscriptions.push(vscode.workspace.onDidChangeWorkspaceFolders(async event => {
    await Promise.all(event.removed.map(folder => stop(folder.uri.toString())));
    await Promise.all(event.added.map(start));
  }));
  await Promise.all((vscode.workspace.workspaceFolders || []).map(start));
}
async function deactivate() { await Promise.all([...clients.keys()].map(stop)); }
module.exports = { activate, deactivate };
