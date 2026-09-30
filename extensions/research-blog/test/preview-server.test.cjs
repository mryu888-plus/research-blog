'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const core = require('../src/collections-core.cjs');
const { startPreviewServer } = require('../src/preview-server.cjs');

test('preview API persists only authenticated same-origin moves and rejects stale or invalid writes', async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'blog-preview-editor-'));
  const contentRoot = path.join(root, 'site/content'); fs.mkdirSync(contentRoot, { recursive: true });
  const series = core.createSeries(contentRoot, '系列');
  const article = core.createArticle(contentRoot, '可拖动草稿');
  const { server, config } = await startPreviewServer({ contentRoot, origin: 'http://127.0.0.1:1111' });
  t.after(async () => { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); fs.rmSync(root, { recursive: true, force: true }); });
  const headers = { Origin: 'http://127.0.0.1:1111', 'X-Preview-Token': config.token, 'Content-Type': 'application/json' };
  const saved = fs.readFileSync(core.catalogPath(contentRoot));
  for (const rejected of [{}, { ...headers, Origin: 'https://example.test' }, { ...headers, 'X-Preview-Token': 'wrong' }]) {
    const response = await fetch(config.endpoint + '/move', { method: 'POST', headers: rejected, body: '{}' });
    assert.equal(response.status, 403);
  }
  const badHost = await new Promise((resolve, reject) => {
    const request = http.get(config.endpoint + '/library', { headers: { ...headers, Host: 'attacker.test' } }, response => { response.resume(); resolve(response.statusCode); });
    request.on('error', reject);
  });
  assert.equal(badHost, 403);
  assert.deepEqual(fs.readFileSync(core.catalogPath(contentRoot)), saved);
  const preflight = await fetch(config.endpoint + '/move', { method: 'OPTIONS', headers: { Origin: headers.Origin } });
  assert.equal(preflight.status, 204);
  assert.equal(preflight.headers.get('Access-Control-Allow-Origin'), headers.Origin);
  const state = await (await fetch(config.endpoint + '/library', { headers })).json();
  assert.equal(state.articles[0].key, article.key); assert.equal(state.articles[0].file, undefined);
  const body = { key: article.key, targetId: series.id, beforeKey: null, revision: state.revision };
  const move = await fetch(config.endpoint + '/move', { method: 'POST', headers, body: JSON.stringify(body) });
  assert.equal(move.status, 200);
  const next = await move.json();
  assert.deepEqual(core.readCatalog(contentRoot).series[0].articles, [article.key]);
  assert.notEqual(next.revision, state.revision);
  const stale = await fetch(config.endpoint + '/move', { method: 'POST', headers, body: JSON.stringify({ ...body, targetId: null }) });
  assert.equal(stale.status, 409);
  const bad = await fetch(config.endpoint + '/move', { method: 'POST', headers, body: JSON.stringify({ ...body, key: '../outside', revision: next.revision }) });
  assert.equal(bad.status, 400);
  const missingRevision = await fetch(config.endpoint + '/move', { method: 'POST', headers, body: JSON.stringify({ key: article.key, targetId: null, beforeKey: null }) });
  assert.equal(missingRevision.status, 400);
  assert.deepEqual(core.readCatalog(contentRoot).series[0].articles, [article.key]);
});
