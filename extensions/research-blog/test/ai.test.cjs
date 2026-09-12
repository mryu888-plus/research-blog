'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const http = require('node:http');
const { once } = require('node:events');
const { completionContext, endpoint, requestCompletion } = require('../src/ai-core.cjs');
test('AI only activates in math, fenced code or Typst and bounds context', () => {
  for (const text of ['普通正文', '```py\nx = 1\n```\n普通正文', '$x$ 已结束', '`$x` 正文']) assert.equal(completionContext(text, text.length, 'markdown'), null);
  for (const text of ['$x = ', '$$\nx = ', '\\[x = ', '```python\nx = ', '~~~rust\nlet a = ']) assert(completionContext(text, text.length, 'markdown'));
  const source = 'x'.repeat(20000); const data = completionContext(source, 15000, 'typst', 1000);
  assert(data.prefix.length + data.suffix.length <= 1000);
});
test('endpoint rejects credentials, redirects by request policy, and insecure remote hosts', () => {
  assert.throws(() => endpoint('http://remote.test/v1'));
  assert.throws(() => endpoint('https://user:pass@example.com/v1'));
  assert.equal(endpoint('http://127.0.0.1:1234/v1/').pathname, '/v1/chat/completions');
});
test('AI request crosses real HTTP, respects model and token config, returns only insertion', async t => {
  let body, auth, requestPath;
  const server = http.createServer(async (req, res) => {
    let raw = ''; for await (const chunk of req) raw += chunk;
    body = JSON.parse(raw); auth = req.headers.authorization; requestPath = req.url;
    res.setHeader('Content-Type', 'application/json'); res.end(JSON.stringify({ choices: [{ message: { content: '```latex\nb^2\n```' } }] }));
  });
  server.listen(0, '127.0.0.1'); await once(server, 'listening'); t.after(() => server.close());
  const result = await requestCompletion({ baseUrl: `http://127.0.0.1:${server.address().port}/v1`, key: 'test-only', model: 'configured-model', maxTokens: 96 }, { kind: 'LaTeX math', prefix: '$a = ', suffix: '$' });
  assert.equal(result, 'b^2'); assert.equal(body.model, 'configured-model'); assert.equal(body.max_completion_tokens, 96);
  assert.equal(auth, 'Bearer test-only'); assert.equal(requestPath, '/v1/chat/completions');
  assert.deepEqual(JSON.parse(body.messages[1].content), { kind: 'LaTeX math', prefix: '$a = ', suffix: '$' });
});
test('AI requests can be aborted and do not expose response bodies on failures', async () => {
  const controller = new AbortController(); controller.abort();
  await assert.rejects(requestCompletion({ baseUrl: 'http://127.0.0.1:9/v1' }, {}, controller.signal), { name: 'AbortError' });
  await assert.rejects(requestCompletion({ baseUrl: 'https://example.test/v1' }, {}, undefined,
    async () => ({ ok: false, status: 401 })), /HTTP 401/);
});
