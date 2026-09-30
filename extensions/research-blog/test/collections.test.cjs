'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const toml = require('smol-toml');
const core = require('../src/collections-core.cjs');
function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'blog-library-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const content = path.join(root, 'site/content'); fs.mkdirSync(content, { recursive: true });
  return { root, content };
}
test('series, groups, Chinese drafts, reassignment and reading order persist without moving files', t => {
  const { content } = fixture(t);
  const s1 = core.createSeries(content, '检索研究', '从词到向量'), s2 = core.createSeries(content, '其他笔记');
  const g1 = core.createGroup(content, s1.id, '基础'), g2 = core.createGroup(content, s1.id, '实践'), g3 = core.createGroup(content, s2.id, '基础');
  const a = core.createArticle(content, '从文字到向量', g1.id), b = core.createArticle(content, '检索的边界', g1.id);
  const original = fs.readFileSync(a.file);
  assert.equal(core.listArticles(content).find(x => x.key === a.key).draft, true);
  core.moveArticle(content, b.key, -1);
  assert.deepEqual(core.readCatalog(content).series[0].groups[0].articles, [b.key, a.key]);
  core.assignArticle(content, b.key, g1.id);
  assert.deepEqual(core.readCatalog(content).series[0].groups[0].articles, [b.key, a.key]);
  core.assignArticle(content, a.key, g2.id);
  core.assignArticle(content, b.key, g3.id);
  core.assignArticle(content, a.key, null);
  const final = core.readCatalog(content);
  assert.deepEqual(final.series[0].groups.flatMap(g => g.articles), []);
  assert.deepEqual(final.series[1].groups[0].articles, [b.key]);
  assert.deepEqual(fs.readFileSync(a.file), original);
  assert.equal(core.listArticles(content).length, 2);
});
test('duplicate titles and failed operations preserve catalog and existing articles', t => {
  const { content } = fixture(t);
  const series = core.createSeries(content, '同名'); const group = core.createGroup(content, series.id, '第一组');
  const article = core.createArticle(content, '现有文章', group.id);
  const catalogBefore = fs.readFileSync(core.catalogPath(content));
  assert.throws(() => core.createSeries(content, ' 同名 '), /重复/);
  assert.throws(() => core.createGroup(content, series.id, '第一组'), /重复/);
  assert.throws(() => core.createArticle(content, '现有文章'), /已经存在/);
  assert.throws(() => core.assignArticle(content, article.key, 'not-found'), /不存在/);
  assert.deepEqual(fs.readFileSync(core.catalogPath(content)), catalogBefore);
  assert.equal(fs.existsSync(core.catalogPath(content) + '.lock'), false);
});
test('bad catalogs, traversal, duplicate membership and concurrent mutation are rejected', t => {
  const { content, root } = fixture(t);
  assert.throws(() => core.safePath(content, '../escape'), /目录内/);
  assert.throws(() => core.articleKey('posts/../index.md'));
  assert.throws(() => core.articleKey('posts/a/index.md:secret'));
  const series = core.createSeries(content, '研究'), group = core.createGroup(content, series.id, '小组');
  const article = core.createArticle(content, '草稿', group.id);
  const data = core.readCatalog(content); data.series[0].groups[0].articles.push(article.key);
  assert.throws(() => core.validateCatalog(data), /一个系列或组/);
  fs.writeFileSync(core.catalogPath(content) + '.lock', '');
  assert.throws(() => core.createSeries(content, '并发'), /另一个窗口/);
  fs.unlinkSync(core.catalogPath(content) + '.lock');
  fs.writeFileSync(core.catalogPath(content), '{broken');
  assert.throws(() => core.createSeries(content, '不覆盖坏文件'), /检查 collections/);
  assert.equal(fs.readFileSync(core.catalogPath(content), 'utf8'), '{broken');
  // A junction/symlink must not turn a blog asset directory into an external write.
  const external = path.join(root, 'external'); fs.mkdirSync(external);
  const link = path.join(content, 'linked');
  try { fs.symlinkSync(external, link, process.platform === 'win32' ? 'junction' : 'dir'); }
  catch (error) { if (error.code === 'EPERM') return; throw error; }
  assert.throws(() => core.safePath(content, 'linked/file'), /符号链接|联接/);
});

test('direct series articles support ordered insertion and moves between series, groups and unassigned', t => {
  const { content } = fixture(t);
  const first = core.createSeries(content, '直接阅读'), second = core.createSeries(content, '其他系列');
  const group = core.createGroup(content, second.id, '可选分组');
  const a = core.createArticle(content, '甲', first.id), b = core.createArticle(content, '乙', first.id), c = core.createArticle(content, '丙', first.id);
  core.assignArticle(content, c.key, first.id, { beforeKey: a.key });
  assert.deepEqual(core.readCatalog(content).series[0].articles, [c.key, a.key, b.key]);
  core.assignArticle(content, c.key, first.id);
  assert.deepEqual(core.readCatalog(content).series[0].articles, [c.key, a.key, b.key]);
  core.assignArticle(content, c.key, first.id, { beforeKey: null });
  core.moveArticle(content, b.key, -1);
  assert.deepEqual(core.readCatalog(content).series[0].articles, [b.key, a.key, c.key]);
  core.assignArticle(content, a.key, group.id);
  core.assignArticle(content, a.key, second.id);
  core.assignArticle(content, c.key, null);
  const data = core.readCatalog(content);
  assert.deepEqual(data.series[0].articles, [b.key]);
  assert.deepEqual(data.series[1].articles, [a.key]);
  assert.deepEqual(data.series[1].groups[0].articles, []);
  assert.equal(core.listArticles(content).length, 3);
  data.series[1].groups[0].articles.push(a.key);
  assert.throws(() => core.validateCatalog(data), /一个系列或组/);
});

test('legacy group catalogs remain compatible and stale drag operations never overwrite newer edits', t => {
  const { content } = fixture(t);
  const series = core.createSeries(content, '旧目录'), group = core.createGroup(content, series.id, '旧组');
  const a = core.createArticle(content, '旧文', group.id), b = core.createArticle(content, '新文');
  const legacy = core.readCatalog(content); delete legacy.series[0].articles;
  fs.writeFileSync(core.catalogPath(content), JSON.stringify(legacy));
  const revision = core.catalogRevision(core.readCatalog(content));
  core.assignArticle(content, a.key, series.id, { beforeKey: null, expectedRevision: revision });
  const saved = fs.readFileSync(core.catalogPath(content));
  assert.throws(() => core.assignArticle(content, b.key, series.id, { beforeKey: null, expectedRevision: revision }), { code: 'CONFLICT' });
  assert.throws(() => core.assignArticle(content, b.key, series.id, { beforeKey: 'posts/missing/index.md' }), /目标文章/);
  assert.deepEqual(fs.readFileSync(core.catalogPath(content)), saved);
  assert.deepEqual(core.readCatalog(content).series[0].articles, [a.key]);
});
test('article titles are safely escaped in paths and TOML; partial source edits remain in the tree', t => {
  const { content } = fixture(t);
  const a = core.createArticle(content, '研究 "A" +++ / B');
  const text = fs.readFileSync(a.file, 'utf8');
  assert.equal(toml.parse(text.split('+++')[1]).title, '研究 "A" +++ / B');
  assert(core.inside(content, a.file));
  fs.writeFileSync(a.file, '+++\ntitle = "half typed\n+++\n');
  assert.equal(core.listArticles(content).length, 1);
  const reserved = core.createArticle(content, 'CON');
  assert.match(reserved.key, /文章-CON/);
});
