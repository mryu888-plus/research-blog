const { test } = require('node:test');
const assert = require('node:assert/strict');
const { parseWikiLinks, createKnowledgeGraph } = require('../site/static/knowledge-core.js');

const base = 'https://example.test/research-blog/';
const post = (slug, title, extra = {}) => ({
  id: `posts/${slug}/index.md`, slug, title, url: `${base}posts/${slug}/`,
  headings: [], references: [], links: [], ...extra,
});

test('wikilinks preserve Chinese text, aliases, heading targets and exact source ranges', () => {
  const text = '前文 [[技能学习]]，[[技能学习#问题与背景|研究背景]] 和 [[#我的思考]]。';
  const refs = parseWikiLinks(text);
  assert.equal(refs.length, 3);
  assert.equal(refs[0].target, '技能学习');
  assert.equal(refs[1].heading, '问题与背景');
  assert.equal(refs[1].label, '研究背景');
  assert.equal(refs[2].target, '');
  assert.equal(refs[2].label, '我的思考');
  for (const ref of refs) assert.equal(text.slice(ref.start, ref.end), ref.raw);
});

test('malformed, multiline and escaped wikilinks remain plain text', () => {
  for (const text of ['[[]]', '[[ |label]]', '[[a|]]', '[[a#]]', '[[a\nb]]', '[[a\rb]]', '[[[a]]]', '[[a]]]', '[[a|b|c]]', '[[a [[b]]', String.raw`\[[a]]`]) {
    assert.deepEqual(parseWikiLinks(text), [], text);
  }
  assert.equal(parseWikiLinks(String.raw`\\[[a]]`).length, 1);
  assert.deepEqual(parseWikiLinks(null), []);
});

test('Chinese titles and frontmatter aliases resolve without transliteration guesses', () => {
  const target = post('ji-neng-xue-xi', '技能学习', { aliases: ['可复用技能', 'Agent Skills'] });
  const graph = createKnowledgeGraph([target]);
  assert.equal(graph.resolve('技能学习').node.id, target.id);
  assert.equal(graph.resolve('可复用技能').url, target.url);
  assert.equal(graph.resolve('agent skills').node.id, target.id);
  assert.equal(graph.resolve('另一个标题').status, 'missing');
});

test('actual nested headings resolve by Chinese title, id and encoded id', () => {
  const target = post('skills', '技能', { headings: [{ id: 'overview', title: '概览', children: [{ id: '问题-背景', title: '问题与背景' }] }] });
  const graph = createKnowledgeGraph([target]);
  for (const heading of ['问题与背景', '问题-背景', encodeURIComponent('问题-背景')]) {
    assert.equal(graph.resolve(`技能#${heading}`).url, target.url + '#' + encodeURIComponent('问题-背景'));
  }
  assert.equal(graph.resolve('#概览', target.id).url, target.url + '#overview');
  assert.equal(graph.resolve('#概览', 'missing-source').status, 'missing');
  const invalid = graph.resolve('技能#还没写的章节');
  assert.equal(invalid.status, 'heading-missing');
  assert.equal(invalid.node.id, target.id);
  assert.equal(invalid.url, undefined);
});

test('ambiguous titles or aliases never choose an arbitrary article; paths take precedence', () => {
  const one = post('one', '重复标题', { aliases: ['别名'] });
  const two = post('two', '重复标题', { aliases: ['别名', 'one'] });
  const graph = createKnowledgeGraph([one, two]);
  assert.equal(graph.resolve('重复标题').status, 'ambiguous');
  assert.equal(graph.resolve('别名').status, 'ambiguous');
  assert.equal(graph.resolve('one').node.id, one.id);
  assert.equal(graph.resolve('posts/two/index.md').node.id, two.id);
});

test('relative Markdown paths, folders, slugs and canonical website paths resolve', () => {
  const source = post('source', '来源');
  const target = post('target', '目标');
  const graph = createKnowledgeGraph([source, target]);
  for (const path of [target.id, '@/posts/target/index.md', 'posts/target', 'target', '../target/index.md', '/research-blog/posts/target/', target.url]) {
    assert.equal(graph.resolve(path, source.id).node.id, target.id, path);
  }
  assert.equal(graph.resolve('./index.md', source.id).node.id, source.id);
});

test('directed edges combine repeated wiki and Markdown references and expose backlinks', () => {
  const a = post('a', '甲', { references: parseWikiLinks('[[乙]] [[乙|第二次]] [[甲]]'), links: [`${base}posts/b/?from=one#section`, `${base}posts/a/`] });
  const b = post('b', '乙', { references: parseWikiLinks('[[甲]]') });
  const graph = createKnowledgeGraph([a, b]);
  assert.equal(graph.edges.length, 2);
  assert.equal(graph.outgoing(a.id)[0].count, 3);
  assert.equal(graph.outgoing(a.id)[0].references.filter(ref => ref.kind === 'wiki').length, 2);
  assert.equal(graph.incoming(b.id)[0].source, a.id);
  assert.equal(graph.incoming(a.id)[0].source, b.id);
  assert.deepEqual(graph.incoming('unknown'), []);
  assert.deepEqual(graph.unresolved, []);
});

test('missing and ambiguous references are reported without producing misleading edges', () => {
  const source = post('source', '来源', { references: parseWikiLinks('[[缺失]] [[重复]] [[目标#不存在]]') });
  const graph = createKnowledgeGraph([source, post('a', '重复'), post('b', '重复'), post('target', '目标')]);
  assert.deepEqual(graph.unresolved.map(item => item.reason), ['missing', 'ambiguous', 'heading-missing']);
  assert.ok(graph.unresolved.every(item => item.source === source.id));
  assert.deepEqual(graph.edges, []);
});

test('external sites, unsafe URL schemes and encoded slash lookalikes do not create links', () => {
  const source = post('source', '来源', {
    links: ['https://elsewhere.test/research-blog/posts/target/', 'javascript:alert(1)', 'data:text/html,hi', `${base}posts%2Ftarget/`],
    references: parseWikiLinks('[[javascript:alert(1)]] [[https://elsewhere.test/research-blog/posts/target/]]'),
  });
  const graph = createKnowledgeGraph([source, post('target', '目标'), post('unsafe', '危险', { url: 'javascript:alert(1)' }), post('external', '外站', { url: 'https://elsewhere.test/article/' })]);
  assert.equal(graph.nodes.length, 2);
  assert.equal(graph.edges.length, 0);
  assert.ok(graph.unresolved.every(item => item.reason === 'missing'));
});

test('labels remain plain data and malicious heading permalinks cannot change the destination', () => {
  const target = post('target', '目标', { headings: [{ id: 'safe-heading', title: '标题', permalink: 'javascript:alert(1)' }] });
  const graph = createKnowledgeGraph([target]);
  const label = '<img src=x onerror=alert(1)>';
  const reference = parseWikiLinks(`[[目标#标题|${label}]]`)[0];
  const result = graph.resolve(reference);
  assert.equal(result.label, label);
  assert.equal(result.url, target.url + '#safe-heading');
  assert.equal(graph.resolve('//elsewhere.test/article').status, 'missing');
});

test('same-origin relative URLs support a deployment subpath; drafts follow supplied visibility', () => {
  const a = post('a', '草稿', { url: '/research-blog/posts/a/', draft: true });
  const b = post('b', '文章', { url: '/research-blog/posts/b/', links: ['../a/#heading'] });
  const graph = createKnowledgeGraph([a, b], { baseURL: base });
  assert.equal(graph.nodes[0].url, base + 'posts/a/');
  assert.equal(graph.resolve('草稿').status, 'resolved');
  assert.equal(graph.edges.length, 1);
  const publishedOnly = createKnowledgeGraph([b], { baseURL: base });
  assert.equal(publishedOnly.resolve('草稿').status, 'missing');
});

test('ordinary Unicode URLs match equivalent encoded canonical paths and ignore fragments', () => {
  const target = post('中文', '中文');
  const source = post('source', '来源', { links: [base + 'posts/' + encodeURIComponent('中文') + '/?search=1#section'] });
  const graph = createKnowledgeGraph([source, target]);
  assert.equal(graph.edges.length, 1);
  assert.equal(graph.edges[0].target, target.id);
});
