'use strict';
const vscode = require('vscode');
const fs = require('node:fs');
const path = require('node:path');
const core = require('./collections-core.cjs');

function scopeFor(folder) {
  if (!folder || folder.uri.scheme !== 'file') throw new Error('请先用 VS Code 打开博客文件夹。');
  const configured = vscode.workspace.getConfiguration('researchBlog', folder.uri).get('contentPath', 'site/content');
  const contentRoot = core.safePath(folder.uri.fsPath, configured);
  if (!core.inside(folder.uri.fsPath, path.dirname(contentRoot))) throw new Error('请打开包含 content 和 data 的博客目录。');
  if (!fs.existsSync(path.join(contentRoot, 'posts'))) throw new Error('没有找到文章目录，请打开博客根目录，或检查 researchBlog.contentPath。');
  return { folder, contentRoot };
}
async function chooseScope(node) {
  if (node?.folder) return scopeFor(node.folder);
  const active = vscode.window.activeTextEditor?.document.uri;
  const current = active && vscode.workspace.getWorkspaceFolder(active);
  if (current) { try { return scopeFor(current); } catch { /* Offer other workspace roots. */ } }
  const choices = (vscode.workspace.workspaceFolders || []).flatMap(folder => {
    try { return [{ label: folder.name, ...scopeFor(folder) }]; } catch { return []; }
  });
  if (!choices.length) throw new Error('请先打开包含 site/content/posts 的博客文件夹。');
  return choices.length === 1 ? choices[0] : vscode.window.showQuickPick(choices, { title: '选择博客项目' });
}
function trusted() {
  if (!vscode.workspace.isTrusted) throw new Error('请先信任此工作区，再创建内容或使用 AI。');
}
function registerWriting(context) {
  const changes = new vscode.EventEmitter();
  const refresh = () => changes.fire();
  context.subscriptions.push(changes);
  const command = (name, action) => context.subscriptions.push(vscode.commands.registerCommand(name, async (...args) => {
    try { trusted(); const result = await action(...args); refresh(); return result; }
    catch (error) { vscode.window.showErrorMessage(`Research Blog: ${error.message}`); }
  }));
  async function askTitle(title) { return vscode.window.showInputBox({ title, ignoreFocusOut: true, validateInput: value => { try { core.titleText(value); } catch (e) { return e.message; } } }); }
  async function newSeries(scope) {
    const title = await askTitle('新建系列，例如：从零理解检索增强生成');
    if (!title) return;
    const description = await vscode.window.showInputBox({ title: '系列简介（可留空）', ignoreFocusOut: true });
    if (description === undefined) return;
    return core.createSeries(scope.contentRoot, title, description);
  }
  async function pickSeries(scope, fixedId) {
    const list = core.readCatalog(scope.contentRoot).series;
    if (fixedId) return list.find(series => series.id === fixedId);
    const chosen = await vscode.window.showQuickPick([
      ...list.map(series => ({ label: series.title, description: `${(series.articles || []).length} 篇文章 · ${series.groups.length} 个组`, series })),
      { label: '$(add) 新建系列', create: true },
    ], { title: '选择所属系列' });
    if (!chosen) return;
    return chosen.create ? newSeries(scope) : chosen.series;
  }
  async function newGroup(scope, fixedId) {
    const series = await pickSeries(scope, fixedId);
    if (!series) return;
    const title = await askTitle(`在「${series.title}」中新建组`);
    if (!title) return;
    const description = await vscode.window.showInputBox({ title: '组简介（可留空）', ignoreFocusOut: true });
    if (description === undefined) return;
    return core.createGroup(scope.contentRoot, series.id, title, description);
  }
  async function pickDestination(scope, node) {
    if (node?.groupId) return { id: node.groupId };
    if (node?.kind === 'series') return { id: node.seriesId };
    const destinations = core.readCatalog(scope.contentRoot).series.flatMap(s => [
      { label: s.title, description: '直接加入系列', target: s },
      ...s.groups.map(g => ({ label: `${s.title} / ${g.title}`, description: '分组', target: g })),
    ]);
    const chosen = await vscode.window.showQuickPick([
      ...destinations, { label: '$(add) 新建系列', create: 'series' },
      { label: '$(new-folder) 新建分组', create: 'group' },
      { label: '暂不加入系列', target: { id: null } },
    ], { title: '选择文章所属的系列或分组', matchOnDescription: true });
    if (!chosen) return;
    if (chosen.create === 'series') return newSeries(scope);
    return chosen.create === 'group' ? newGroup(scope) : chosen.target;
  }
  command('researchBlog.newSeries', async node => { const scope = await chooseScope(node); if (scope) return newSeries(scope); });
  command('researchBlog.newGroup', async node => { const scope = await chooseScope(node); if (scope) return newGroup(scope, node?.seriesId); });
  command('researchBlog.newArticle', async node => {
    const scope = await chooseScope(node); if (!scope) return;
    const group = await pickDestination(scope, node); if (!group) return;
    const title = await askTitle('新建文章（支持中文标题）'); if (!title) return;
    const article = core.createArticle(scope.contentRoot, title, group.id);
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(article.file));
  });
  command('researchBlog.assignGroup', async node => {
    const scope = await chooseScope(node); if (!scope) return;
    const uri = node?.key ? vscode.Uri.file(core.safePath(scope.contentRoot, node.key)) : vscode.window.activeTextEditor?.document.uri;
    if (!uri || uri.scheme !== 'file') throw new Error('请打开需要移动的文章。');
    const key = core.articleKey(path.relative(scope.contentRoot, uri.fsPath).split(path.sep).join('/'));
    const group = await pickDestination(scope); if (!group) return;
    core.assignArticle(scope.contentRoot, key, group.id);
  });
  for (const [name, delta] of [['moveArticleUp', -1], ['moveArticleDown', 1]]) command(`researchBlog.${name}`, async node => {
    if (!node?.key) return;
    core.moveArticle(scopeFor(node.folder).contentRoot, node.key, delta);
  });
  context.subscriptions.push(vscode.commands.registerCommand('researchBlog.refreshLibrary', refresh));
  command('researchBlog.editCollections', async node => {
    const scope = await chooseScope(node); if (!scope) return;
    const file = core.catalogPath(scope.contentRoot);
    if (!fs.existsSync(file)) { vscode.window.showInformationMessage('先新建一个系列，即会生成系列目录。'); return; }
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(file));
  });
  for (const [action, label] of [['preview', '博客：预览网站（含草稿）'], ['check', '博客：构建并检查']]) command(`researchBlog.${action}`, async node => {
    const scope = await chooseScope(node); if (!scope) return;
    const repo = path.dirname(path.dirname(scope.contentRoot)), script = path.join(repo, 'scripts/editor.py');
    if (!fs.existsSync(script)) throw new Error('此博客缺少 scripts/editor.py，请打开完整博客仓库。');
    const running = vscode.tasks.taskExecutions.find(e => e.task.definition.type === 'researchBlog' && e.task.definition.action === action && e.task.scope?.uri?.toString() === scope.folder.uri.toString());
    if (running) { vscode.window.showInformationMessage(`${label}已在运行，请查看终端。`); return; }
    const task = new vscode.Task({ type: 'researchBlog', action }, scope.folder, label, 'Research Blog', new vscode.ProcessExecution('python', [script, action], { cwd: repo }), []);
    task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, panel: vscode.TaskPanelKind.Dedicated };
    task.isBackground = action === 'preview';
    await vscode.tasks.executeTask(task);
  });
  const provider = {
    onDidChangeTreeData: changes.event,
    getTreeItem(node) {
      const expandable = ['root', 'series', 'group', 'ungrouped'].includes(node.kind);
      const item = new vscode.TreeItem(node.label, expandable ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None);
      item.contextValue = node.kind;
      item.description = node.description;
      item.id = `${node.folder?.uri.toString() || ''}:${node.seriesId || ''}:${node.groupId || ''}:${node.key || node.kind}`;
      item.iconPath = new vscode.ThemeIcon({ root: 'root-folder', series: 'library', group: 'folder', ungrouped: 'inbox', article: 'markdown', error: 'warning' }[node.kind]);
      if (node.kind === 'article') {
        const uri = vscode.Uri.file(core.safePath(scopeFor(node.folder).contentRoot, node.key));
        item.resourceUri = uri;
        item.command = { command: 'vscode.open', title: '打开文章', arguments: [uri] };
      }
      return item;
    },
    getChildren(node) {
      try {
        const scopes = (vscode.workspace.workspaceFolders || []).flatMap(folder => { try { return [scopeFor(folder)]; } catch { return []; } });
        if (!node && scopes.length > 1) return scopes.map(s => ({ kind: 'root', folder: s.folder, label: s.folder.name }));
        const scope = node?.folder ? scopeFor(node.folder) : scopes[0];
        if (!scope) return [];
        const data = core.readCatalog(scope.contentRoot), articles = core.listArticles(scope.contentRoot);
        const articleNodes = keys => keys.map(key => {
          const article = articles.find(a => a.key === key);
          return article && { kind: 'article', folder: scope.folder, key, label: article.title, description: article.draft ? '草稿' : undefined };
        }).filter(Boolean);
        if (node?.kind === 'group') return articleNodes(data.series.flatMap(s => s.groups).find(g => g.id === node.groupId)?.articles || []);
        if (node?.kind === 'series') {
          const series = data.series.find(s => s.id === node.seriesId);
          return [...articleNodes(series?.articles || []), ...(series?.groups || []).map(g => ({ kind: 'group', folder: scope.folder, seriesId: node.seriesId, groupId: g.id, label: g.title, description: `${g.articles.filter(k => articles.some(a => a.key === k)).length} 篇` }))];
        }
        const grouped = new Set(core.containers(data).flatMap(item => item.articles || []));
        const loose = articles.filter(a => !grouped.has(a.key));
        if (node?.kind === 'ungrouped') return articleNodes(loose.map(a => a.key));
        return [...data.series.map(s => ({ kind: 'series', folder: scope.folder, seriesId: s.id, label: s.title, description: `${[s, ...s.groups].flatMap(item => item.articles || []).filter(k => articles.some(a => a.key === k)).length} 篇` })),
          ...(loose.length ? [{ kind: 'ungrouped', folder: scope.folder, label: '未加入系列', description: `${loose.length} 篇` }] : [])];
      } catch (error) { return [{ kind: 'error', label: error.message }]; }
    },
  };
  context.subscriptions.push(vscode.window.createTreeView('researchBlog.library', { treeDataProvider: provider, showCollapseAll: true }));
  const watcher = vscode.workspace.createFileSystemWatcher('**/{collections.json,*.md}');
  context.subscriptions.push(watcher, watcher.onDidChange(refresh), watcher.onDidCreate(refresh), watcher.onDidDelete(refresh), vscode.workspace.onDidChangeWorkspaceFolders(refresh), vscode.workspace.onDidChangeConfiguration(refresh));
}
module.exports = { registerWriting, scopeFor, chooseScope, trusted };
