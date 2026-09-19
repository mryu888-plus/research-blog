const { test } = require('node:test');
const assert = require('node:assert/strict');
const { createSseParser, searchPosts, parseSearchIndex } = require('../site/static/main.js');
test('SSE preserves tokens across every possible network boundary', () => {
  const data = 'data: {"type":"token","text":"你好"}\r\n\r\ndata: {"type":"token","text":" world"}\n\ndata: [DONE]\n\n';
  for (let cut = 0; cut <= data.length; cut++) {
    const received = [];
    const parser = createSseParser(item => received.push(item));
    parser.push(data.slice(0, cut)); parser.push(data.slice(cut));
    assert.equal(received.map(item => item.text).join(''), '你好 world');
  }
});
test('SSE accepts one character at a time and ignores keepalive comments', () => {
  const received = [];
  const parser = createSseParser(item => received.push(item));
  for (const char of ':keepalive\n\ndata: {"type":"done"}\n\n') parser.push(char);
  assert.deepEqual(received, [{ type: 'done' }]);
});
test('Chinese and English search matches body, tags and all query terms', () => {
  const posts = [
    { title:'技能学习', body:'Agent 轨迹', tags:['research'], description:'' },
    { title:'其他笔记', body:'技能学习的研究', tags:[], description:'' },
  ];
  assert.equal(searchPosts(posts, '技能')[0], posts[0]);
  assert.deepEqual(searchPosts(posts, 'AGENT 轨迹'), [posts[0]]);
  assert.deepEqual(searchPosts(posts, '不存在'), []);
  assert.deepEqual(searchPosts(posts, '  '), []);
});

test('search index supports Zola preview injection and rejects malformed JSON', () => {
  const posts = [{title:'中文文章', body:'test', tags:[], description:''}];
  const json = JSON.stringify(posts);
  assert.deepEqual(parseSearchIndex(json), posts);
  assert.deepEqual(parseSearchIndex(json + '\r\n<script>window.LiveReloadOptions={port:1111};</script><script src="/livereload.js"></script>'), posts);
  assert.throws(() => parseSearchIndex('{broken'));
  assert.throws(() => parseSearchIndex('{}'), /Invalid search index/);
});

test('numeric formula entities become searchable TeX without changing article metadata', () => {
  const post = {
    title: '公式 &#92;alpha', description: '&#x03B1;', tags: ['&#945;'],
    url: 'https://example.test/?query=&#945;', extra: { body: '&#945;' },
    body: '&#92;frac&#123;a&#95;1&#125;&#123;b&#95;2&#125; &#x3B1; &#X1F600;',
  };
  const parsed = parseSearchIndex(JSON.stringify([post]));
  assert.deepEqual(parsed[0], { ...post, body: String.raw`\frac{a_1}{b_2} α 😀` });
  assert.deepEqual(searchPosts(parsed, String.raw`\frac a_1 α`), parsed);
});

test('numeric entity decoding is bounded, single-pass, and preserves malformed or named references', () => {
  const unchanged = '&amp; &alpha; &#; &#x; &#-1; &#xGG; &#65 &#0; &#55296; &#xDFFF; &#1114112; &#x110000; &#999999999999999999999999;';
  const [{ body }] = parseSearchIndex(JSON.stringify([{ body: `${unchanged} &#00065; &#x10FFFF; &#38;#92;` }]));
  assert.equal(body, `${unchanged} A ${String.fromCodePoint(0x10ffff)} &#92;`);
});

test('search index decoding preserves records with absent or non-string bodies', () => {
  const records = [null, 7, 'text', {}, { body: null }, { body: 9 }, { body: { value: '&#65;' } }];
  assert.deepEqual(parseSearchIndex(JSON.stringify(records)), records);
});
