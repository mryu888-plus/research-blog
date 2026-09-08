const { test } = require('node:test');
const assert = require('node:assert/strict');
const { createSseParser, searchPosts } = require('../site/static/main.js');
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
