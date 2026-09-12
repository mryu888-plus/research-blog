const { test } = require('node:test');
const assert = require('node:assert/strict');
const {
  safeUrl, normalizeGraph, neighborhood, filterGraph, layoutGraph, createCameraMotion,
} = require('../site/static/graph.js');

const base = 'https://example.com/research-blog/graph/';
const note = (id, title = id, extra = {}) => ({ id, title, url: `../posts/${id}/`, ...extra });
const connected = () => normalizeGraph({
  nodes: [note('a', '技能学习', { tags: ['Agent'] }), note('b', '规划'), note('c', '评估'), note('d', '独立笔记')],
  edges: [{ source: 'a', target: 'b' }, { source: 'b', target: 'c' }],
}, base);

test('node URLs retain the GitHub Pages subpath and reject executable protocols', () => {
  assert.equal(safeUrl('../posts/a/', base), 'https://example.com/research-blog/posts/a/');
  assert.equal(safeUrl('http://127.0.0.1:1111/posts/a/', base), 'http://127.0.0.1:1111/posts/a/');
  for (const url of ['javascript:alert(1)', 'data:text/html,hello', 'file:///C:/secret', 'vbscript:1', 'https://[invalid']) {
    assert.equal(safeUrl(url, base), null, url);
  }
});

test('normalization removes invalid nodes, dangling edges, duplicate edges and self links', () => {
  const input = {
    nodes: [note('b'), note('a', '中文', { draft: true }), note('a', '重复'), note('bad', '', { url: 'javascript:1' }), null],
    edges: [{ source: 'a', target: 'b' }, { source: 'a', target: 'b' }, { source: 'b', target: 'a' },
      { source: 'a', target: 'bad' }, { source: 'a', target: 'missing' }, { source: 'a', target: 'a' }],
  };
  const snapshot = JSON.stringify(input);
  const graph = normalizeGraph(input, base);
  assert.deepEqual(graph.nodes.map(node => node.id), ['a', 'b']);
  assert.equal(graph.nodes[0].draft, true);
  assert.deepEqual(graph.edges, [{ source: 'a', target: 'b' }, { source: 'b', target: 'a' }]);
  assert.equal(JSON.stringify(input), snapshot);
});

test('focus includes incoming and outgoing neighbors but does not traverse a second hop', () => {
  const graph = connected();
  assert.deepEqual([...neighborhood(graph, 'b')].sort(), ['a', 'b', 'c']);
  assert.deepEqual(filterGraph(graph, '', 'a').nodes.map(node => node.id), ['a', 'b']);
  assert.deepEqual(filterGraph(graph, '', 'a').edges, [{ source: 'a', target: 'b' }]);
  assert.deepEqual(filterGraph(graph, '', 'd').nodes.map(node => node.id), ['d']);
  assert.deepEqual(filterGraph(graph, '', 'd').edges, []);
});

test('Chinese title and case-insensitive tag search intersect the selected neighborhood', () => {
  const graph = connected();
  assert.deepEqual(filterGraph(graph, '技能').nodes.map(node => node.id), ['a']);
  assert.deepEqual(filterGraph(graph, '  AGENT  ').nodes.map(node => node.id), ['a']);
  assert.deepEqual(filterGraph(graph, '技能', 'c').nodes, []);
  const one = filterGraph(graph, '规划', 'a');
  assert.deepEqual(one.nodes.map(node => node.id), ['b']);
  assert.deepEqual(one.edges, []);
});

test('clearing a no-result filter restores the graph without mutating its topology', () => {
  const graph = connected();
  const snapshot = JSON.stringify(graph);
  assert.deepEqual(filterGraph(graph, '没有这篇文章'), { nodes: [], edges: [] });
  assert.deepEqual(filterGraph(graph, '', ''), graph);
  assert.equal(JSON.stringify(graph), snapshot);
});

test('empty and single-node layouts are finite and centered', () => {
  assert.equal(layoutGraph({ nodes: [], edges: [] }).size, 0);
  const positions = layoutGraph(normalizeGraph({ nodes: [note('only')], edges: [] }, base));
  assert.deepEqual(positions.get('only'), { x: 0, y: 0 });
});

test('layout is deterministic across updates and cached coordinates cannot be mutated by dragging', () => {
  const graph = connected();
  const first = layoutGraph(graph);
  const second = layoutGraph(graph);
  assert.deepEqual(first, second);
  first.get('a').x = 999999;
  assert.notEqual(second.get('a').x, 999999);
  assert.deepEqual(layoutGraph(graph), second);
  const renamed = { ...graph, nodes: graph.nodes.map(node => ({ ...node, title: '改名后' })) };
  assert.deepEqual(layoutGraph(renamed), second);
});

test('large collections use a finite deterministic layout without dropping notes', () => {
  const graph = { nodes: Array.from({ length: 1000 }, (_, i) => note(`large-${i}`)), edges: [] };
  const positions = layoutGraph(graph);
  assert.equal(positions.size, 1000);
  assert.ok([...positions.values()].every(point => Number.isFinite(point.x) && Number.isFinite(point.y)));
  assert.deepEqual(layoutGraph(graph), positions);
});

function cameraHarness() {
  let clock=0, serial=0, reduced=false;
  const pending=new Map(), views=[];
  const camera=createCameraMotion({
    request:callback => { pending.set(++serial,callback); return serial; },
    cancel:id => pending.delete(id), now:() => clock, reduced:() => reduced,
    apply:view => views.push(view),
  });
  return {
    camera, pending, views, reduce:() => { reduced=true; },
    step(milliseconds) {
      clock+=milliseconds;
      const callbacks=[...pending.values()]; pending.clear();
      callbacks.forEach(callback => callback(clock));
    },
  };
}
const origin={zoom:1,pan:{x:0,y:0}};
const destination={zoom:2,pan:{x:300,y:-120}};

test('camera motion reaches its target and leaves no perpetual animation frame', () => {
  const {camera,pending,views,step}=cameraHarness();
  camera.move(origin,destination);
  assert.equal(pending.size,1);
  step(190);
  assert.ok(views.at(-1).zoom>1 && views.at(-1).zoom<2);
  assert.equal(pending.size,1);
  step(190);
  assert.deepEqual(views.at(-1),destination);
  assert.equal(pending.size,0);
  const count=views.length;
  step(10000);
  assert.equal(views.length,count);
});

test('a new camera action replaces the previous transition and disposal cancels it', () => {
  const {camera,pending,views,step}=cameraHarness();
  camera.move(origin,destination);
  step(80);
  const replacement={zoom:.8,pan:{x:20,y:30}};
  camera.move(views.at(-1),replacement);
  assert.equal(pending.size,1);
  step(380);
  assert.deepEqual(views.at(-1),replacement);
  camera.move(replacement,origin);
  camera.stop();
  assert.equal(pending.size,0);
  const count=views.length;
  step(1000);
  assert.equal(views.length,count);
});

test('reduced motion applies the destination immediately without scheduling animation', () => {
  const {camera,pending,views,reduce}=cameraHarness();
  reduce(); camera.move(origin,destination);
  assert.deepEqual(views,[destination]);
  assert.equal(pending.size,0);
});

test('changing motion preference mid-transition settles once and cancels pending frames', () => {
  const {camera,pending,views,step,reduce}=cameraHarness();
  camera.move(origin,destination); step(70);
  reduce(); camera.finish();
  assert.deepEqual(views.at(-1),destination);
  assert.equal(pending.size,0);
  const count=views.length;
  camera.finish(); step(1000);
  assert.equal(views.length,count);
});
