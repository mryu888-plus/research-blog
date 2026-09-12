const { test } = require('node:test');
const assert = require('node:assert/strict');
const { buildTopics, findTopic } = require('../site/static/topics.js');

function post(slug, date = '2026-09-01') {
  return { url: `https://example.test/blog/posts/${slug}/`, title: slug,
    description: `${slug} 的摘要`, date, dateLabel: date.replaceAll('-', '.') };
}

function group(id, posts, label = id) {
  return { id, label, description: `${label} 的介绍`,
    url: `https://example.test/blog/tags/${id}/`, posts };
}

function flatten(nodes) {
  return nodes.flatMap(node => [node, ...flatten(node.children)]);
}

test('three-level categories include all descendant articles in date order', () => {
  const rootPost = post('总览', '2026-09-02');
  const middlePost = post('决策', '2026-09-03');
  const leafPost = post('技能', '2026-09-01');
  const { tree } = buildTopics([
    group('research', [rootPost], '研究'),
    group('learning', [middlePost], '学习'),
    group('skills', [leafPost], '技能发现'),
  ], [{ id: 'root', tag: 'research', children: [
    { id: 'middle', tag: 'learning', children: [{ id: 'leaf', tag: 'skills' }] },
  ] }]);

  assert.equal(tree.length, 1);
  assert.deepEqual(tree[0].posts, [middlePost, rootPost, leafPost]);
  assert.deepEqual(tree[0].children[0].posts, [middlePost, leafPost]);
  assert.deepEqual(tree[0].children[0].children[0].posts, [leafPost]);
  assert.equal(tree[0].children[0].children[0].label, '技能发现');
});

test('an article assigned to multiple tags appears once per category and in all articles', () => {
  const shared = post('跨主题文章', '2026-09-04');
  const other = post('另一篇');
  const { tree, posts } = buildTopics([
    group('parent', [shared]),
    group('a', [{ ...shared }, other]),
    group('b', [{ ...shared }]),
  ], [{ tag: 'parent', children: [{ tag: 'a' }, { tag: 'b' }] }]);

  assert.deepEqual(tree[0].posts.map(item => item.url), [shared.url, other.url]);
  assert.deepEqual(posts.map(item => item.url), [shared.url, other.url]);
  assert.deepEqual(tree[0].children.map(node => node.posts.length), [2, 1]);
});

test('tags outside the configured hierarchy retain their original labels and articles', () => {
  const known = group('learning', [post('学习笔记')], '强化学习');
  const untranslated = group('new-topic', [post('新主题')]);
  const uncategorized = group('essays', [post('随笔')], '随笔');
  const { tree, posts } = buildTopics([known, untranslated, uncategorized], [
    { id: 'learning-root', label: '学习与决策', children: [{ tag: 'learning' }] },
  ]);

  assert.deepEqual(tree.map(node => node.label), ['学习与决策', 'new-topic', '随笔']);
  for (const original of [untranslated, uncategorized]) {
    const retained = tree.find(node => node.label === original.label);
    assert.equal(retained.description, original.description);
    assert.equal(retained.url, original.url);
    assert.deepEqual(retained.posts, original.posts);
  }
  assert.equal(posts.length, 3);
});

test('missing tags and unused branches are omitted while ancestors of existing tags survive', () => {
  const note = post('已有笔记');
  const { tree } = buildTopics([group('present', [note], '已有主题')], [
    { id: 'unused', label: '尚未使用', children: [{ tag: 'absent' }] },
    { id: 'retained', tag: 'missing-parent', label: '保留分组', children: [
      { id: 'empty', children: [{ tag: 'another-missing' }] },
      { id: 'present', tag: 'present' },
    ] },
    null,
  ]);

  assert.equal(tree.length, 1);
  assert.equal(tree[0].id, 'retained');
  assert.deepEqual(tree[0].posts, [note]);
  assert.deepEqual(tree[0].children.map(node => node.id), ['present']);
  assert.equal(findTopic(tree, 'unused'), null);
  assert.equal(findTopic(tree, 'empty'), null);
});

test('missing or invalid hierarchy configuration still exposes every existing tag', () => {
  const groups = [group('a', [post('甲')]), group('b', [post('乙')])];
  for (const definitions of [undefined, null, {}, []]) {
    const model = buildTopics(groups, definitions);
    assert.deepEqual(model.tree.map(node => node.label), ['a', 'b']);
    assert.equal(model.posts.length, 2);
  }
  assert.deepEqual(buildTopics([], [{ label: '空分类', tag: 'missing' }]), { tree: [], posts: [] });
});

test('findTopic resolves nested IDs and returns null for unknown IDs', () => {
  const { tree } = buildTopics([group('deep', [post('深层笔记')])], [
    { id: 'one', label: '一级', children: [
      { id: 'two', label: '二级', children: [{ id: 'three', tag: 'deep' }] },
    ] },
  ]);
  assert.equal(findTopic(tree, 'one'), tree[0]);
  assert.equal(findTopic(tree, 'two'), tree[0].children[0]);
  assert.equal(findTopic(tree, 'three'), tree[0].children[0].children[0]);
  assert.equal(findTopic(tree, 'missing'), null);
  assert.equal(findTopic([], 'one'), null);
});

test('duplicate configured IDs never hide categories or collide with generated IDs', () => {
  const groups = ['a', 'b', 'c', 'extra'].map(id => group(id, [post(id)]));
  const { tree } = buildTopics(groups, [
    { id: 'shared', label: '分组', children: [
      { id: 'shared', tag: 'a' },
      { id: 'shared', tag: 'b' },
    ] },
    { id: 'topic-extra-0', tag: 'c' },
  ]);
  const nodes = flatten(tree);
  assert.equal(nodes.length, 5);
  assert.equal(new Set(nodes.map(node => node.id)).size, nodes.length);
  for (const node of nodes) assert.equal(findTopic(tree, node.id), node);
  assert.deepEqual(nodes.filter(node => !node.children.length).map(node => node.label), ['a', 'b', 'c', 'extra']);
});
