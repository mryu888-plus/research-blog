'use strict';
const http = require('node:http');
const { randomBytes } = require('node:crypto');
const { once } = require('node:events');
const core = require('./collections-core.cjs');

async function startPreviewServer({ contentRoot, origin }) {
  const source = new URL(origin);
  if (source.protocol !== 'http:' || !['127.0.0.1', 'localhost'].includes(source.hostname) || source.origin !== origin) throw new Error('预览编辑仅支持本机 HTTP 地址。');
  const token = randomBytes(32).toString('hex');
  const snapshot = () => {
    const catalog = core.readCatalog(contentRoot);
    return { catalog, revision: core.catalogRevision(catalog), articles: core.listArticles(contentRoot).map(({ key, title, draft }) => ({ key, title, draft })) };
  };
  const server = http.createServer(async (request, response) => {
    const send = (status, data) => {
      response.writeHead(status, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
      response.end(JSON.stringify(data));
    };
    // Even possession of the token never authorizes a different web origin.
    if (request.headers.host !== `127.0.0.1:${server.address().port}` || request.headers.origin !== origin) return send(403, { error: '请从本地博客预览中编辑。' });
    response.setHeader('Access-Control-Allow-Origin', origin);
    response.setHeader('Vary', 'Origin');
    if (request.method === 'OPTIONS') {
      response.setHeader('Access-Control-Allow-Methods', 'GET, POST');
      response.setHeader('Access-Control-Allow-Headers', 'Content-Type, X-Preview-Token');
      return send(204, null);
    }
    if (request.headers['x-preview-token'] !== token) return send(403, { error: '预览会话已失效，请重新打开预览。' });
    try {
      if (request.method === 'GET' && request.url === '/library') return send(200, snapshot());
      if (request.method !== 'POST' || request.url !== '/move') return send(404, { error: '没有这个编辑操作。' });
      if (!/^application\/json(?:;|$)/i.test(request.headers['content-type'] || '')) return send(415, { error: '需要 JSON 请求。' });
      if (Number(request.headers['content-length']) > 16384) return send(413, { error: '请求过大。' });
      let size = 0; const chunks = [];
      for await (const chunk of request) {
        size += chunk.length;
        if (size > 16384) return send(413, { error: '请求过大。' });
        chunks.push(chunk);
      }
      const data = JSON.parse(Buffer.concat(chunks).toString('utf8'));
      if (!data || typeof data.key !== 'string' || !(data.targetId === null || typeof data.targetId === 'string') ||
          !(data.beforeKey === null || typeof data.beforeKey === 'string') || !/^[a-f0-9]{64}$/.test(data.revision || '')) {
        return send(400, { error: '移动请求不完整，请刷新后重试。' });
      }
      core.assignArticle(contentRoot, data.key, data.targetId, { beforeKey: data.beforeKey, expectedRevision: data.revision });
      return send(200, snapshot());
    } catch (error) {
      send(error.code === 'CONFLICT' ? 409 : 400, { error: error.message });
    }
  });
  server.requestTimeout = 5000;
  server.headersTimeout = 5000;
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  return { server, config: { endpoint: `http://127.0.0.1:${server.address().port}`, token } };
}

if (require.main === module) {
  const [contentRoot, origin] = process.argv.slice(2);
  startPreviewServer({ contentRoot, origin }).then(({ config }) => process.stdout.write(JSON.stringify(config) + '\n'))
    .catch(error => { console.error(error.message); process.exitCode = 1; });
}
module.exports = { startPreviewServer };
