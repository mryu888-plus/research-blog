'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const { URI } = require('vscode-uri');
const zotero = require('../src/zotero-core.cjs');
const { registerReferences, citationKeys, mergeSnapshot, draftMarkdown, validateMessage, publicOnly } = require('../src/references.cjs');
const { referenceView } = require('../src/references-view.cjs');
const library = require('../src/collections-core.cjs');
const TEST_ROOT = path.resolve(__dirname, '..', '.test-work');
function scratch(prefix) { fs.mkdirSync(TEST_ROOT, { recursive: true }); return fs.mkdtempSync(path.join(TEST_ROOT, prefix)); }

const PAPER = 'ABCD2345', PAPER2 = 'BCDE3456', PDF = 'CDEF4567', ANNOTATION = 'DEFG5678', COLLECTION = 'EFGH6789';
const paper = (key = PAPER, title = '检索与研究') => ({ key, version: 3, data: { key, version: 3, itemType: 'journalArticle', title, date: '2024', creators: [{ creatorType: 'author', firstName: 'Ada', lastName: 'Lovelace' }], publicationTitle: 'Research Journal', DOI: '10.1000/fixture', url: 'https://example.org/paper', tags: [{ tag: '检索' }], collections: [COLLECTION], abstractNote: '<p>可复用的摘要</p>', extra: 'Citation Key: research2024' } });
const attachment = { key: PDF, data: { key: PDF, itemType: 'attachment', title: '全文', contentType: 'application/pdf', filename: 'paper.pdf', parentItem: PAPER } };
const annotation = { key: ANNOTATION, data: { key: ANNOTATION, itemType: 'annotation', parentItem: PDF, annotationText: 'Grounded retrieval improves verification.', annotationComment: '需要检查数据来源。', annotationPageLabel: 'iv', annotationPosition: '{"pageIndex":3}' } };
function harness(t, options = {}) {
  const root = options.root || scratch('blog-references-');
  const contentRoot = path.join(root, 'site', 'content'); fs.mkdirSync(path.join(contentRoot, 'posts'), { recursive: true });
  const article = library.createArticle(contentRoot, `测试文章-${Math.random().toString(36).slice(2, 7)}`);
  const commands = new Map(), errors = [], infos = [], panels = [], secretValues = new Map(options.secretValues || []), inputs = [], picks = [], opened = [], spawned = [];
  const config = { baseUrl: 'http://127.0.0.1:23119/api/', libraryPath: 'users/0', autoRefreshSeconds: 0, ...options.config };
  const source = { items: [paper(), paper(PAPER2, '未被引用的文献')], collections: [{ key: COLLECTION, version: 1, data: { key: COLLECTION, name: '阅读计划', parentCollection: false } }], online: true, writeSupported: true, remember: true, listCount: 0, mutationCount: 0, ...options.source };
  let text = fs.readFileSync(article.file, 'utf8'), configHandler, activeHandler, messageHandler;
  const document = { uri: URI.file(article.file), languageId: 'markdown', getText: () => text, offsetAt: position => position.offset, positionAt: offset => ({ offset }) };
  const editor = { document, selection: { active: { offset: text.indexOf('在这里开始写作。') }, end: { offset: text.indexOf('在这里开始写作。') } }, edit: async callback => {
    if (options.editFails) return false;
    callback({ insert: (position, value) => { text = text.slice(0, position.offset) + value + text.slice(position.offset); } }); return true;
  } };
  class Emitter { constructor() { this.event = () => ({ dispose() {} }); } fire() {} dispose() {} }
  class Client {
    constructor(data) { Object.assign(this, data); this.serverID = 'fixture-db'; source.clients ||= []; source.clients.push(this); }
    async probe() { if (!source.online) throw new Error('无法连接 Zotero。'); return { read: true, canRead: true, writeSupported: source.writeSupported, canWrite: source.writeSupported && !!this.apiKey, writeAuthorized: !!this.apiKey }; }
    async listCollections() { return source.collections; }
    async listItems() { source.listCount++; if (source.delay) await new Promise(resolve => setTimeout(resolve, source.delay)); return source.items; }
    async children(key) { if (source.childrenFail) throw new Error('批注读取失败。'); return key === PAPER ? [attachment] : key === PDF ? [annotation] : []; }
    async getItem(key) { return source.items.find(item => item.key === key); }
    async authorizeWrites() { if (source.deny) throw new Error('你拒绝了 Zotero 写入授权。'); this.apiKey = 'private-fixture-key'; this.remember = source.remember; return { key: this.apiKey, remember: source.remember }; }
    async updateItem(item, patch) { source.mutationCount++; if (source.writeFail) throw new Error('文献已被修改。'); Object.assign(item.data, patch); item.version++; return item; }
    async createCollection(name, parentKey) { source.mutationCount++; const c = { key: 'FGHI7892', version: 1, data: { name, parentCollection: parentKey } }; source.collections.push(c); return c; }
    async updateCollection(collection, patch) { source.mutationCount++; if (source.writeFail) throw new Error('分类版本冲突。'); Object.assign(collection.data, patch); return collection; }
    async importPDF(filePath, data) { source.mutationCount++; source.lastImport = { filePath, data }; if (source.importFail) throw new Error('附件上传失败，请刷新后检查。'); return { item: source.items[0], attachment }; }
  }
  const vscode = {
    EventEmitter: Emitter, Uri: URI, ViewColumn: { Active: 1, One: 1, Beside: -2 }, TreeItemCollapsibleState: { None: 0 }, TreeItem: class { constructor(label) { this.label = label; } }, ThemeIcon: class { constructor(id) { this.id = id; } },
    commands: { registerCommand: (name, fn) => { commands.set(name, fn); return { dispose: () => commands.delete(name) }; } },
    workspace: { isTrusted: true, textDocuments: [document], getConfiguration: () => ({ get: (key, fallback) => config[key] ?? fallback }), onDidChangeConfiguration: callback => { configHandler = callback; return { dispose() {} }; }, openTextDocument: async file => ({ uri: URI.file(file), getText: () => fs.readFileSync(file, 'utf8') }) },
    window: { activeTextEditor: editor, onDidChangeActiveTextEditor: callback => { activeHandler = callback; return { dispose() {} }; }, showErrorMessage: async message => { errors.push(message); }, showInformationMessage: async message => { infos.push(message); }, showInputBox: async data => inputs.length ? inputs.shift() : data.value, showQuickPick: async (choices, data) => {
      const chosen = picks.shift(); if (typeof chosen === 'function') return chosen(choices, data); if (chosen === null) return undefined; if (chosen !== undefined) return chosen;
      return data.canPickMany ? choices.filter(c => c.picked) : choices[0];
    }, showOpenDialog: async () => options.dialog || [URI.file(path.join(root, 'input.pdf'))], showTextDocument: async doc => { source.shownDocument = doc; return doc; }, createTreeView: () => ({ dispose() {} }), createWebviewPanel: () => {
      const panel = { visible: true, webview: { html: '', states: [], postMessage: async data => { panel.webview.states.push(data.state); }, onDidReceiveMessage: fn => { messageHandler = fn; return { dispose() {} }; } }, reveal() {}, onDidDispose: fn => { panel.close = fn; return { dispose() {} }; }, dispose() { panel.close?.(); } }; panels.push(panel); return panel;
    } },
    env: { openExternal: async uri => { opened.push(uri.toString(true)); return options.externalFails ? false : true; } },
  };
  const scope = { contentRoot, folder: { name: 'fixture', uri: URI.file(root) } };
  const context = { subscriptions: [], globalStorageUri: URI.file(path.join(root, '.private-storage')), secrets: { get: async key => secretValues.get(key), store: async (key, value) => { secretValues.set(key, value); } } };
  const controller = registerReferences(context, { vscode, zotero: { ...zotero, ZoteroClient: Client }, writing: { chooseScope: async () => scope, trusted: () => { if (!vscode.workspace.isTrusted) throw new Error('请先信任此工作区。'); } }, spawn: (binary, args, data) => { spawned.push({ binary, args, data }); const child = new EventEmitter(); process.nextTick(() => child.emit('exit', 0)); return child; } });
  t.after(() => { controller.dispose(); if (!options.keepRoot) fs.rmSync(root, { recursive: true, force: true }); });
  const command = (name, arg) => commands.get(`researchBlog.${name}`)(arg);
  return { root, article, contentRoot, source, config, commands, command, errors, infos, inputs, picks, panels, secretValues, opened, spawned, controller, vscode, editor, document, get text() { return text; }, setText: value => { text = value; }, send: message => messageHandler(message), configChanged: () => configHandler({ affectsConfiguration: () => true }), activeChanged: value => activeHandler(value) };
}

test('workbench registers usable entry points and does not embed library HTML in its document', async t => {
  const h = harness(t); await h.command('openReferences');
  for (const command of ['insertCitation', 'openReferencePDF', 'createReferenceNote', 'importReferencePDF', 'editReference', 'exportReferences', 'authorizeZotero', 'startZotero']) assert(h.commands.has(`researchBlog.${command}`));
  const panel = h.panels[0]; assert.match(panel.webview.html, /Content-Security-Policy/); assert(!panel.webview.html.includes('检索与研究'));
  const state = panel.webview.states.at(-1); assert.equal(state.online, true); assert.equal(state.canWrite, false); assert.equal(state.items.length, 2); assert(!JSON.stringify(state).includes('private-fixture-key'));
});

test('citation insertion writes only that bibliography record and preserves the selected prose', async t => {
  const h = harness(t); const original = h.text; await h.command('insertCitation', PAPER);
  assert(h.text.includes('[@research2024]在这里开始写作。')); assert(h.text.endsWith(original.slice(original.indexOf('在这里开始写作。'))));
  const snapshot = JSON.parse(fs.readFileSync(path.join(h.root, 'site/data/references.json')));
  assert.equal(snapshot.items.length, 1); const item = snapshot.items[0]; assert.equal(item.id, 'research2024'); assert.deepEqual(item.author, [{ family: 'Lovelace', given: 'Ada' }]); assert.deepEqual(item.issued, { 'date-parts': [[2024]] }); assert.equal(item['container-title'], 'Research Journal');
  for (const field of ['abstractNote', 'tags', 'collections', 'attachments', 'extra', 'apiKey', 'serverID']) assert(!(field in item));
  assert.equal(h.errors.length, 0); assert(fs.readdirSync(path.join(h.root, '.private-storage')).some(file => file.endsWith('.json')));
});

test('failed editor insertion is reported without an insertion success message', async t => {
  const h = harness(t, { editFails: true }); const original = h.text; await h.command('insertCitation', PAPER);
  assert.equal(h.text, original); assert.match(h.errors[0], /未能插入引用/); assert.equal(h.infos.length, 0);
});

test('citation insertion revives a hidden editor and restores its saved cursor', async t => {
  const h = harness(t); await h.command('openReferences'); const edit = h.editor.edit, selection = h.editor.selection; h.editor.edit = async () => false; h.vscode.window.activeTextEditor = undefined; h.vscode.window.visibleTextEditors = [];
  let showOptions; const visible = { document: h.document, selection: { active: { offset: 0 } }, edit };
  h.vscode.window.showTextDocument = async (document, options) => { assert.equal(document, h.document); showOptions = options; h.vscode.window.visibleTextEditors = [visible]; return visible; };
  await h.command('insertCitation', PAPER); assert(h.text.includes('[@research2024]在这里开始写作。')); assert.equal(visible.selection, selection); assert.equal(showOptions.preserveFocus, true); assert.equal(showOptions.viewColumn, 1); assert.equal(h.errors.length, 0);
});

test('metadata and code blocks reject citation insertion', async t => {
  const h = harness(t); h.editor.selection.active = { offset: 3 }; await h.command('insertCitation', PAPER); assert.match(h.errors.at(-1), /正文/);
  for (const body of ['```python\nprint(1)\n```\n正文', '正文 `print(1)` 后面的文字', '正文 $print(1)$ 后面的文字', '$$\nprint(1)\n$$\n正文', '正文 \\(print(1)\\) 之后']) {
    const source = `+++\ntitle = "测试"\n+++\n\n${body}`; h.setText(source); h.editor.selection.active = { offset: source.indexOf('print') + 3 }; await h.command('insertCitation', PAPER); assert.match(h.errors.at(-1), /代码或公式/); assert(!fs.existsSync(path.join(h.root, 'site/data/references.json')));
  }
});

test('PDF annotations become a real draft article, with public reference and source-page links', async t => {
  const h = harness(t); await h.command('refreshReferences'); h.inputs.push('阅读检索论文'); const result = await h.command('createReferenceNote', PAPER);
  assert(result && fs.existsSync(result.file)); const text = fs.readFileSync(result.file, 'utf8'); assert.match(text, /draft = true/); assert.match(text, /title = "阅读检索论文"/); assert.match(text, /\[@research2024\]/); assert.match(text, /zotero:\/\/open-pdf\/library\/items\/CDEF4567\?page=4&annotation=DEFG5678/); assert(text.includes('需要检查数据来源。'));
  assert.equal(JSON.parse(fs.readFileSync(path.join(h.root, 'site/data/references.json'))).items.length, 1); assert.equal(path.relative(h.source.shownDocument.uri.fsPath, result.file), ''); assert.equal(h.errors.length, 0);
});

test('a details API failure creates no draft and never reports success', async t => {
  const h = harness(t, { source: { childrenFail: true } }); await h.command('refreshReferences'); await h.command('createReferenceNote', PAPER);
  assert.equal(library.listArticles(h.contentRoot).length, 1); assert.match(h.errors.at(-1), /批注读取失败/); assert.equal(h.infos.length, 0);
});

test('authorization uses private credentials and a rejected authorization stays read only', async t => {
  const h = harness(t); await h.command('authorizeZotero'); assert.equal(h.secretValues.size, 1); assert.equal([...h.secretValues.values()][0], 'private-fixture-key');
  await h.command('openReferences'); assert.equal(h.panels[0].webview.states.at(-1).canWrite, true);
  const secretFiles = fs.readdirSync(path.join(h.root, '.private-storage')).map(file => fs.readFileSync(path.join(h.root, '.private-storage', file), 'utf8')).join(''); assert(!secretFiles.includes('private-fixture-key'));
  h.source.deny = true; const before = h.infos.length; await h.command('authorizeZotero'); assert.match(h.errors.at(-1), /拒绝/); assert.equal(h.infos.length, before);
});

test('one-use authorization is retained only in memory', async t => {
  const h = harness(t, { source: { remember: false } }); await h.command('authorizeZotero'); assert.equal(h.secretValues.size, 0); assert.equal(h.errors.length, 0);
});

test('old Zotero is read only and modifying commands do not write', async t => {
  const h = harness(t, { source: { writeSupported: false } }); await h.command('openReferences'); const state = h.panels[0].webview.states.at(-1); assert.equal(state.online, true); assert.equal(state.writeSupported, false);
  await h.command('importReferencePDF'); assert.equal(h.source.mutationCount, 0); assert.match(h.errors.at(-1), /只支持读取/);
});

test('PDF import success follows completed upload; an upload failure is never announced as success', async t => {
  const h = harness(t); await h.command('authorizeZotero'); h.inputs.push('导入论文'); const imported = await h.command('importReferencePDF'); assert(imported.attachment); assert.equal(h.source.lastImport.data.title, '导入论文'); assert(h.infos.at(-1).includes('PDF 已导入'));
  h.source.importFail = true; h.inputs.push('上传失败论文'); const count = h.infos.length; await h.command('importReferencePDF'); assert.equal(h.infos.length, count); assert.match(h.errors.at(-1), /附件上传失败/);
});

test('editing title, tags and classification updates Zotero metadata', async t => {
  const h = harness(t); await h.command('authorizeZotero');
  h.picks.push(choices => choices.find(c => c.field === 'title')); h.inputs.push('新的研究标题'); await h.command('editReference', PAPER); assert.equal(h.source.items[0].data.title, '新的研究标题');
  h.picks.push(choices => choices.find(c => c.field === 'tags')); h.inputs.push('检索, 复现，检索'); await h.command('editReference', PAPER); assert.deepEqual(h.source.items[0].data.tags, [{ tag: '检索' }, { tag: '复现' }]);
  h.picks.push(choices => choices.find(c => c.field === 'collections')); h.picks.push([]); await h.command('editReference', PAPER); assert.deepEqual(h.source.items[0].data.collections, []); assert.equal(h.source.mutationCount, 3); assert.equal(h.errors.length, 0);
});

test('export uses actual article citations and excludes code, math and metadata', async t => {
  const h = harness(t); h.source.items[1].data.extra = 'Citation Key: unused';
  h.setText('+++\ntitle = "[@metadata]"\n+++\n真实引用 [@research2024]\n\n```md\n[@unused]\n```\n`[@inline]` $[@math]$\n$$\n[@display]\n$$');
  mergeSnapshot(h.contentRoot, [{ id: 'anotherLibrary', title: '来自另一个文献库', type: 'book' }]);
  await h.command('exportReferences'); const snapshot = JSON.parse(fs.readFileSync(path.join(h.root, 'site/data/references.json'))); assert.deepEqual(snapshot.items.map(item => item.id), ['anotherLibrary', 'research2024']); assert.equal(h.errors.length, 0);
});

test('offline library cache remains readable and citations work while modifying actions fail', async t => {
  const h = harness(t); await h.command('openReferences'); await h.send({ type: 'select', key: PAPER });
  h.source.online = false; await h.command('refreshReferences'); let state = h.panels[0].webview.states.at(-1); assert.equal(state.online, false); assert.equal(state.items.length, 2); assert.equal(state.detail.annotations.length, 1); assert.match(state.statusLabel, /离线缓存/);
  await h.command('insertCitation', PAPER); assert(h.text.includes('[@research2024]')); await h.command('importReferencePDF'); assert.equal(h.source.mutationCount, 0); assert.match(h.errors.at(-1), /当前离线/);
});

test('private cache survives an extension restart and keeps offline group-library annotation links', async t => {
  const h = harness(t, { keepRoot: true, config: { libraryPath: 'groups/123' } }); await h.command('openReferences'); await h.send({ type: 'select', key: PAPER }); h.controller.dispose();
  const restored = harness(t, { root: h.root, config: { libraryPath: 'groups/123' }, source: { online: false } }); await restored.command('openReferences'); const state = restored.panels[0].webview.states.at(-1); assert.equal(state.online, false); assert.equal(state.items.length, 2); assert.match(state.statusLabel, /离线缓存/);
  restored.inputs.push('离线群组阅读草稿'); const article = await restored.command('createReferenceNote', PAPER); assert(article && fs.existsSync(article.file)); assert.match(fs.readFileSync(article.file, 'utf8'), /zotero:\/\/open-pdf\/groups\/123\/items\/CDEF4567/); assert.equal(restored.errors.length, 0);
});

test('article backlinks include unsaved citations, label drafts, and only open scoped article files', async t => {
  const h = harness(t); await h.command('openReferences'); await h.command('insertCitation', PAPER); await h.send({ type: 'select', key: PAPER });
  const state = h.panels[0].webview.states.at(-1); assert.equal(state.detail.citedBy.length, 1); assert.equal(state.detail.citedBy[0].articleKey, h.article.key); assert.equal(state.detail.citedBy[0].draft, true);
  await h.send({ type: 'openArticle', articleKey: h.article.key }); assert.equal(path.relative(h.source.shownDocument.uri.fsPath, h.article.file), '');
  const shown = h.source.shownDocument; await h.send({ type: 'openArticle', articleKey: 'posts/../../private/index.md' }); assert.equal(h.source.shownDocument, shown);
  assert.equal(validateMessage({ type: 'openArticle', articleKey: 'posts/真实文章/index.md' }), true); assert.equal(validateMessage({ type: 'openArticle', articleKey: '../private.md' }), false);
});

test('concurrent refresh requests share one request and configuration switches libraries safely', async t => {
  const h = harness(t, { source: { delay: 20 } }); await h.command('openReferences'); const before = h.source.listCount;
  await Promise.all([h.command('refreshReferences'), h.command('refreshReferences'), h.command('refreshReferences')]); assert.equal(h.source.listCount, before + 1);
  const pending = h.command('refreshReferences'); h.config.libraryPath = 'groups/123'; h.configChanged(); await pending; await new Promise(resolve => setTimeout(resolve, 35)); assert.equal([...h.controller.states.values()][0].client.libraryPath, 'groups/123');
});

test('webview validates messages and annotation HTML is reduced to text in the view model', async t => {
  const h = harness(t); await h.command('openReferences'); const before = h.source.listCount;
  await h.send({ type: 'filter', q: 'Ada', collectionKey: '', tag: '' }); assert.equal(h.panels[0].webview.states.at(-1).items.length, 2);
  await h.send({ type: 'select', key: '../../outside' }); await h.send({ type: 'refresh', apiKey: 'intruder' }); assert.equal(h.source.listCount, before);
  const original = annotation.data.annotationText; annotation.data.annotationText = '<img src="file:///private.pdf" onerror="alert(1)"><b>安全文本</b><script>steal()</script>';
  t.after(() => { annotation.data.annotationText = original; }); await h.send({ type: 'select', key: PAPER }); const state = h.panels[0].webview.states.at(-1); assert.equal(state.detail.annotations[0].text, '安全文本'); assert(!JSON.stringify(state).includes('file:///private'));
  await h.send({ type: 'openAnnotation', key: PAPER, attachmentKey: PDF, annotationKey: ANNOTATION }); assert(h.opened.at(-1).includes('page=4')); assert.equal(h.errors.length, 0);
});

test('workspace trust gates browsing, authorization, local launch and all file writes', async t => {
  const h = harness(t); h.vscode.workspace.isTrusted = false;
  for (const command of ['openReferences', 'insertCitation', 'importReferencePDF', 'startZotero', 'authorizeZotero']) await h.command(command, PAPER);
  assert.equal(h.source.listCount, 0); assert.equal(h.source.mutationCount, 0); assert.equal(h.spawned.length, 0); assert.equal(h.panels.length, 0); assert.equal(h.errors.length, 5); assert(!fs.existsSync(path.join(h.root, 'site/data/references.json')));
});

test('portable Zotero fallback launches only a checked script with structured arguments', async t => {
  const h = harness(t, { externalFails: true }); fs.mkdirSync(path.join(h.root, 'scripts')); fs.writeFileSync(path.join(h.root, 'scripts/zotero.py'), '# fixture');
  await h.command('openZotero'); const call = h.spawned[0]; assert.equal(call.binary, 'python'); assert.equal(call.data.shell, false); assert.equal(call.data.windowsHide, true); assert.equal(path.relative(call.args[0], path.join(h.root, 'scripts/zotero.py')), ''); assert.deepEqual(call.args.slice(1), ['start', '--port', '23119', '--uri', 'zotero://select/library']); assert.equal(h.errors.length, 0);
});

test('snapshot merge strips private fields, retains other libraries and prevents path escapes', t => {
  const root = scratch('public-references-'); t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const contentRoot = path.join(root, 'content'); fs.mkdirSync(contentRoot);
  const item = publicOnly({ id: 'cite', title: '论文', type: 'article-journal', author: [{ family: '研究者', given: '甲', tags: ['private'] }], issued: { 'date-parts': [[2024, 1, 3]], raw: 'private' }, URL: 'file:///C:/private.pdf', annotationText: 'private', apiKey: 'private' });
  assert.deepEqual(item.author, [{ family: '研究者', given: '甲' }]); assert(!('URL' in item)); assert(!('apiKey' in item)); assert(!('annotationText' in item));
  mergeSnapshot(contentRoot, [item]); mergeSnapshot(contentRoot, [{ id: 'second', title: '另一篇' }]); assert.equal(JSON.parse(fs.readFileSync(path.join(root, 'data/references.json'))).items.length, 2);
  fs.writeFileSync(path.join(root, 'data/references.json'), 'broken'); assert.throws(() => mergeSnapshot(contentRoot, [item]), /有效的 JSON/); assert.equal(fs.existsSync(path.join(root, 'data/references.json.lock')), false);
});

test('citation extraction and draft rendering share the article masking rules', () => {
  const root = path.resolve('sample-content'), file = path.join(root, 'posts', 'sample', 'index.md');
  const text = '+++\ntitle = "[@meta]"\n+++\n[@keep, p. 4; @grouped, sec. 2]\n`[@inline]`\n$[@math]$\n\n    [@indented]\n\n```\n[@fenced]\n```\n<!--[@comment]-->';
  assert.deepEqual([...citationKeys(text, file, root)], ['keep', 'grouped']);
  assert.match(draftMarkdown('+++\ntitle="旧标题"\ndraft=false\n+++\n\n批注正文', '新标题'), /draft = true/);
  assert.equal(validateMessage({ type: 'openAnnotation', key: PAPER, attachmentKey: PDF, annotationKey: ANNOTATION }), true);
  assert.equal(validateMessage({ type: 'filter', q: 'x', collectionKey: '', tag: '' }), true);
  assert.equal(validateMessage({ type: 'filter', q: 'x\n', collectionKey: '', tag: '' }), false);
  assert.equal(validateMessage({ type: 'filter', q: 'x', collectionKey: '', tag: '', extra: true }), false);
  assert.throws(() => referenceView('" onload="unsafe'), /nonce/);
});
