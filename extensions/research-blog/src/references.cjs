'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { createHash, randomBytes, randomUUID } = require('node:crypto');
const { spawn } = require('node:child_process');
const { URI } = require('vscode-uri');
const { parse: parseToml, stringify: stringifyToml } = require('smol-toml');
const core = require('./collections-core.cjs');
const { INSTALL_HINT, blogctlPath, blogRoot } = require('./blogctl.cjs');
const { parseArticle } = require('./engine.cjs');
const { extractCitations } = require('./reference-index.cjs');
const markdown = new (require('markdown-it'))({ html: true });
const { referenceView } = require('./references-view.cjs');

const MAX_CACHE_BYTES = 32 * 1024 * 1024;
const KEY = /^[23456789ABCDEFGHIJKLMNPQRSTUVWXYZ]{8}$/;
const CITEKEY = /^[\p{L}\p{N}_.:+/-]{1,200}$/u;
const PUBLIC_FIELDS = ['id', 'citekey', 'title', 'author', 'issued', 'type', 'container-title', 'DOI', 'URL', 'volume', 'issue', 'page', 'publisher'];
const plain = (value, max = 5000) => String(value ?? '').replace(/<(script|style)\b[^>]*>[\s\S]*?<\/\1>/gi, '').replace(/<[^>]*>/g, '').replace(/&(?:lt|gt|amp|quot|apos|nbsp);/g, code => ({ '&lt;': '<', '&gt;': '>', '&amp;': '&', '&quot;': '"', '&apos;': "'", '&nbsp;': ' ' }[code])).replace(/&#(x[0-9a-f]+|\d+);/gi, (_, code) => { const n = code[0].toLowerCase() === 'x' ? parseInt(code.slice(1), 16) : Number(code); return n > 0 && n <= 0x10ffff ? String.fromCodePoint(n) : ''; }).replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g, '').slice(0, max);
const dataOf = item => item?.data || item || {};
const keyOf = item => item?.key || item?.data?.key;
function authorText(creators) { return (creators || []).filter(c => !c.creatorType || ['author', 'editor', 'inventor'].includes(c.creatorType)).map(c => plain(c.name || `${c.firstName || ''} ${c.lastName || ''}`.trim(), 150)).filter(Boolean).join('、'); }
function citationKeys(text, file, contentRoot) {
  const visible = parseArticle(URI.file(file).toString(), text, contentRoot).visible;
  return new Set(extractCitations(visible).map(citation => citation.key).filter(key => CITEKEY.test(key)));
}
function citationInsertionError(article, offset) {
  if (offset < article.bodyStart) return '请将光标放在文章正文中，再插入引用。';
  const offsets = [0]; for (const match of article.text.matchAll(/\n/g)) offsets.push(match.index + 1);
  // The same Markdown tokenizer and masking spans as parseArticle keep a caret
  // inside an otherwise blank code line, inline code or formula from inserting.
  for (const token of markdown.parse(article.text, {})) if (['fence', 'code_block'].includes(token.type) && token.map) {
    const start = offsets[token.map[0]], end = offsets[token.map[1]] ?? article.text.length;
    if (offset >= start && offset < end) return '请将光标移到代码或公式之外，再插入引用。';
  }
  for (const expression of [/<(?:pre|code|script|style|textarea)\b[^>]*>[\s\S]*?<\/(?:pre|code|script|style|textarea)>/gi, /<!--[\s\S]*?-->/g, /(`+)[\s\S]*?\1(?!`)/g, /(?<!\\)\$\$[\s\S]*?(?<!\\)\$\$/g, /(?<![\\$])\$(?!\$)[^\n$]*?(?<!\\)\$/g, /\\\[[\s\S]*?\\\]/g, /\\\([\s\S]*?\\\)/g]) {
    for (const match of article.text.matchAll(expression)) if (offset > match.index && offset < match.index + match[0].length) return '请将光标移到代码或公式之外，再插入引用。';
  }
}
function publicOnly(item) {
  if (!item || typeof item !== 'object' || Array.isArray(item)) throw new Error('公开书目信息格式错误。');
  const result = {};
  for (const field of PUBLIC_FIELDS) if (item[field] !== undefined) {
    if (field === 'author') {
      if (!Array.isArray(item[field])) throw new Error('公开作者信息格式错误。');
      result[field] = item[field].map(author => Object.fromEntries(['family', 'given', 'literal'].filter(k => typeof author?.[k] === 'string').map(k => [k, plain(author[k], 300)])));
    } else if (field === 'issued') {
      const parts = item.issued?.['date-parts'];
      if (!Array.isArray(parts) || !parts.length || parts.some(p => !Array.isArray(p) || p.length < 1 || p.length > 3 || p.some(n => !Number.isInteger(n)))) throw new Error('公开书目的日期格式错误。');
      result.issued = { 'date-parts': parts.map(p => [...p]) };
    } else if (field === 'URL') {
      try { const url = new URL(item.URL); if (['http:', 'https:'].includes(url.protocol) && !url.username && !url.password && !['localhost', '127.0.0.1', '[::1]'].includes(url.hostname.toLowerCase())) result.URL = url.href; } catch { /* Private/non-web addresses are omitted. */ }
    } else if (typeof item[field] === 'string' || typeof item[field] === 'number') result[field] = plain(item[field], field === 'title' ? 2000 : 1000);
  }
  result.id ||= result.citekey;
  if (!CITEKEY.test(result.id || '') || typeof result.title !== 'string') throw new Error('公开书目需要有效的引用键与标题。');
  result.citekey ||= result.id;
  return result;
}
function snapshotFile(contentRoot) { return core.safePath(path.dirname(contentRoot), 'data', 'references.json'); }
function mergeSnapshot(contentRoot, incoming) {
  const file = snapshotFile(contentRoot);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const lock = core.safePath(path.dirname(file), 'references.json.lock');
  let fd;
  try { fd = fs.openSync(lock, 'wx'); }
  catch (error) { if (error.code === 'EEXIST') throw new Error('另一个窗口正在写入引用快照，请稍后重试。'); throw error; }
  const temp = core.safePath(path.dirname(file), `references-${randomUUID()}.tmp`);
  try {
    let previous = [];
    if (fs.existsSync(file)) {
      if (fs.statSync(file).size > MAX_CACHE_BYTES) throw new Error('引用快照文件过大。');
      let snapshot; try { snapshot = JSON.parse(fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, '')); } catch { throw new Error('references.json 不是有效的 JSON，请先修复此文件。'); }
      if (snapshot.version !== 1 || !Array.isArray(snapshot.items)) throw new Error('references.json 需要 version: 1 与 items 数组。');
      previous = snapshot.items.map(publicOnly);
    }
    const byId = new Map(previous.map(item => [item.id, item]));
    for (const item of incoming.map(publicOnly)) byId.set(item.id, item);
    const snapshot = { version: 1, items: [...byId.values()].sort((a, b) => a.id.localeCompare(b.id)) };
    fs.writeFileSync(temp, JSON.stringify(snapshot, null, 2) + '\n', { flag: 'wx' });
    fs.renameSync(temp, file);
    return { file, count: incoming.length, total: snapshot.items.length };
  } finally { if (fs.existsSync(temp)) fs.unlinkSync(temp); fs.closeSync(fd); fs.unlinkSync(lock); }
}
function draftMarkdown(markdown, title) {
  if (typeof markdown !== 'string' || markdown.length > 4 * 1024 * 1024) throw new Error('批注笔记内容无效或过大。');
  const match = /^\uFEFF?\+\+\+\s*\r?\n([\s\S]*?)^\+\+\+\s*\r?\n?/m.exec(markdown);
  let metadata = {}, body = markdown;
  if (match && match.index === 0) { metadata = parseToml(match[1]); body = markdown.slice(match[0].length); }
  metadata.title = core.titleText(title); metadata.draft = true; metadata.description ||= '';
  return `+++\n${stringifyToml(metadata).trim()}\n+++\n\n${body.trim()}\n`;
}
function validateMessage(message) {
  if (!message || typeof message !== 'object' || Array.isArray(message)) return false;
  const schemas = {
    ready: [], refresh: [], authorize: [], importPDF: [], newCollection: [], export: [], openZotero: [],
    select: ['key'], insertCitation: ['key'], openPDF: ['key'], createNote: ['key'], editReference: ['key'], renameCollection: ['key'],
    openAnnotation: ['key', 'attachmentKey', 'annotationKey'], openArticle: ['articleKey'], filter: ['q', 'collectionKey', 'tag'],
  };
  const fields = schemas[message.type]; if (!fields) return false;
  if (Object.keys(message).some(field => field !== 'type' && !fields.includes(field))) return false;
  return fields.every(field => typeof message[field] === 'string' && (field === 'articleKey' ? (() => { try { core.articleKey(message[field]); return true; } catch { return false; } })() : field === 'q' || field === 'tag' ? message[field].length <= 200 && !/[\x00-\x1f]/.test(message[field]) : field === 'collectionKey' && message[field] === '' || KEY.test(message[field])));
}

function registerReferences(context, injected = {}) {
  const vscode = injected.vscode || require('vscode');
  const zotero = injected.zotero || require('./zotero-core.cjs');
  const { chooseScope, trusted } = injected.writing || require('./writing.cjs');
  const states = new Map();
  const changes = new vscode.EventEmitter(); context.subscriptions.push(changes);
  let disposed = false;
  const setting = scope => vscode.workspace.getConfiguration('researchBlog.zotero', scope.folder.uri);
  const identity = scope => `${scope.folder.uri.toString()}\n${setting(scope).get('baseUrl', 'http://127.0.0.1:23119/api/')}\n${setting(scope).get('libraryPath', 'users/0')}`;
  const hash = value => createHash('sha256').update(value).digest('hex');
  const secretName = state => `researchBlog.zotero.${hash(identity(state.scope)).slice(0, 24)}`;
  function safeError(state, error) {
    let message = plain(error?.message || '操作失败，请重试。', 800);
    for (const key of [state?.client?.apiKey, state?.transientKey].filter(Boolean)) message = message.split(key).join('[密钥已隐藏]');
    return message;
  }
  function cachePath(state) {
    const storage = context.globalStorageUri || context.storageUri;
    if (!storage || storage.scheme !== 'file') return;
    return core.safePath(storage.fsPath, `zotero-${hash(identity(state.scope)).slice(0, 32)}.json`);
  }
  function loadCache(state) {
    try {
      const file = cachePath(state); if (!file || !fs.existsSync(file) || fs.statSync(file).size > MAX_CACHE_BYTES) return;
      const cache = JSON.parse(fs.readFileSync(file, 'utf8'));
      if (cache.version === 1 && Array.isArray(cache.items) && Array.isArray(cache.collections)) {
        state.items = cache.items.filter(item => KEY.test(keyOf(item) || '')); state.collections = cache.collections.filter(item => KEY.test(keyOf(item) || ''));
        state.details = new Map(Object.entries(cache.details || {}).filter(([key, value]) => KEY.test(key) && Array.isArray(value?.attachments) && Array.isArray(value?.annotations)).map(([key, value]) => [key, { attachments: value.attachments.filter(item => KEY.test(keyOf(item) || '')), annotations: value.annotations.filter(item => KEY.test(keyOf(item) || '')) }]));
        state.serverID = cache.serverID || state.items[0]?.serverID || '';
        state.updatedAt = cache.updatedAt; state.cached = true;
      }
    } catch { /* A damaged private cache is never treated as a live connection. */ }
  }
  function saveCache(state) {
    const file = cachePath(state); if (!file) return;
    fs.mkdirSync(path.dirname(file), { recursive: true });
    // The live API stamps provenance with non-enumerable properties. Retain it
    // in the private cache for offline group-library PDF links; writable() always
    // refreshes live wrappers before any mutation.
    const cachedItem = item => ({ ...item, libraryPath: item.libraryPath || setting(state.scope).get('libraryPath', 'users/0'), serverID: item.serverID || state.client?.serverID || '' });
    const cache = JSON.stringify({ version: 1, serverID: state.serverID || '', updatedAt: state.updatedAt, items: state.items.map(cachedItem), collections: state.collections.map(cachedItem), details: Object.fromEntries([...state.details].map(([key, value]) => [key, { attachments: value.attachments.map(cachedItem), annotations: value.annotations.map(cachedItem) }])) });
    if (Buffer.byteLength(cache) > MAX_CACHE_BYTES) return;
    const temp = core.safePath(path.dirname(file), `${path.basename(file)}-${randomUUID()}.tmp`);
    try { fs.writeFileSync(temp, cache, { flag: 'wx', mode: 0o600 }); fs.renameSync(temp, file); }
    finally { if (fs.existsSync(temp)) fs.unlinkSync(temp); }
  }
  function normalized(item) { return zotero.normalizeItem(item); }
  function viewItem(item) {
    const n = normalized(item);
    return { key: n.key || keyOf(item), title: plain(n.title, 2000), authors: authorText(n.creators), year: plain(n.year, 40), citationKey: n.citationKey || n.citekey, tags: (n.tags || []).map(t => plain(typeof t === 'string' ? t : t.tag, 200)), collections: n.collections || [], itemType: plain(n.itemType, 100), doi: plain(n.doi || n.DOI, 500), abstract: plain(n.abstract || n.abstractNote, 20000) };
  }
  function linkedArticles(state, citekey) {
    const links = [], retained = new Set();
    for (const article of core.listArticles(state.scope.contentRoot)) {
      retained.add(article.key);
      try {
        const open = (vscode.workspace.textDocuments || []).find(doc => doc.uri.scheme === 'file' && path.relative(doc.uri.fsPath, article.file) === '');
        const source = open ? open.getText() : fs.readFileSync(article.file, 'utf8');
        const signature = hash(source); let cached = state.articleLinks.get(article.key);
        if (!cached || cached.signature !== signature) {
          const parsed = parseArticle(URI.file(article.file).toString(), source, state.scope.contentRoot);
          cached = { signature, title: plain(parsed.metadata.title || article.title, 500), draft: parsed.metadata.draft === true, keys: new Set(extractCitations(parsed.visible).map(c => c.key)) };
          state.articleLinks.set(article.key, cached);
        }
        if (cached.keys.has(citekey)) links.push({ articleKey: article.key, title: cached.title, draft: cached.draft });
      } catch { /* An article being removed or edited must not break the library. */ }
    }
    for (const key of state.articleLinks.keys()) if (!retained.has(key)) state.articleLinks.delete(key);
    return links;
  }
  function publish(state) {
    if (!state.panel || disposed) return;
    const items = state.items.map(viewItem).filter(item => KEY.test(item.key || ''));
    const filter = state.filter;
    const matches = items.filter(item => (!filter.collectionKey || item.collections.includes(filter.collectionKey)) && (!filter.tag || item.tags.includes(filter.tag)) && (!filter.q || [item.title, item.authors, item.year, item.doi, item.citationKey, ...item.tags].join(' ').toLocaleLowerCase().includes(filter.q.toLocaleLowerCase())));
    const selected = items.find(item => item.key === state.selectedKey), details = state.details.get(state.selectedKey);
    const collections = state.collections.map(c => ({ key: keyOf(c), name: plain(dataOf(c).name || c.name, 200), parentCollection: dataOf(c).parentCollection || null })).filter(c => KEY.test(c.key || ''));
    const annotation = a => ({ key: keyOf(a), attachmentKey: a.attachmentKey || dataOf(a).parentItem, text: plain(dataOf(a).annotationText || dataOf(a).text), comment: plain(dataOf(a).annotationComment || dataOf(a).comment), page: plain(dataOf(a).annotationPageLabel || '', 30) });
    const detail = selected ? { ...selected, citedBy: linkedArticles(state, selected.citationKey), collectionNames: collections.filter(c => selected.collections.includes(c.key)).map(c => c.name).join('、'), loading: state.detailLoading === selected.key, attachments: (details?.attachments || []).map(a => ({ key: keyOf(a), title: plain(dataOf(a).title, 200) })), annotations: (details?.annotations || []).map(annotation), annotationError: state.detailErrors.get(selected.key) || '' } : undefined;
    let statusLabel = state.busy ? '正在同步本机文献库…' : state.online ? state.canWrite ? '已连接 Zotero · 可浏览与修改' : state.writeSupported ? '已连接 Zotero · 只读，授权后可修改' : '已连接 Zotero · 此版本仅支持读取' : state.cached ? '离线缓存 · 可浏览和引用，修改需连接 Zotero' : '未连接 Zotero';
    if (!state.busy && state.updatedAt) statusLabel += ` · 上次读取 ${new Date(state.updatedAt).toLocaleString()}`;
    void state.panel.webview.postMessage({ type: 'state', state: { items: matches, total: items.length, collections, tags: [...new Set(items.flatMap(item => item.tags))].sort((a, b) => a.localeCompare(b)), selectedKey: state.selectedKey, filter, detail, busy: state.busy, online: state.online, canWrite: state.canWrite, writeSupported: state.writeSupported, statusLabel, error: state.error } });
    changes.fire();
  }
  async function getState(arg) {
    trusted();
    const scope = await chooseScope(arg && typeof arg === 'object' ? arg : undefined); if (!scope) return;
    const id = scope.folder.uri.toString(); let state = states.get(id);
    if (!state) {
      state = { scope, items: [], collections: [], details: new Map(), detailErrors: new Map(), articleLinks: new Map(), filter: { q: '', collectionKey: '', tag: '' }, online: false, canWrite: false, writeSupported: false, busy: false, error: '', selectedKey: '', revision: 0, editor: vscode.window.activeTextEditor };
      states.set(id, state); loadCache(state);
    }
    return state;
  }
  async function clientFor(state) {
    const config = setting(state.scope);
    if (state.transientKey && state.client && state.client.apiKey !== state.transientKey) state.transientKey = undefined;
    const apiKey = state.transientKey || await context.secrets.get(secretName(state));
    const base = new URL(config.get('baseUrl', 'http://127.0.0.1:23119/api/')); if (base.pathname === '/api') base.pathname = '/api/';
    const options = { baseUrl: base.href, libraryPath: config.get('libraryPath', 'users/0'), apiKey };
    if (!state.client || state.client.baseUrl?.toString() !== options.baseUrl || state.client.libraryPath !== options.libraryPath || state.client.apiKey !== apiKey) state.client = new zotero.ZoteroClient(options);
    return state.client;
  }
  async function refresh(state, automatic = false) {
    if (state.inflight) return state.inflight;
    trusted(); state.busy = true; state.error = ''; publish(state);
    const revision = state.revision;
    state.inflight = (async () => {
      try {
        const client = await clientFor(state), previousKey = client.apiKey; const status = await client.probe();
        if (previousKey && !client.apiKey && state.revision === revision) { state.transientKey = undefined; await context.secrets.delete?.(secretName(state)); }
        const [collections, items] = await Promise.all([client.listCollections(), client.listItems({})]);
        if (state.revision !== revision || disposed) return;
        if (!Array.isArray(collections) || !Array.isArray(items)) throw new Error('Zotero 返回的文献库格式不正确。');
        const serverID = status.serverID || client.serverID || '';
        if (state.serverID && serverID && state.serverID !== serverID) { state.details.clear(); state.detailErrors.clear(); state.selectedKey = ''; }
        state.serverID = serverID;
        state.collections = collections; state.items = items; state.online = !!(status.canRead ?? status.read); state.writeSupported = !!status.writeSupported; state.canWrite = !!(status.canWrite ?? status.writeAuthorized); state.updatedAt = new Date().toISOString(); state.cached = true; state.error = '';
        try { saveCache(state); } catch (error) { state.error = `已读取文献库，但私有缓存未保存：${safeError(state, error)}`; }
      } catch (error) {
        if (state.revision !== revision || disposed) return;
        state.online = false; state.canWrite = false; state.writeSupported = false;
        state.error = `${safeError(state, error)}\n请打开 Zotero，并检查本机通信设置。${state.cached ? '当前显示上次保存的文献库。' : ''}`;
        if (!automatic && !state.panel) await vscode.window.showErrorMessage(`Research Blog: ${state.error}`);
      } finally { state.busy = false; state.inflight = undefined; publish(state); changes.fire(); }
    })();
    return state.inflight;
  }
  async function needItems(state) { if (!state.items.length && !state.online) await refresh(state); }
  async function pickItem(state, arg) {
    const requested = typeof arg === 'string' ? arg : arg?.key;
    await needItems(state);
    if (requested) {
      if (!KEY.test(requested)) throw new Error('文献标识无效。');
      const item = state.items.find(i => keyOf(i) === requested); if (!item) throw new Error('这篇文献已不在当前文献库中，请刷新。');
      return item;
    }
    const choices = state.items.map(item => { const n = viewItem(item); return { label: n.title || '未命名文献', description: [n.authors, n.year].filter(Boolean).join(' · '), detail: `[@${n.citationKey}]`, item }; });
    if (!choices.length) { await vscode.window.showInformationMessage('文献库为空。打开文献工作台连接 Zotero 或导入 PDF。'); return; }
    const revision = state.revision;
    const choice = await vscode.window.showQuickPick(choices, { title: '选择文献', matchOnDescription: true, matchOnDetail: true });
    if (state.revision !== revision) throw new Error('文献库设置已改变，请重新选择文献。');
    return choice?.item;
  }
  async function detailsFor(state, item) {
    const key = keyOf(item);
    if (!state.online) return state.details.get(key) || { attachments: [], annotations: [] };
    const revision = state.revision;
    state.detailLoading = key; state.detailErrors.delete(key); publish(state);
    try {
      const client = await clientFor(state), children = await client.children(key);
      const attachments = children.filter(c => dataOf(c).itemType === 'attachment' && (dataOf(c).contentType === 'application/pdf' || /\.pdf$/i.test(dataOf(c).filename || '')));
      const annotations = children.filter(c => dataOf(c).itemType === 'annotation');
      for (const attachment of attachments) {
        const nested = await client.children(keyOf(attachment));
        annotations.push(...nested.filter(c => dataOf(c).itemType === 'annotation').map(c => ({ ...c, attachmentKey: keyOf(attachment) })));
      }
      if (state.revision !== revision) throw new Error('文献库设置已改变，请重新选择文献。');
      const details = { attachments, annotations }; state.details.set(key, details); try { saveCache(state); } catch { /* Existing cached library remains usable. */ } return details;
    } catch (error) { state.detailErrors.set(key, safeError(state, error)); throw error; }
    finally { if (state.detailLoading === key) state.detailLoading = undefined; publish(state); }
  }
  async function writable(state) {
    await refresh(state, true);
    if (!state.online) throw new Error('当前离线。请先连接 Zotero，再修改文献库。');
    if (!state.writeSupported) throw new Error('此 Zotero 版本只支持读取。请升级到 Zotero 10 或更高版本。');
    if (!state.canWrite) throw new Error('请先在文献工作台点击「授权修改」，并在 Zotero 中确认。');
    return clientFor(state);
  }
  async function launchZotero(state, uri) {
    trusted();
    const repo = blogRoot(state.scope.contentRoot);
    const binary = blogctlPath(vscode, state.scope.folder.uri);
    await new Promise((resolve, reject) => {
      const port = new URL(setting(state.scope).get('baseUrl', 'http://127.0.0.1:23119/api/')).port || '80';
      const child = (injected.spawn || spawn)(binary, ['--root', repo, 'zotero', 'start', '--port', port, ...(uri ? ['--uri', uri] : [])], { cwd: repo, windowsHide: true, shell: false, stdio: 'ignore' });
      child.once('error', () => reject(new Error(INSTALL_HINT)));
      child.once('exit', code => code === 0 ? resolve() : reject(new Error('Zotero 启动助手失败。请在终端运行 blogctl zotero status 查看状态。')));
    });
    for (let attempt = 0; attempt < 8; attempt++) {
      await refresh(state, true); if (state.online) return true;
      if (attempt < 7) await new Promise(resolve => setTimeout(resolve, 700));
    }
    throw new Error('已发送 Zotero 启动请求，但本机 API 尚未连接。请检查 Zotero 窗口与通信设置，再点击刷新。');
  }
  async function openZoteroURI(state, uri) {
    let opened = false;
    try { opened = await vscode.env.openExternal(vscode.Uri.parse(uri)); } catch { /* The portable client may not register a URI protocol. */ }
    if (!opened) await launchZotero(state, uri);
  }
  function incomingReference(item) { return publicOnly(zotero.toPublicReference(item)); }
  async function lastEditor(state) {
    let editor = vscode.window.activeTextEditor || state.editor;
    if (!editor || editor.document.uri.scheme !== 'file' || editor.document.languageId !== 'markdown') throw new Error('请先在编辑器中打开需要引用的 Markdown 文章。');
    const relative = path.relative(state.scope.contentRoot, editor.document.uri.fsPath).split(path.sep).join('/'); core.articleKey(relative); core.safePath(state.scope.contentRoot, relative);
    if (editor !== vscode.window.activeTextEditor || Array.isArray(vscode.window.visibleTextEditors) && !vscode.window.visibleTextEditors.includes(editor)) {
      // A webview can hide the original editor. VS Code refuses edit() on that
      // stale TextEditor, so reveal the document and restore its cursor first.
      const selection = editor.selection;
      editor = await vscode.window.showTextDocument(editor.document, { viewColumn: editor.viewColumn || state.editorColumn || vscode.ViewColumn.One, preserveFocus: true, preview: false, selection });
      editor.selection = selection;
      state.editor = editor;
    }
    return editor;
  }
  async function insertCitation(state, item) {
    const editor = await lastEditor(state), n = normalized(item), citekey = n.citationKey || n.citekey;
    if (!CITEKEY.test(citekey || '')) throw new Error('这篇文献的引用键无效。请在 Zotero 中检查 Citation Key。');
    const text = editor.document.getText(); const article = parseArticle(editor.document.uri.toString(), text, state.scope.contentRoot);
    const offset = editor.document.offsetAt(editor.selection.active || editor.selection.end);
    const error = citationInsertionError(article, offset); if (error) throw new Error(error);
    mergeSnapshot(state.scope.contentRoot, [incomingReference(item)]);
    const applied = await editor.edit(edit => edit.insert(editor.selection.active || editor.selection.end, `[@${citekey}]`));
    if (!applied) throw new Error('编辑器未能插入引用。请检查文章是否只读。');
    return citekey;
  }
  async function openPDF(state, item, annotation) {
    const details = state.details.get(keyOf(item)) || await detailsFor(state, item);
    let attachment;
    if (annotation) {
      const known = details.annotations.find(a => keyOf(a) === annotation.annotationKey && (a.attachmentKey || dataOf(a).parentItem) === annotation.attachmentKey);
      if (!known) throw new Error('批注已改变，请重新读取文献详情。');
      attachment = details.attachments.find(a => keyOf(a) === annotation.attachmentKey);
      if (!attachment) throw new Error('批注的 PDF 不在当前文献中。');
      let position = dataOf(known).annotationPosition; try { if (typeof position === 'string') position = JSON.parse(position); } catch { position = undefined; }
      const label = dataOf(known).annotationPageLabel;
      const page = Number.isInteger(position?.pageIndex) && position.pageIndex >= 0 ? position.pageIndex + 1 : /^[1-9][0-9]*$/.test(label || '') ? Number(label) : undefined;
      const uri = zotero.openPDFURI(keyOf(attachment), page, keyOf(known), setting(state.scope).get('libraryPath', 'users/0'));
      await openZoteroURI(state, uri); return;
    }
    if (!details.attachments.length) throw new Error('这篇文献没有 PDF 附件。请先在 Zotero 中添加 PDF。');
    attachment = details.attachments.length === 1 ? details.attachments[0] : (await vscode.window.showQuickPick(details.attachments.map(a => ({ label: plain(dataOf(a).title || dataOf(a).filename || 'PDF'), item: a })), { title: '选择 PDF 附件' }))?.item;
    if (!attachment) return;
    await openZoteroURI(state, zotero.openPDFURI(keyOf(attachment), undefined, undefined, setting(state.scope).get('libraryPath', 'users/0')));
  }
  async function createNote(state, item) {
    const revision = state.revision;
    const details = await detailsFor(state, item);
    if (!details.annotations.length) throw new Error('这篇文献还没有可导出的 PDF 批注。');
    const n = normalized(item); const defaultTitle = `${plain(n.title, 108)} · 阅读笔记`;
    const title = await vscode.window.showInputBox({ title: '批注生成阅读草稿', value: defaultTitle, ignoreFocusOut: true, validateInput: value => { try { core.titleText(value); } catch (e) { return e.message; } } }); if (!title) return;
    if (state.revision !== revision) throw new Error('文献库设置已改变，请重新选择文献。');
    const markdown = draftMarkdown(zotero.annotationsToMarkdown(item, details.annotations), title);
    mergeSnapshot(state.scope.contentRoot, [incomingReference(item)]);
    const article = core.createArticle(state.scope.contentRoot, title, null);
    try { fs.writeFileSync(core.safePath(state.scope.contentRoot, article.key), markdown); }
    catch (error) { throw new Error(`草稿目录已创建到 ${article.file}，但批注未写入：${safeError(state, error)}`); }
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(article.file));
    await vscode.window.showInformationMessage('阅读草稿已创建。请检查批注和自己的分析，再决定是否发布。');
    return article;
  }
  async function exportReferences(state) {
    await needItems(state);
    const requested = new Set();
    for (const article of core.listArticles(state.scope.contentRoot)) {
      const open = (vscode.workspace.textDocuments || []).find(doc => doc.uri.scheme === 'file' && path.relative(doc.uri.fsPath, article.file) === '');
      for (const key of citationKeys(open ? open.getText() : fs.readFileSync(article.file, 'utf8'), article.file, state.scope.contentRoot)) requested.add(key);
    }
    const incoming = state.items.filter(item => { const n = normalized(item); return requested.has(n.citationKey || n.citekey); }).map(incomingReference);
    const result = mergeSnapshot(state.scope.contentRoot, incoming);
    const included = new Set(incoming.map(item => item.id));
    const missing = [...requested].filter(key => !included.has(key));
    await vscode.window.showInformationMessage(`已更新 ${result.count} 条文章引用的公开书目。${missing.length ? `另有 ${missing.length} 个引用不在当前文献库中，已有快照已保留。` : ''}`);
    return result;
  }
  async function authorize(state) {
    await refresh(state, true);
    if (!state.online) throw new Error('请先打开 Zotero 并开启本机通信。');
    if (!state.writeSupported) throw new Error('写入授权需要 Zotero 10 或更高版本，当前可继续只读浏览。');
    const client = await clientFor(state), result = await client.authorizeWrites('Research Blog');
    if (!result?.key) throw new Error('Zotero 尚未授予修改权限。');
    if (result.remember) { await context.secrets.store(secretName(state), result.key); state.transientKey = undefined; }
    else state.transientKey = result.key;
    await refresh(state, true);
    if (!state.canWrite) throw new Error('授权未生效，请检查 Zotero 中的授权提示。');
    await vscode.window.showInformationMessage(result.remember ? '已获得 Zotero 修改权限，密钥保存在 VS Code 私有凭据存储中。' : '已获得本次 Zotero 修改权限。Zotero 可能在操作后再次要求授权。');
  }
  async function importPDF(state) {
    const client = await writable(state);
    const revision = state.revision;
    const files = await vscode.window.showOpenDialog({ title: '导入 PDF 到 Zotero', canSelectMany: false, filters: { PDF: ['pdf'] } });
    if (!files?.length) return;
    if (files[0].scheme !== 'file') throw new Error('请选择本机 PDF 文件。');
    const title = await vscode.window.showInputBox({ title: '文献标题（后续可在 Zotero 中补全文献信息）', value: path.basename(files[0].fsPath, path.extname(files[0].fsPath)), validateInput: value => { try { core.titleText(value); } catch (e) { return e.message; } } }); if (!title) return;
    if (state.revision !== revision) throw new Error('文献库设置已改变，请重新选择 PDF。');
    let result;
    try { result = await client.importPDF(files[0].fsPath, { title, collectionKey: state.filter.collectionKey || undefined }); }
    catch (error) { if (state.revision === revision && (error.partialItem || error.partialAttachment)) { state.selectedKey = keyOf(error.partialItem) || ''; await refresh(state, true); } throw error; }
    if (!result?.item || !result?.attachment) throw new Error('PDF 导入未返回完整文献和附件，请在 Zotero 中检查导入状态。');
    state.selectedKey = keyOf(result.item); await refresh(state, true);
    await vscode.window.showInformationMessage('PDF 已导入 Zotero。标题、作者、年份等信息可继续在 Zotero 中补全。');
    return result;
  }
  async function createCollection(state) {
    const client = await writable(state);
    const revision = state.revision;
    const name = await vscode.window.showInputBox({ title: '新建文献分类', validateInput: value => { try { core.titleText(value); } catch (e) { return e.message; } } }); if (!name) return;
    const parent = await vscode.window.showQuickPick([{ label: '作为顶层分类', key: null }, ...state.collections.map(c => ({ label: plain(dataOf(c).name), key: keyOf(c) }))], { title: '选择上级分类' }); if (!parent) return;
    if (state.revision !== revision) throw new Error('文献库设置已改变，请重新创建分类。');
    const result = await client.createCollection(core.titleText(name), parent.key); await refresh(state, true); return result;
  }
  async function renameCollection(state, arg) {
    const client = await writable(state); const revision = state.revision; let collection = state.collections.find(c => keyOf(c) === arg?.key);
    if (!collection) collection = (await vscode.window.showQuickPick(state.collections.map(c => ({ label: plain(dataOf(c).name), collection: c })), { title: '选择需要重命名的分类' }))?.collection;
    if (!collection) return;
    const name = await vscode.window.showInputBox({ title: '重命名文献分类', value: plain(dataOf(collection).name), validateInput: value => { try { core.titleText(value); } catch (e) { return e.message; } } }); if (!name) return;
    if (state.revision !== revision) throw new Error('文献库设置已改变，请重新选择分类。');
    const result = await client.updateCollection(collection, { name: core.titleText(name) }); await refresh(state, true); return result;
  }
  async function editReference(state, item) {
    const client = await writable(state), n = normalized(item);
    const revision = state.revision;
    if (item.serverID && client.serverID && item.serverID !== client.serverID || item.libraryPath && item.libraryPath !== client.libraryPath) throw new Error('Zotero 文献数据库或文献库已改变，请刷新后重新选择文献。');
    const chosen = await vscode.window.showQuickPick([{ label: '标题', field: 'title' }, { label: '标签', field: 'tags' }, { label: '所属分类', field: 'collections' }, { label: '日期 / 年份', field: 'date' }, { label: 'DOI', field: 'DOI' }], { title: '编辑哪项文献信息？' }); if (!chosen) return;
    let patch;
    if (chosen.field === 'collections') {
      const list = await vscode.window.showQuickPick(state.collections.map(c => ({ label: plain(dataOf(c).name), key: keyOf(c), picked: (n.collections || []).includes(keyOf(c)) })), { title: '选择所属分类（可多选，清空表示不分类）', canPickMany: true }); if (!list) return; patch = { collections: list.map(c => c.key) };
    } else {
      const value = await vscode.window.showInputBox({ title: chosen.field === 'tags' ? '编辑标签（用中文或英文逗号分隔）' : `编辑${chosen.label}`, value: chosen.field === 'tags' ? (n.tags || []).join('、') : plain(dataOf(item)[chosen.field] ?? n[chosen.field] ?? '', 2000), ignoreFocusOut: true, validateInput: value => chosen.field === 'title' ? (() => { try { core.titleText(value); } catch (e) { return e.message; } })() : value.length > 2000 || /[\x00-\x1f]/.test(value) ? '内容需为不超过 2000 字的单行文字。' : undefined }); if (value === undefined) return;
      patch = chosen.field === 'tags' ? { tags: [...new Set(value.split(/[,，、]/).map(t => t.trim()).filter(Boolean))].map(tag => ({ tag })) } : { [chosen.field]: chosen.field === 'title' ? core.titleText(value) : value.trim() };
    }
    if (state.revision !== revision) throw new Error('文献库设置已改变，请重新选择文献。');
    const latest = await client.getItem(keyOf(item));
    const result = await client.updateItem(latest, patch); state.details.delete(keyOf(item)); await refresh(state, true);
    if (result) {
      const citations = new Set(core.listArticles(state.scope.contentRoot).flatMap(article => [...citationKeys(fs.readFileSync(article.file, 'utf8'), article.file, state.scope.contentRoot)]));
      const edited = state.items.find(i => keyOf(i) === keyOf(item)); if (edited && citations.has(normalized(edited).citationKey || normalized(edited).citekey)) mergeSnapshot(state.scope.contentRoot, [incomingReference(edited)]);
    }
    return result;
  }
  async function openWorkbench(state) {
    if (state.panel) { state.panel.reveal(); return; }
    state.editor = vscode.window.activeTextEditor || state.editor;
    state.editorColumn = state.editor?.viewColumn || vscode.ViewColumn.One;
    const panel = vscode.window.createWebviewPanel('researchBlog.references', '文献工作台', { viewColumn: vscode.ViewColumn.Beside, preserveFocus: false }, { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [] }); state.panel = panel;
    panel.webview.html = referenceView(randomBytes(24).toString('hex'));
    context.subscriptions.push(panel.onDidDispose(() => { state.panel = undefined; clearInterval(state.timer); state.timer = undefined; }), panel.webview.onDidReceiveMessage(async message => {
      if (!validateMessage(message)) return;
      try {
        trusted();
        if (message.type === 'ready') { publish(state); await refresh(state, true); startTimer(state); }
        else if (message.type === 'filter') { state.filter = { q: message.q.trim(), collectionKey: message.collectionKey, tag: message.tag }; publish(state); }
        else if (message.type === 'select') { const item = await pickItem(state, message); if (item) { state.selectedKey = keyOf(item); publish(state); await detailsFor(state, item); } }
        else if (message.type === 'refresh') { await refresh(state); if (state.selectedKey && state.online) await detailsFor(state, await pickItem(state, { key: state.selectedKey })); }
        else if (message.type === 'authorize') await authorize(state);
        else if (message.type === 'importPDF') await importPDF(state);
        else if (message.type === 'newCollection') await createCollection(state);
        else if (message.type === 'renameCollection') await renameCollection(state, message);
        else if (message.type === 'export') await exportReferences(state);
        else if (message.type === 'openZotero') await openZoteroURI(state, 'zotero://select/library');
        else if (message.type === 'openArticle') {
          const file = core.safePath(state.scope.contentRoot, core.articleKey(message.articleKey));
          if (!fs.existsSync(file) || !fs.statSync(file).isFile()) throw new Error('这篇文章已不存在。');
          await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(file));
        }
        else { const item = await pickItem(state, message); if (!item) return;
          if (message.type === 'insertCitation') await insertCitation(state, item);
          else if (message.type === 'openPDF') await openPDF(state, item);
          else if (message.type === 'openAnnotation') await openPDF(state, item, message);
          else if (message.type === 'createNote') await createNote(state, item);
          else if (message.type === 'editReference') await editReference(state, item);
        }
        publish(state);
      } catch (error) { if (['AUTH_REQUIRED', 'SERVER_CHANGED'].includes(error.code)) { state.transientKey = undefined; state.canWrite = false; await context.secrets.delete?.(secretName(state)); } state.error = safeError(state, error); publish(state); await vscode.window.showErrorMessage(`Research Blog: ${state.error}`); }
    }));
    publish(state); await refresh(state, true); startTimer(state);
  }
  function startTimer(state) {
    clearInterval(state.timer);
    const seconds = Number(setting(state.scope).get('autoRefreshSeconds', 60));
    if (!state.panel || !Number.isFinite(seconds) || seconds <= 0) return;
    state.timer = setInterval(() => { if (state.panel?.visible && !state.inflight && !disposed) void refresh(state, true).catch(error => { state.error = safeError(state, error); publish(state); }); }, Math.max(15, Math.min(3600, seconds)) * 1000);
    state.timer.unref?.();
  }
  const command = (name, action) => context.subscriptions.push(vscode.commands.registerCommand(`researchBlog.${name}`, async arg => {
    let state;
    try { state = await getState(arg); if (!state) return; const result = await action(state, arg); publish(state); return result; }
    catch (error) { if (state) { if (['AUTH_REQUIRED', 'SERVER_CHANGED'].includes(error.code)) { state.transientKey = undefined; state.canWrite = false; await context.secrets.delete?.(secretName(state)); } state.error = safeError(state, error); publish(state); } await vscode.window.showErrorMessage(`Research Blog: ${safeError(state, error)}`); }
  }));
  command('openReferences', openWorkbench); command('refreshReferences', state => refresh(state)); command('authorizeZotero', authorize);
  command('insertCitation', async (state, arg) => { const item = await pickItem(state, arg); if (item) return insertCitation(state, item); });
  command('openReferencePDF', async (state, arg) => { const item = await pickItem(state, arg); if (item) return openPDF(state, item); });
  command('createReferenceNote', async (state, arg) => { const item = await pickItem(state, arg); if (item) return createNote(state, item); });
  command('importReferencePDF', importPDF); command('createReferenceCollection', createCollection); command('renameReferenceCollection', renameCollection);
  command('editReference', async (state, arg) => { const item = await pickItem(state, arg); if (item) return editReference(state, item); });
  command('exportReferences', exportReferences);
  command('openZotero', state => openZoteroURI(state, 'zotero://select/library'));
  command('startZotero', async state => { await launchZotero(state); await vscode.window.showInformationMessage('已连接本机 Zotero 文献库。'); });
  const provider = {
    onDidChangeTreeData: changes.event,
    getChildren() {
      return [{ label: '打开文献工作台', icon: 'library', command: 'researchBlog.openReferences' }, { label: '插入文献引用', icon: 'quote', command: 'researchBlog.insertCitation' }, { label: '从 PDF 批注创建阅读草稿', icon: 'note', command: 'researchBlog.createReferenceNote' }];
    },
    getTreeItem(node) { const item = new vscode.TreeItem(node.label, vscode.TreeItemCollapsibleState.None); item.iconPath = new vscode.ThemeIcon(node.icon); item.command = { command: node.command, title: node.label }; return item; },
  };
  context.subscriptions.push(vscode.window.createTreeView('researchBlog.references', { treeDataProvider: provider }));
  context.subscriptions.push(vscode.workspace.onDidChangeConfiguration(event => {
    if (!event.affectsConfiguration('researchBlog.zotero')) return;
    for (const state of states.values()) { state.revision++; state.client = undefined; state.transientKey = undefined; state.items = []; state.collections = []; state.details.clear(); state.cached = false; state.updatedAt = undefined; state.online = false; state.canWrite = false; loadCache(state); startTimer(state); publish(state); if (state.panel) void Promise.resolve(state.inflight).then(() => refresh(state, true)); }
  }));
  if (vscode.window.onDidChangeActiveTextEditor) context.subscriptions.push(vscode.window.onDidChangeActiveTextEditor(editor => {
    if (editor?.document.uri.scheme === 'file') for (const state of states.values()) if (core.inside(state.scope.contentRoot, editor.document.uri.fsPath)) state.editor = editor;
  }));
  if (vscode.workspace.onDidChangeTextDocument) context.subscriptions.push(vscode.workspace.onDidChangeTextDocument(event => {
    for (const state of states.values()) if (state.panel && state.selectedKey && event.document.uri.scheme === 'file' && core.inside(state.scope.contentRoot, event.document.uri.fsPath)) publish(state);
  }));
  if (vscode.workspace.createFileSystemWatcher) {
    const watcher = vscode.workspace.createFileSystemWatcher('**/*.md');
    const refreshLinks = () => { for (const state of states.values()) if (state.panel && state.selectedKey) publish(state); };
    context.subscriptions.push(watcher, watcher.onDidChange(refreshLinks), watcher.onDidCreate(refreshLinks), watcher.onDidDelete(refreshLinks));
  }
  const dispose = () => { disposed = true; for (const state of states.values()) { clearInterval(state.timer); state.transientKey = undefined; state.panel?.dispose(); } states.clear(); };
  context.subscriptions.push({ dispose });
  return { states, refresh, dispose };
}
module.exports = { registerReferences, citationKeys, citationInsertionError, publicOnly, mergeSnapshot, draftMarkdown, validateMessage, plain };
