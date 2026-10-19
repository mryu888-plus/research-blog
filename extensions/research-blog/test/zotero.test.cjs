'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const http = require('node:http');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHash } = require('node:crypto');
const toml = require('smol-toml');
const { ZoteroClient, normalizeItem, toPublicReference, annotationsToMarkdown, openPDFURI } = require('../src/zotero-core.cjs');

const KEY = 'ABCD2345', ATTACHMENT = 'BCDE3456', COLLECTION = 'CDEF4567', ANNOTATION = 'DEFG5678';
const API_KEY = 'a'.repeat(32);
function wrapper(key = KEY, data = {}, version = 7) {
  return { key, version, library: { type: 'user', id: 42, name: 'Private Library' }, links: { self: { href: 'private' } }, meta: { creatorSummary: 'Secret' }, data: { key, version, itemType: 'journalArticle', title: 'An Example', creators: [], tags: [], collections: [], relations: {}, ...data } };
}
async function mock(t, route, { modern = true } = {}) {
  const requests = [];
  const server = http.createServer(async (req, res) => {
    const chunks = []; for await (const chunk of req) chunks.push(chunk);
    const body = Buffer.concat(chunks);
    requests.push({ method: req.method, url: req.url, headers: req.headers, body });
    res.setHeader('Zotero-API-Version', '3');
    if (modern) res.setHeader('Zotero-Server-ID', 'database-A');
    const reply = (status, data, headers = {}) => {
      res.writeHead(status, { 'Content-Type': 'application/json', ...headers });
      res.end(data == null ? undefined : JSON.stringify(data));
    };
    try {
      if (req.url === '/api/' && req.method === 'GET') return reply(200, { title: 'Zotero Local API' });
      await route({ req, res, body, reply, requests });
    } catch (error) { reply(500, { message: error.message }); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  const baseUrl = `http://127.0.0.1:${server.address().port}/api/`;
  return { baseUrl, requests, client: new ZoteroClient({ baseUrl }) };
}

test('connection configuration cannot target remote servers or carry keys in URLs', () => {
  for (const baseUrl of ['https://127.0.0.1:23119/api/', 'http://example.org:23119/api/', 'http://127.0.0.1.attacker.test/api/', 'http://u:p@localhost:23119/api/', 'http://localhost:23119/api/?key=secret', 'file:///api/']) {
    assert.throws(() => new ZoteroClient({ baseUrl }), { code: 'UNSAFE_URL' });
  }
  assert.throws(() => new ZoteroClient({ libraryPath: '../items' }), { code: 'INVALID_LIBRARY' });
  assert.throws(() => new ZoteroClient({ baseUrl: 'http://localhost:23119/wrong' }), { code: 'INVALID_URL' });
  assert.doesNotThrow(() => new ZoteroClient({ baseUrl: 'http://[::1]:23119/api/' }));
});

test('reads preserve wrapper metadata, search parameters and all Link pagination pages without sending keys', async t => {
  const setup = await mock(t, ({ req, reply }) => {
    const url = new URL(req.url, 'http://localhost');
    assert.equal(req.headers['zotero-server-id'], 'database-A');
    assert.equal(req.headers['zotero-api-version'], '3');
    assert.equal(req.headers['zotero-api-key'], undefined);
    if (url.pathname.endsWith('/collections')) return reply(200, [wrapper(COLLECTION, { name: 'Research' })]);
    if (url.pathname.endsWith('/items/top')) {
      assert.equal(url.searchParams.get('q'), 'a & b'); assert.equal(url.searchParams.get('tag'), '私有 标签');
      if (url.searchParams.get('start') === '0') return reply(200, [wrapper()], { 'Total-Results': '2', Link: `</api/users/0/collections/${COLLECTION}/items/top?start=1&limit=100&q=a+%26+b&tag=${encodeURIComponent('私有 标签')}>; rel="next"` });
      return reply(200, [wrapper(ATTACHMENT, { title: 'Next page' })], { 'Total-Results': '2' });
    }
    if (url.pathname.endsWith('/children')) return reply(200, [wrapper(ATTACHMENT, { itemType: 'attachment', contentType: 'application/pdf', parentItem: KEY })]);
    return reply(200, wrapper());
  });
  setup.client.apiKey = API_KEY;
  const capabilities = await setup.client.probe();
  assert.equal(capabilities.writeSupported, true); assert.equal(capabilities.serverID, 'database-A');
  const items = await setup.client.listItems({ collectionKey: COLLECTION, q: 'a & b', tag: '私有 标签' });
  assert.equal(items.length, 2); assert.equal(items[0].data.title, 'An Example');
  assert.equal(items[0].serverID, 'database-A'); assert.equal(items[0].libraryPath, 'users/0');
  assert.equal(Object.keys(items[0]).includes('serverID'), false);
  assert.equal(normalizeItem(items[0]).identity, `database-A:users/0:${KEY}`);
  assert.equal((await setup.client.listCollections())[0].data.name, 'Research');
  assert.equal((await setup.client.children(KEY))[0].data.parentItem, KEY);
  assert.equal((await setup.client.getItem(KEY)).key, KEY);
});

test('pagination falls back to Total-Results and rejects remote Link URLs before forwarding any request', async t => {
  let external = false;
  const setup = await mock(t, ({ req, reply }) => {
    const url = new URL(req.url, 'http://localhost');
    if (external) return reply(200, [wrapper()], { Link: '<http://example.test/api/users/0/items?start=1>; rel="next"' });
    return reply(200, [wrapper(url.searchParams.get('start') === '0' ? KEY : ATTACHMENT)], { 'Total-Results': '2' });
  });
  assert.equal((await setup.client.listItems()).length, 2);
  external = true;
  await assert.rejects(setup.client.listItems(), { code: 'UNSAFE_URL' });
  assert.equal(setup.requests.length, 4);
});

test('pagination detects a library change instead of returning an inconsistent snapshot', async t => {
  const setup = await mock(t, ({ req, reply }) => {
    const start = new URL(req.url, 'http://localhost').searchParams.get('start');
    return reply(200, [wrapper(start === '0' ? KEY : ATTACHMENT)], { 'Total-Results': '2', 'Last-Modified-Version': start === '0' ? '10' : '11' });
  });
  await assert.rejects(setup.client.listItems(), { code: 'CONFLICT' });
});

test('attachment children recover annotations omitted by Zotero 10.0.6 without mixing another PDF annotations', async t => {
  const setup = await mock(t, ({ req, reply }) => {
    const url = new URL(req.url, 'http://localhost');
    if (url.pathname.endsWith('/children')) return reply(200, []);
    if (url.searchParams.get('itemType') === 'annotation') return reply(200, [wrapper(ANNOTATION, { itemType: 'annotation', parentItem: ATTACHMENT }), wrapper(COLLECTION, { itemType: 'annotation', parentItem: KEY })]);
    reply(200, wrapper(ATTACHMENT, { itemType: 'attachment', contentType: 'application/pdf', parentItem: KEY }));
  });
  const children = await setup.client.children(ATTACHMENT);
  assert.deepEqual(children.map(item => item.key), [ANNOTATION]);
});

test('write authorization sends the database ID and returns the Zotero local key; denied and old clients stay read only', async t => {
  let deny = false;
  const setup = await mock(t, ({ req, body, reply }) => {
    assert.equal(req.url, '/api/local/authorize'); assert.equal(req.headers['zotero-server-id'], 'database-A');
    assert.equal(req.headers['zotero-api-key'], undefined); assert.equal(JSON.parse(body).appName, 'Research Blog');
    reply(deny ? 403 : 200, deny ? { denied: true } : { key: API_KEY, remember: true });
  });
  assert.deepEqual(await setup.client.authorizeWrites('Research Blog'), { key: API_KEY, remember: true });
  assert.equal(setup.client.apiKey, API_KEY); assert.equal((await setup.client.probe()).authorizationStatus, 'granted');
  deny = true; await assert.rejects(setup.client.authorizeWrites('Research Blog'), { code: 'AUTH_DENIED' });
  const old = await mock(t, ({ reply }) => reply(500, null), { modern: false });
  assert.equal((await old.client.probe()).writeSupported, false);
  await assert.rejects(old.client.authorizeWrites(), { code: 'READ_ONLY' });
  await assert.rejects(old.client.createItem({ itemType: 'book' }), { code: 'READ_ONLY' });
  assert.equal(old.requests.length, 1);
});

test('item and collection edits use local object versions; conflicts and rejected keys never retry mutations', async t => {
  let status = 204, mutationCount = 0;
  const setup = await mock(t, ({ req, body, reply }) => {
    if (req.method === 'GET') return reply(200, req.url.startsWith('/api/users/0/collections?') ? [wrapper(COLLECTION, { name: 'A collection' })] : wrapper(req.url.includes('/collections/') ? COLLECTION : KEY, { name: 'A collection' }));
    mutationCount++;
    assert.equal(req.headers['zotero-api-key'], API_KEY); assert.equal(req.headers['zotero-server-id'], 'database-A');
    if (req.method === 'PATCH') {
      assert.equal(req.headers['if-unmodified-since-version'], '7');
      assert.deepEqual(JSON.parse(body), { title: 'Revised' });
    } else if (req.method === 'PUT') {
      assert.equal(req.headers['if-unmodified-since-version'], '7'); assert.equal(JSON.parse(body).version, 7);
    } else {
      assert.match(req.headers['zotero-write-token'], /^[0-9a-f]{32}$/);
      const data = JSON.parse(body)[0];
      return reply(200, { successful: { 0: wrapper(data.itemType ? KEY : COLLECTION, data, 8) }, failed: {}, unchanged: {} });
    }
    reply(status, status === 204 ? null : { message: 'server rejection' });
  });
  setup.client.apiKey = API_KEY;
  const item = await setup.client.getItem(KEY), collection = (await setup.client.listCollections())[0];
  await setup.client.updateItem(item, { title: 'Revised' });
  await setup.client.updateCollection(collection, { name: 'Renamed' });
  assert.equal((await setup.client.createItem({ itemType: 'book', title: 'Created' })).data.title, 'Created');
  assert.equal((await setup.client.createCollection('Child', COLLECTION)).data.parentCollection, COLLECTION);
  status = 412; await assert.rejects(setup.client.updateItem(item, { title: 'Revised' }), { code: 'CONFLICT' });
  const before = mutationCount; status = 401;
  await assert.rejects(setup.client.updateItem(item, { title: 'Revised' }), { code: 'AUTH_REQUIRED' });
  assert.equal(mutationCount, before + 1); assert.equal(setup.client.apiKey, '');
  await assert.rejects(setup.client.updateItem(item, { title: 'Revised' }), { code: 'AUTH_REQUIRED' });
  assert.equal(mutationCount, before + 1);
});

test('server switches invalidate local versions and authorization, including cached wrappers and unbound JSON clones', async t => {
  let changed = false;
  const setup = await mock(t, ({ res, reply }) => {
    if (changed) res.setHeader('Zotero-Server-ID', 'database-B');
    reply(200, wrapper());
  });
  setup.client.apiKey = API_KEY;
  const item = await setup.client.getItem(KEY);
  await assert.rejects(setup.client.updateItem({ ...item }, { title: 'Changed' }), { code: 'SERVER_CHANGED' });
  changed = true;
  await assert.rejects(setup.client.getItem(KEY), { code: 'SERVER_CHANGED' });
  assert.equal(setup.client.apiKey, '');
  setup.client.serverID = 'database-B'; setup.client.probed = true;
  await assert.rejects(setup.client.updateItem(item, { title: 'Changed' }), { code: 'SERVER_CHANGED' });
});

test('a single-use key is consumed after a validated successful write', async t => {
  const setup = await mock(t, ({ req, reply }) => {
    if (req.url === '/api/local/authorize') return reply(200, { key: API_KEY, remember: false });
    return reply(200, { successful: { 0: wrapper() }, failed: {}, unchanged: {} });
  });
  await setup.client.authorizeWrites(); await setup.client.createItem({ itemType: 'book' });
  assert.equal(setup.client.apiKey, '');
  await assert.rejects(setup.client.createItem({ itemType: 'book' }), { code: 'AUTH_REQUIRED' });
  assert.equal(setup.requests.filter(r => r.url === '/api/users/0/items').length, 1);
});

test('single-use authorization refuses a multi-step PDF import before creating a partial reference', async t => {
  const setup = await mock(t, ({ reply }) => reply(200, { key: API_KEY, remember: false }));
  await setup.client.authorizeWrites();
  await assert.rejects(setup.client.importPDF('does-not-exist.pdf'), { code: 'PERSISTENT_AUTH_REQUIRED' });
  assert.equal(setup.requests.length, 2);
});

test('PDF import uses atomic parent/child creation and the authenticated three-phase local upload protocol', async t => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'zotero-local-test-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const filePath = path.join(dir, '论文 & notes.pdf'), bytes = Buffer.from('%PDF-1.7\nmock PDF\n%%EOF');
  fs.writeFileSync(filePath, bytes); const saved = new Map(); let phase = 0;
  const setup = await mock(t, ({ req, body, reply }) => {
    if (req.method === 'GET') { const key = req.url.split('/').at(-1).split('?')[0]; return reply(200, saved.get(key)); }
    if (req.url === '/api/users/0/items') {
      const data = JSON.parse(body); assert.equal(data.length, 2); assert.equal(data[0].itemType, 'journalArticle');
      assert.equal(data[0].version, 0); assert.equal(data[1].version, 0); assert.equal(req.headers['zotero-write-token'], undefined);
      assert.equal(data[1].parentItem, data[0].key); assert.equal(data[1].linkMode, 'imported_file'); assert.deepEqual(data[0].collections, [COLLECTION]);
      const successful = {}; data.forEach((item, index) => { successful[index] = wrapper(item.key, item, 10); saved.set(item.key, successful[index]); });
      return reply(200, { successful, unchanged: {}, failed: {} });
    }
    if (req.url === '/api/local/uploads/upload-123') {
      assert.equal(req.headers['zotero-api-key'], undefined); assert.equal(req.headers['zotero-server-id'], undefined);
      assert.deepEqual(body, bytes); phase++; return reply(201, null);
    }
    assert.equal(req.headers['zotero-api-key'], API_KEY); assert.equal(req.headers['zotero-server-id'], 'database-A'); assert.equal(req.headers['if-none-match'], '*');
    const form = new URLSearchParams(body.toString());
    if (form.has('md5')) {
      assert.equal(form.get('filename'), path.basename(filePath)); assert.equal(form.get('md5'), createHash('md5').update(bytes).digest('hex'));
      assert.equal(Number(form.get('filesize')), bytes.length); assert.ok(Number(form.get('mtime')) > 1e12); phase++;
      return reply(200, { url: setup.baseUrl + 'local/uploads/upload-123', uploadKey: 'upload-123', contentType: 'application/octet-stream', prefix: '', suffix: '' });
    }
    assert.equal(form.get('upload'), 'upload-123'); phase++; return reply(204, null, { 'Last-Modified-Version': '11' });
  });
  setup.client.apiKey = API_KEY;
  const result = await setup.client.importPDF(filePath, { title: 'Imported research', collectionKey: COLLECTION });
  assert.equal(phase, 3); assert.equal(result.item.data.title, 'Imported research'); assert.equal(result.attachment.data.parentItem, result.item.key);
  assert.equal(result.attachment.data.filename, path.basename(filePath));
});

test('PDF upload refuses remote destinations and leaves a recoverable partial item without retrying creation', async t => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'zotero-upload-test-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const filePath = path.join(dir, 'paper.pdf'); fs.writeFileSync(filePath, '%PDF-1.7\n%%EOF');
  const setup = await mock(t, ({ req, body, reply }) => {
    if (req.url === '/api/users/0/items') { const data = JSON.parse(body); return reply(200, { successful: { 0: wrapper(data[0].key, data[0]), 1: wrapper(data[1].key, data[1]) }, failed: {}, unchanged: {} }); }
    reply(200, { url: 'http://example.test/api/local/uploads/key', uploadKey: 'key', contentType: 'application/pdf', prefix: '', suffix: '' });
  });
  setup.client.apiKey = API_KEY;
  await assert.rejects(setup.client.importPDF(filePath), error => error.code === 'UNSAFE_URL' && !!error.partialItem && !!error.partialAttachment);
  assert.equal(setup.requests.filter(r => r.url === '/api/users/0/items').length, 1);
  fs.writeFileSync(filePath, 'not a PDF');
  const count = setup.requests.length;
  await assert.rejects(setup.client.importPDF(filePath), { code: 'INVALID_PDF' }); assert.equal(setup.requests.length, count);
});

test('public references export only bibliographic fields and stable citekeys, never annotations, paths or private metadata', () => {
  const item = wrapper(KEY, {
    title: 'Useful Research', date: '2024-08-12', extra: 'Private note\nCitation Key: smith2024',
    creators: [{ creatorType: 'author', firstName: 'Alex', lastName: 'Smith' }, { creatorType: 'author', name: 'Institute' }, { creatorType: 'editor', firstName: 'Private', lastName: 'Editor' }],
    publicationTitle: 'Journal', DOI: '10.123/example', abstractNote: 'Secret abstract', tags: [{ tag: 'private-project' }],
    collections: [COLLECTION], url: 'https://example.org/paper', filename: 'private.pdf', path: 'C:\\private.pdf', annotationComment: 'Private', note: '<p>Secret</p>', volume: '1', issue: '2', pages: '3–9', publisher: 'Publisher'
  });
  const reference = toPublicReference(item);
  assert.deepEqual(reference, { id: 'smith2024', citekey: 'smith2024', title: 'Useful Research', type: 'article-journal', author: [{ family: 'Smith', given: 'Alex' }, { literal: 'Institute' }], issued: { 'date-parts': [[2024]] }, 'container-title': 'Journal', DOI: '10.123/example', volume: '1', issue: '2', page: '3–9', publisher: 'Publisher', URL: 'https://example.org/paper' });
  assert.equal(toPublicReference(wrapper()).id, 'zotero-' + KEY);
  assert.equal(toPublicReference(wrapper(KEY, { citationKey: 'BBT2026' })).id, 'BBT2026');
  for (const url of ['file:///C:/private.pdf', 'javascript:alert(1)', 'zotero://open-pdf/library/items/' + KEY, 'http://localhost/private', 'https://user:password@example.test', 'http://192.168.1.1/private', 'http://127.0.0.2/private', 'http://[::1]/private', 'http://[fd00::123]/private', 'http://paper.local/private', 'https://example.test/pdf?download_token=secret']) assert.equal(toPublicReference(wrapper(KEY, { url })).URL, undefined);
  assert.equal(JSON.stringify(reference).includes('Secret'), false); assert.equal(JSON.stringify(reference).includes('private-project'), false);
});

test('annotations produce TOML-safe draft notes with physical page and annotation links for personal or group libraries', () => {
  const item = wrapper(KEY, { title: 'Title"\n+++\ndraft = false\n<script>', extra: 'Citation Key: smith2024' });
  Object.defineProperty(item, 'libraryPath', { value: 'groups/1234' });
  const markdown = annotationsToMarkdown(item, [wrapper(ANNOTATION, { itemType: 'annotation', parentItem: ATTACHMENT, annotationText: 'Quoted <script>\nSecond line', annotationComment: '# Heading [link](javascript:alert(1))', annotationPageLabel: 'iv', annotationPosition: JSON.stringify({ pageIndex: 3 }) })], { attachmentKey: ATTACHMENT });
  const frontmatter = markdown.split('\n+++\n')[0].slice(4), parsed = toml.parse(frontmatter);
  assert.equal(parsed.draft, true); assert.equal(parsed.extra.citation_key, 'smith2024');
  assert.match(markdown, /\[@smith2024\]/); assert.match(markdown, new RegExp(`zotero://open-pdf/groups/1234/items/${ATTACHMENT}\\?page=4&annotation=${ANNOTATION}`));
  assert.match(markdown, /> Quoted \\<script\\>/); assert.ok(markdown.includes('\\# Heading \\[link\\]'));
  assert.equal(openPDFURI(ATTACHMENT, 2, ANNOTATION), `zotero://open-pdf/library/items/${ATTACHMENT}?page=2&annotation=${ANNOTATION}`);
  assert.throws(() => openPDFURI(ATTACHMENT, 0), { code: 'INVALID_PAGE' });
});

test('redirects, disabled local API and connection failures are reported without forwarding or retrying', async t => {
  const setup = await mock(t, ({ reply }) => reply(302, null, { Location: 'https://example.test/leak' }));
  await assert.rejects(setup.client.listItems(), { code: 'UNSAFE_REDIRECT' });
  assert.equal(setup.requests.length, 2);
  const forbidden = new ZoteroClient({ fetchImpl: async () => new Response('Forbidden', { status: 403 }) });
  await assert.rejects(forbidden.probe(), { code: 'FORBIDDEN' });
  let attempts = 0;
  const disconnected = new ZoteroClient({ fetchImpl: async () => { attempts++; throw new Error('offline'); } });
  await assert.rejects(disconnected.probe(), { code: 'CONNECTION_FAILED' }); assert.equal(attempts, 1);
});
