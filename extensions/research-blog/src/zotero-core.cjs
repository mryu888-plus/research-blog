'use strict';
// Local API protocol: https://www.zotero.org/support/dev/web_api/v3/local_api
// This module intentionally has no web API fallback and never follows redirects.
const fs = require('node:fs');
const path = require('node:path');
const { createHash, randomBytes } = require('node:crypto');
const { Readable } = require('node:stream');

class ZoteroError extends Error {
  constructor(code, message, details = {}) { super(message); this.name = 'ZoteroError'; this.code = code; Object.assign(this, details); }
}
function fail(code, message, details) { throw new ZoteroError(code, message, details); }
function keyOf(value) {
  if (typeof value !== 'string' || !/^[23456789ABCDEFGHIJKLMNPQRSTUVWXYZ]{8}$/.test(value)) fail('INVALID_KEY', 'Zotero 条目或分类的标识无效。');
  return value;
}
function localURL(value) {
  let url;
  try { url = new URL(value); } catch { fail('INVALID_URL', '请输入 Zotero 本地 API 地址。'); }
  if (url.protocol !== 'http:' || !['localhost', '127.0.0.1', '[::1]'].includes(url.hostname.toLowerCase()) || url.username || url.password || url.hash || url.search) {
    fail('UNSAFE_URL', 'Zotero 适配仅允许本机 HTTP 地址（localhost、127.0.0.1 或 ::1）。');
  }
  if (!/^\/api\/?$/.test(url.pathname)) fail('INVALID_URL', 'Zotero 本地 API 地址必须以 /api/ 结尾。');
  url.pathname = '/api/'; return url;
}
function libraryPathOf(value) {
  if (typeof value !== 'string' || !/^(users|groups)\/(0|[1-9][0-9]*)$/.test(value) || value === 'groups/0') fail('INVALID_LIBRARY', '文献库必须是 users/0、users/用户编号或 groups/群组编号。');
  return value;
}
function text(value) { return typeof value === 'string' ? value.replace(/\u0000/g, '') : ''; }
function publicURL(value) {
  try {
    const url = new URL(value), hostname = url.hostname.toLowerCase();
    if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password || hostname === 'localhost' || hostname.endsWith('.localhost') || hostname.endsWith('.local') || /^\[(?:::1|f[cd][0-9a-f]*:|fe[89ab][0-9a-f]*:)/i.test(hostname)) return undefined;
    const octets = hostname.split('.').map(Number);
    if (octets.length === 4 && octets.every(n => Number.isInteger(n) && n >= 0 && n <= 255) && (octets[0] === 0 || octets[0] === 10 || octets[0] === 127 || (octets[0] === 172 && octets[1] >= 16 && octets[1] <= 31) || (octets[0] === 192 && octets[1] === 168) || (octets[0] === 169 && octets[1] === 254))) return undefined;
    if ([...url.searchParams.keys()].some(name => /(?:token|auth|secret|password|api[_-]?key)/i.test(name))) return undefined;
    return url.href;
  } catch { return undefined; }
}
function objectData(item) { return item && typeof item.data === 'object' ? item.data : item || {}; }
function citationKey(item) {
  const data = objectData(item);
  const fromExtra = text(data.extra).match(/^\s*(?:Citation Key|BibTeX Key)\s*:\s*(\S+)\s*$/im)?.[1];
  const candidate = fromExtra || data.citationKey || data.citekey || data['citation-key'] || item?.citationKey || item?.citekey;
  // Citation keys are inserted into [@key] and must not break that syntax.
  if (typeof candidate === 'string' && /^[A-Za-z0-9_][A-Za-z0-9_.:+/-]{0,199}$/.test(candidate)) return candidate;
  return 'zotero-' + keyOf(item?.key || data.key);
}
function itemLibrary(item) {
  if (item?.libraryPath) return libraryPathOf(item.libraryPath);
  if (item?.library?.type === 'group' && item.library.id) return libraryPathOf(`groups/${item.library.id}`);
  return libraryPathOf(`users/${item?.library?.id || 0}`);
}
function normalizeItem(item) {
  const data = objectData(item), key = keyOf(item?.key || data.key);
  const yearMatch = text(data.date).match(/\b([12][0-9]{3})\b/);
  const citekey = citationKey(item), serverID = item?.serverID || item?._serverID || '';
  const abstract = text(data.abstractNote), DOI = text(data.DOI);
  return {
    identity: `${serverID || 'local-unknown'}:${itemLibrary(item)}:${key}`, serverID, key,
    version: item?.version ?? data.version, title: text(data.title) || text(data.name) || '无标题',
    creators: Array.isArray(data.creators) ? data.creators.map(c => ({ ...c })) : [],
    year: yearMatch ? Number(yearMatch[1]) : null, DOI, doi: DOI, abstract, abstractNote: abstract,
    tags: Array.isArray(data.tags) ? data.tags.map(t => typeof t === 'string' ? t : text(t?.tag)).filter(Boolean) : [],
    collections: Array.isArray(data.collections) ? data.collections.filter(k => typeof k === 'string') : [],
    citekey, citationKey: citekey, itemType: text(data.itemType), url: text(data.url)
  };
}
const CSL_TYPES = {
  journalArticle: 'article-journal', conferencePaper: 'paper-conference', book: 'book', bookSection: 'chapter',
  thesis: 'thesis', report: 'report', webpage: 'webpage', preprint: 'article', manuscript: 'manuscript',
  newspaperArticle: 'article-newspaper', magazineArticle: 'article-magazine', patent: 'patent',
  computerProgram: 'software', dataset: 'dataset', presentation: 'speech', encyclopediaArticle: 'entry-encyclopedia'
};
function toPublicReference(item) {
  const data = objectData(item), normalized = normalizeItem(item);
  const reference = { id: normalized.citekey, citekey: normalized.citekey, title: normalized.title, type: CSL_TYPES[data.itemType] || 'article' };
  const authors = normalized.creators.filter(c => c.creatorType === 'author' || !c.creatorType).map(c => c.name
    ? { literal: text(c.name) }
    : { family: text(c.lastName), given: text(c.firstName) }).filter(c => c.literal || c.family || c.given);
  if (authors.length) reference.author = authors;
  if (normalized.year) reference.issued = { 'date-parts': [[normalized.year]] };
  const fields = { 'container-title': data.publicationTitle || data.bookTitle || data.proceedingsTitle || data.websiteTitle, DOI: data.DOI, volume: data.volume, issue: data.issue, page: data.pages, publisher: data.publisher };
  for (const [name, value] of Object.entries(fields)) if (text(value).trim()) reference[name] = text(value).trim();
  // File, zotero, data and javascript URLs must never appear in a public snapshot.
  const url = publicURL(data.url);
  if (url) reference.URL = url;
  return reference;
}
function openPDFURI(attachmentKey, page, annotationKey, libraryPath = 'users/0') {
  keyOf(attachmentKey); libraryPathOf(libraryPath);
  const library = libraryPath.startsWith('groups/') ? libraryPath : 'library';
  const params = new URLSearchParams();
  if (page != null && page !== '') {
    if (!Number.isSafeInteger(Number(page)) || Number(page) < 1) fail('INVALID_PAGE', 'PDF 页码必须是从 1 开始的整数。');
    params.set('page', String(Number(page)));
  }
  if (annotationKey) params.set('annotation', keyOf(annotationKey));
  return `zotero://open-pdf/${library}/items/${attachmentKey}${params.size ? '?' + params : ''}`;
}
function markdownText(value) { return text(value).replace(/[\\`*_{}\[\]<>#!|]/g, '\\$&'); }
function annotationPage(data) {
  let position = data.annotationPosition;
  if (typeof position === 'string') { try { position = JSON.parse(position); } catch { position = null; } }
  if (Number.isSafeInteger(position?.pageIndex) && position.pageIndex >= 0) return position.pageIndex + 1;
  if (/^[1-9][0-9]*$/.test(text(data.annotationPageLabel))) return Number(data.annotationPageLabel);
  return undefined;
}
function annotationsToMarkdown(item, annotations, { attachmentKey } = {}) {
  const reference = normalizeItem(item), libraryPath = itemLibrary(item);
  // JSON quoted strings are also valid TOML basic strings and prevent newline injection.
  const lines = ['+++', `title = ${JSON.stringify(reference.title + ' · 阅读笔记')}`, 'draft = true', '', '[extra]', `citation_key = ${JSON.stringify(reference.citekey)}`, `zotero_key = ${JSON.stringify(reference.key)}`, '+++', '', `# ${markdownText(reference.title)}`, '', `文献引用：[@${reference.citekey}]`, ''];
  const sorted = Array.isArray(annotations) ? annotations.slice().sort((a, b) => text(objectData(a).annotationSortIndex).localeCompare(text(objectData(b).annotationSortIndex))) : [];
  for (const annotation of sorted) {
    const data = objectData(annotation);
    if (data.itemType !== 'annotation') continue;
    const attachment = attachmentKey || data.parentItem;
    if (!attachment) continue;
    const annotationKey = annotation.key || data.key, page = annotationPage(data);
    const label = text(data.annotationPageLabel) || (page ? String(page) : '未知');
    const uri = openPDFURI(attachment, page, annotationKey, libraryPath);
    lines.push(`## [第 ${markdownText(label)} 页](${uri})`, '');
    if (text(data.annotationText)) lines.push(...markdownText(data.annotationText).split(/\r?\n/).map(line => '> ' + line), '');
    if (text(data.annotationComment)) lines.push(markdownText(data.annotationComment), '');
    if (!data.annotationText && !data.annotationComment) lines.push(`批注类型：${markdownText(data.annotationType) || '批注'}，可点击页码在 Zotero 中查看。`, '');
  }
  return lines.join('\n').trimEnd() + '\n';
}

class ZoteroClient {
  constructor({ baseUrl = 'http://127.0.0.1:23119/api/', apiKey = '', libraryPath = 'users/0', fetchImpl = globalThis.fetch, timeoutMs = 15000 } = {}) {
    this.baseUrl = localURL(baseUrl).href; this.libraryPath = libraryPathOf(libraryPath);
    if (typeof fetchImpl !== 'function') fail('NO_FETCH', '运行环境不支持 HTTP 请求。');
    this.fetchImpl = fetchImpl; this.apiKey = typeof apiKey === 'string' ? apiKey : '';
    this.timeoutMs = Math.max(100, Math.min(120000, Number(timeoutMs) || 15000));
    this.serverID = null; this.apiVersion = null; this.clientVersion = null; this.probed = false; this.writeSupported = false;
    this.authorizationStatus = this.apiKey ? 'unverified' : 'none'; this.remember = null;
  }
  _url(endpoint) {
    let url;
    try { url = new URL(endpoint, this.baseUrl); } catch { fail('UNSAFE_URL', 'Zotero 返回了无效的请求地址。'); }
    const base = new URL(this.baseUrl);
    if (url.origin !== base.origin || !url.pathname.startsWith(base.pathname) || url.username || url.password || url.hash || url.searchParams.has('key')) fail('UNSAFE_URL', '拒绝将 Zotero 请求转发到其他地址。');
    return url;
  }
  async _request(endpoint, { method = 'GET', headers = {}, body, discovery = false, authorize = false, upload = false, duplex, timeoutMs = this.timeoutMs } = {}) {
    const url = this._url(endpoint), mutation = method !== 'GET' && method !== 'HEAD';
    if (mutation && !upload) {
      if (!this.writeSupported || !this.serverID) fail('READ_ONLY', '当前 Zotero 仅支持读取。请使用 Zotero 10+，或在 Zotero 中管理文献。');
      if (!authorize && !this.apiKey) fail('AUTH_REQUIRED', '请先在 Zotero 中授权 Research Blog 修改文献库。');
    }
    const requestHeaders = { Accept: 'application/json', ...headers };
    if (!discovery && !upload) requestHeaders['Zotero-API-Version'] = String(this.apiVersion || 3);
    if (this.serverID && !discovery && !upload) requestHeaders['Zotero-Server-ID'] = this.serverID;
    if (mutation && !authorize && !upload) requestHeaders['Zotero-API-Key'] = this.apiKey;
    const controller = new AbortController(), timer = setTimeout(() => controller.abort(), timeoutMs);
    let response, raw;
    try {
      response = await this.fetchImpl(url.href, { method, headers: requestHeaders, body, signal: controller.signal, redirect: 'manual', ...(duplex ? { duplex } : {}) });
      raw = await response.text();
    } catch (cause) {
      fail(controller.signal.aborted ? 'TIMEOUT' : 'CONNECTION_FAILED', controller.signal.aborted ? 'Zotero 本地请求超时；写操作可能已经完成，请刷新后检查。' : '无法连接 Zotero。请启动 Zotero，并在高级设置中允许本机应用通信。', { cause, uncertainWrite: mutation });
    } finally { clearTimeout(timer); }
    const serverID = response.headers.get('Zotero-Server-ID');
    if (!discovery && !upload && this.serverID && serverID !== this.serverID) {
      this.apiKey = ''; this.authorizationStatus = 'none'; this.probed = false;
      fail('SERVER_CHANGED', 'Zotero 文献数据库已切换，请重新连接并刷新文献后再操作。', { status: response.status, serverID });
    }
    let data = raw;
    if (raw.trim()) { try { data = JSON.parse(raw); } catch {} } else data = null;
    if (response.status >= 300 && response.status < 400) fail('UNSAFE_REDIRECT', 'Zotero 请求发生重定向，已停止以保护本机文献和密钥。', { status: response.status });
    if (!response.ok) {
      if (response.status === 401) { this.apiKey = ''; this.authorizationStatus = 'none'; }
      const code = response.status === 401 ? 'AUTH_REQUIRED' : response.status === 403 ? (authorize && data?.denied ? 'AUTH_DENIED' : 'FORBIDDEN') : response.status === 412 ? 'CONFLICT' : response.status === 409 ? 'LIBRARY_LOCKED' : response.status === 429 ? 'RATE_LIMITED' : response.status === 428 ? 'PRECONDITION_REQUIRED' : response.status === 404 ? 'NOT_FOUND' : response.status === 405 || response.status === 501 ? 'UNSUPPORTED' : 'HTTP_ERROR';
      const messages = { AUTH_REQUIRED: 'Zotero 写入授权已失效或是单次授权。请重新授权并检查当前文献状态。', AUTH_DENIED: '你拒绝了 Zotero 写入授权，可继续读取文献。', FORBIDDEN: 'Zotero 拒绝了请求。请检查高级设置的本机通信开关及该文献库的权限。', CONFLICT: '文献、分类或附件已被修改，请刷新后重新操作。', LIBRARY_LOCKED: 'Zotero 文献库暂时锁定，请稍后重新操作。', RATE_LIMITED: 'Zotero 授权或上传请求过于频繁，请稍后重新操作。', PRECONDITION_REQUIRED: 'Zotero 要求数据库身份或版本条件，请重新连接。', UNSUPPORTED: '当前 Zotero 不支持此操作，请在 Zotero 中管理文献。', NOT_FOUND: '文献或 Zotero API 接口不存在，请刷新或更新 Zotero。' };
      fail(code, messages[code] || `Zotero 请求失败（HTTP ${response.status}）。`, { status: response.status, details: typeof data === 'object' ? data : undefined, retryAfter: response.headers.get('Retry-After') });
    }
    if (discovery) {
      if (this.serverID && serverID !== this.serverID) { this.apiKey = ''; this.authorizationStatus = 'none'; this.remember = null; }
      this.serverID = serverID; this.apiVersion = Number(response.headers.get('Zotero-API-Version') || 3);
      this.clientVersion = response.headers.get('X-Zotero-Version');
    }
    if (mutation && !authorize && !upload && this.remember === false) { this.apiKey = ''; this.authorizationStatus = 'none'; }
    return { data, response };
  }
  async probe() {
    await this._request('', { discovery: true });
    if (this.apiVersion !== 3) fail('API_VERSION', `当前 Zotero 使用 API v${this.apiVersion}，本适配只支持 v3。`);
    this.probed = true; this.writeSupported = !!this.serverID;
    return { read: true, canRead: true, writeSupported: this.writeSupported, canWrite: this.writeSupported && !!this.apiKey, writeAuthorized: !!this.apiKey, authorizationStatus: this.authorizationStatus, apiVersion: this.apiVersion, version: this.clientVersion || (this.writeSupported ? '10+' : 'unknown (read only)'), serverID: this.serverID };
  }
  async _ensureProbe() { if (!this.probed) await this.probe(); }
  _stamp(item) {
    if (!item || typeof item !== 'object' || Array.isArray(item)) fail('INVALID_RESPONSE', 'Zotero 返回了无效的文献对象。');
    keyOf(item.key || item.data?.key);
    Object.defineProperties(item, { serverID: { value: this.serverID, enumerable: false, configurable: true }, libraryPath: { value: this.libraryPath, enumerable: false, configurable: true } });
    return item;
  }
  async authorizeWrites(appName = 'Research Blog') {
    await this._ensureProbe();
    if (typeof appName !== 'string' || !appName.trim() || appName.length > 200 || /[\x00-\x1f]/.test(appName)) fail('INVALID_APP_NAME', '授权应用名称无效。');
    const { data } = await this._request('local/authorize', { method: 'POST', authorize: true, timeoutMs: Math.max(120000, this.timeoutMs), headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ appName: appName.trim() }) });
    if (!data || typeof data.key !== 'string' || !/^[a-zA-Z0-9]{32}$/.test(data.key) || typeof data.remember !== 'boolean') fail('INVALID_RESPONSE', 'Zotero 返回了无效的本地授权密钥。');
    this.apiKey = data.key; this.remember = data.remember; this.authorizationStatus = 'granted';
    return { key: data.key, remember: data.remember };
  }
  async _list(endpoint, params = {}) {
    await this._ensureProbe();
    let url = this._url(endpoint);
    url.searchParams.set('format', 'json'); url.searchParams.set('limit', '100'); url.searchParams.set('start', '0');
    for (const [name, value] of Object.entries(params)) if (value != null && value !== '') url.searchParams.set(name, String(value));
    const result = [], visited = new Set(); let libraryVersion;
    while (url) {
      if (visited.has(url.href)) fail('INVALID_PAGINATION', 'Zotero 返回了循环分页地址。');
      visited.add(url.href);
      const { data, response } = await this._request(url.href);
      if (!Array.isArray(data)) fail('INVALID_RESPONSE', 'Zotero 返回的文献列表格式无效。');
      const pageVersion = response.headers.get('Last-Modified-Version');
      if (libraryVersion != null && pageVersion != null && pageVersion !== libraryVersion) fail('CONFLICT', '读取分页期间 Zotero 文献库发生变化，请刷新后重试。');
      if (pageVersion != null) libraryVersion = pageVersion;
      result.push(...data.map(item => this._stamp(item)));
      const next = /<([^>]+)>\s*;\s*rel\s*=\s*"?next"?/i.exec(response.headers.get('Link') || '')?.[1];
      if (next) { url = this._url(new URL(next, url).href); }
      else {
        const total = Number(response.headers.get('Total-Results'));
        const start = Number(url.searchParams.get('start') || 0);
        if (response.headers.has('Total-Results') && Number.isSafeInteger(total) && start + data.length < total) {
          if (!data.length) fail('INVALID_PAGINATION', 'Zotero 分页在读取全部文献前停止。');
          url = new URL(url); url.searchParams.set('start', String(start + data.length));
        } else url = null;
      }
    }
    return result;
  }
  listCollections() { return this._list(`${this.libraryPath}/collections`); }
  listItems({ collectionKey, q, tag } = {}) {
    const endpoint = collectionKey ? `${this.libraryPath}/collections/${keyOf(collectionKey)}/items/top` : `${this.libraryPath}/items/top`;
    return this._list(endpoint, { q, tag, sort: 'title', direction: 'asc' });
  }
  async children(key) {
    keyOf(key);
    const children = await this._list(`${this.libraryPath}/items/${key}/children`);
    // Zotero 10.0.6's includeChildren search omits PDF annotations, even though
    // the annotation objects exist. Fetch annotations with the documented item
    // type filter and select their parent locally so note export is complete.
    const parent = await this.getItem(key);
    if (parent.data?.itemType !== 'attachment') return children;
    const annotations = await this._list(`${this.libraryPath}/items`, { itemType: 'annotation' });
    const all = new Map(children.map(child => [child.key, child]));
    for (const annotation of annotations) if (annotation.data?.parentItem === key) all.set(annotation.key, annotation);
    return [...all.values()];
  }
  async getItem(key) {
    await this._ensureProbe();
    return this._stamp((await this._request(`${this.libraryPath}/items/${keyOf(key)}?format=json`)).data);
  }
  _versioned(object) {
    const key = keyOf(object?.key || object?.data?.key), version = object?.version ?? object?.data?.version;
    if (!Number.isSafeInteger(version) || version < 0) fail('VERSION_REQUIRED', '请先刷新条目或分类，获取当前本地版本。');
    if (!object.serverID || object.serverID !== this.serverID || object.libraryPath !== this.libraryPath) fail('SERVER_CHANGED', '对象缺少当前 Zotero 数据库身份，请刷新后再修改。');
    return { key, version };
  }
  async _create(type, objects) {
    await this._ensureProbe();
    const createData = objects.map(object => {
      if (!object.key) return object;
      keyOf(object.key);
      if (object.version != null && object.version !== 0) fail('INVALID_ITEM', '创建新对象时指定的版本必须为 0。');
      return { ...object, version: 0 };
    });
    // A predetermined key must explicitly use version 0 (new object), or real
    // Zotero responds 428. Versioned writes do not need a duplicate write token.
    const headers = { 'Content-Type': 'application/json' };
    if (!createData.every(object => object.key)) headers['Zotero-Write-Token'] = randomBytes(16).toString('hex');
    const { data } = await this._request(`${this.libraryPath}/${type}`, { method: 'POST', headers, body: JSON.stringify(createData) });
    if (!data || typeof data !== 'object' || !data.successful || !data.failed) fail('INVALID_RESPONSE', 'Zotero 创建响应格式无效；请刷新确认是否已创建。', { uncertainWrite: true });
    const results = [];
    for (let index = 0; index < objects.length; index++) {
      const failed = data.failed[index];
      if (failed) fail(failed.code === 412 ? 'CONFLICT' : 'WRITE_FAILED', 'Zotero 未能保存条目或分类，请刷新后检查。', { status: failed.code, details: failed, successful: Object.values(data.successful).map(item => this._stamp(item)) });
      if (!data.successful[index]) fail('INVALID_RESPONSE', 'Zotero 未返回新对象；请刷新确认。', { uncertainWrite: true });
      results.push(this._stamp(data.successful[index]));
    }
    return results;
  }
  async createItem(data) {
    if (!data || typeof data !== 'object' || Array.isArray(data) || !text(data.itemType)) fail('INVALID_ITEM', '新文献需要 itemType。');
    return (await this._create('items', [data]))[0];
  }
  async updateItem(item, patch) {
    await this._ensureProbe(); const { key, version } = this._versioned(item);
    if (!patch || typeof patch !== 'object' || Array.isArray(patch) || ['key', 'version'].some(field => field in patch)) fail('INVALID_PATCH', '修改内容不能覆盖 Zotero 标识或版本。');
    await this._request(`${this.libraryPath}/items/${key}`, { method: 'PATCH', headers: { 'Content-Type': 'application/json', 'If-Unmodified-Since-Version': String(version) }, body: JSON.stringify(patch) });
    return this.getItem(key);
  }
  async createCollection(name, parentKey = null) {
    if (typeof name !== 'string' || !name.trim() || name.length > 500 || /[\x00-\x1f]/.test(name)) fail('INVALID_NAME', '请输入有效的分类名称。');
    return (await this._create('collections', [{ name: name.trim(), parentCollection: parentKey ? keyOf(parentKey) : false }]))[0];
  }
  async updateCollection(collection, patch) {
    await this._ensureProbe(); const { key, version } = this._versioned(collection);
    if (!patch || typeof patch !== 'object' || Array.isArray(patch) || Object.keys(patch).some(field => !['name', 'parentCollection'].includes(field))) fail('INVALID_PATCH', '分类仅可修改名称和父分类。');
    if ('name' in patch && (typeof patch.name !== 'string' || !patch.name.trim() || patch.name.length > 500 || /[\x00-\x1f]/.test(patch.name))) fail('INVALID_NAME', '请输入有效的分类名称。');
    if (patch.parentCollection) keyOf(patch.parentCollection);
    await this._request(`${this.libraryPath}/collections/${key}`, { method: 'PUT', headers: { 'Content-Type': 'application/json', 'If-Unmodified-Since-Version': String(version) }, body: JSON.stringify({ ...objectData(collection), ...patch, key, version }) });
    return this._stamp((await this._request(`${this.libraryPath}/collections/${key}?format=json`)).data);
  }
  openPDFURI(attachmentKey, page, annotationKey) { return openPDFURI(attachmentKey, page, annotationKey, this.libraryPath); }
  async importPDF(filePath, { title, collectionKey } = {}) {
    await this._ensureProbe();
    if (!this.writeSupported || !this.apiKey) fail(this.writeSupported ? 'AUTH_REQUIRED' : 'READ_ONLY', '导入 PDF 需要 Zotero 10+ 的写入授权。');
    if (this.remember === false) fail('PERSISTENT_AUTH_REQUIRED', 'PDF 导入包含多个写入步骤，请重新授权并在 Zotero 中选择 Always Allow，然后导入。');
    if (collectionKey) keyOf(collectionKey);
    const absolutePath = path.resolve(filePath), stat = await fs.promises.stat(absolutePath);
    if (!stat.isFile() || stat.size < 5 || stat.size >= 4 * 1024 ** 3) fail('INVALID_PDF', 'PDF 必须是小于 4 GB 的本地文件。');
    const file = await fs.promises.open(absolutePath, 'r');
    try { const signature = Buffer.alloc(5); await file.read(signature, 0, 5, 0); if (signature.toString() !== '%PDF-') fail('INVALID_PDF', '所选文件不是有效的 PDF。'); } finally { await file.close(); }
    const hash = createHash('md5'); for await (const chunk of fs.createReadStream(absolutePath)) hash.update(chunk);
    const md5 = hash.digest('hex'), filename = path.basename(absolutePath), itemTitle = text(title).trim() || path.basename(filename, path.extname(filename));
    const alphabet = '23456789ABCDEFGHIJKLMNPQRSTUVWXYZ';
    const randomKey = () => [...randomBytes(8)].map(byte => alphabet[byte % alphabet.length]).join('');
    const parentKey = randomKey(), attachmentKey = randomKey();
    let item, attachment;
    try {
      [item, attachment] = await this._create('items', [
        { key: parentKey, itemType: 'journalArticle', title: itemTitle, creators: [], tags: [], collections: collectionKey ? [collectionKey] : [], relations: {} },
        { key: attachmentKey, itemType: 'attachment', parentItem: parentKey, linkMode: 'imported_file', title: filename, contentType: 'application/pdf', filename, tags: [], relations: {} }
      ]);
      const endpoint = `${this.libraryPath}/items/${attachment.key}/file`, headers = { 'Content-Type': 'application/x-www-form-urlencoded', 'If-None-Match': '*' };
      const parameters = new URLSearchParams({ md5, filename, filesize: String(stat.size), mtime: String(Math.floor(stat.mtimeMs)) });
      const { data: upload } = await this._request(endpoint, { method: 'POST', headers, body: parameters.toString() });
      if (!upload?.exists) {
        if (!upload || typeof upload.url !== 'string' || typeof upload.uploadKey !== 'string' || !/^[A-Za-z0-9_-]+$/.test(upload.uploadKey) || upload.prefix !== '' || upload.suffix !== '') fail('INVALID_UPLOAD', 'Zotero 返回了不支持的本地上传格式。');
        const uploadURL = this._url(upload.url);
        if (uploadURL.pathname !== `/api/local/uploads/${upload.uploadKey}` || uploadURL.search) fail('UNSAFE_URL', '拒绝将 PDF 上传至 Zotero 本地上传接口以外的地址。');
        const stream = fs.createReadStream(absolutePath);
        try {
          const { response } = await this._request(uploadURL.href, { method: 'POST', upload: true, headers: { 'Content-Type': upload.contentType || 'application/octet-stream', 'Content-Length': String(stat.size) }, body: Readable.toWeb(stream), duplex: 'half' });
          if (response.status !== 201) fail('INVALID_UPLOAD', 'Zotero 未确认收到 PDF 文件。');
        } finally { stream.destroy(); }
        await this._request(endpoint, { method: 'POST', headers, body: new URLSearchParams({ upload: upload.uploadKey }).toString() });
      }
      return { item: await this.getItem(item.key), attachment: await this.getItem(attachment.key) };
    } catch (error) {
      if (!item && Array.isArray(error.successful)) {
        item = error.successful.find(saved => saved.key === parentKey);
        attachment = error.successful.find(saved => saved.key === attachmentKey);
      }
      if (item) error.partialItem = item;
      if (attachment) error.partialAttachment = attachment;
      if (item) error.message += ' 文献已创建；请刷新后在 Zotero 检查附件，避免重复导入。';
      throw error;
    }
  }
}

module.exports = { ZoteroClient, ZoteroError, normalizeItem, toPublicReference, citationKey, annotationsToMarkdown, openPDFURI };
