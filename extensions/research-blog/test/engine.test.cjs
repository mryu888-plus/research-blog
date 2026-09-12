'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { URI } = require('vscode-uri');
const { BlogWorkspace } = require('../src/engine.cjs');
const root = path.resolve(__dirname, '../.test-work/engine/content');
const article = (title, body = '', extra = '') => `+++\ntitle = "${title}"\ndate = 2026-09-12\n${extra}\n+++\n${body}`;
function setup() { return new BlogWorkspace(root); }
function add(w, slug, text) { const uri = URI.file(path.join(root, 'posts', slug, 'index.md')).toString(); w.update(uri, text); return uri; }
function at(w, uri, needle, shift = 1) { const a = w.articles.get(uri); return a.doc.positionAt(a.text.indexOf(needle) + shift); }
test('Chinese aliases, headings, relative paths, CRLF and UTF-16 locations', () => {
  const w = setup();
  const target = add(w, '目标', article('中文标题', '## 第一节\n\n## Explicit {#known}\n', '[extra]\naliases = ["别名"]'));
  const source = add(w, '来源', article('来源', '😀 [[别名#第一节|显示]]\n[[../目标/index.md]]\n[[中文标题#known]]').replace(/\n/g, '\r\n'));
  assert.equal(w.diagnostics(source).length, 0);
  const def = w.definition(source, at(w, source, '别名'));
  assert.equal(def.uri, target); assert.equal(def.range.start.line, 6);
  assert.equal(w.references(target, at(w, target, 'title'), false).length, 3);
  assert.equal(w.references(target, at(w, target, '第一节'), false).length, 1);
});
test('missing, ambiguous, missing headings and published-to-draft links', () => {
  const w = setup();
  add(w, 'one', article('同名')); add(w, 'two', article('同名'));
  add(w, 'draft', article('草稿', '', 'draft = true'));
  const uri = add(w, 'source', article('源', '[[丢失]] [[同名]] [[draft#不存在]] [[草稿]]'));
  assert.deepEqual(w.diagnostics(uri).map(d => d.code), ['link-missing', 'link-ambiguous', 'link-heading-missing', 'link-draft']);
});
test('frontmatter, code, escaped links, comments and math are excluded', () => {
  const w = setup();
  const uri = add(w, 'a', article('[[元数据]]', '```md\n[[代码块]]\n```\n\n    [[缩进]]\n\n`[[行内]]`\n$$\n[[公式]]\n$$\n$x [[内联公式]]$\n\\[[转义]]\n<!-- [[注释]] -->\n<code>[[HTML]]</code>\n[[有效]]'));
  assert.equal(w.diagnostics(uri).length, 1); assert.match(w.diagnostics(uri)[0].message, /有效/);
});
test('completion replaces partial target without duplicating suffix, and works unsaved', () => {
  const w = setup(); const target = add(w, 'target', article('文章', '## 章节'));
  let uri = add(w, 'a', article('源', '[[文|自定义]]'));
  let p = at(w, uri, '[[文', 3);
  const item = w.completion(uri, p).find(i => i.label === '文章');
  assert.equal(item.textEdit.newText, '文章');
  uri = add(w, 'a', article('源', '[[文章#章]]'));
  assert.equal(w.completion(uri, at(w, uri, '#章', 2))[0].textEdit.newText, '章节');
  w.update(target, article('修改后的标题', '## 新章节'), 2);
  assert.equal(w.diagnostics(uri)[0].code, 'link-missing');
  uri = add(w, 'a', article('源', '[[修改'));
  assert.equal(w.completion(uri, w.articles.get(uri).doc.positionAt(w.articles.get(uri).text.length))[0].textEdit.newText, '修改后的标题]]');
});
test('TOML parsing handles dates, duplicate keys and wrong field types', () => {
  const w = setup(); let uri = add(w, 'a', article('标题'));
  assert.equal(w.diagnostics(uri).length, 0);
  w.update(uri, article('标题', '', 'title = "重复"')); assert.equal(w.diagnostics(uri)[0].code, 'toml-syntax');
  w.update(uri, article('标题', '', 'draft = "false"')); assert.equal(w.diagnostics(uri)[0].code, 'metadata-type');
  w.update(uri, '+++\ntitle = "未闭合"'); assert.equal(w.diagnostics(uri)[0].code, 'frontmatter-unclosed');
});
test('tags complete across lines, use configured labels, omit selected tags, hover and references', () => {
  const w = setup(); w.tagLabels = { agent: '智能体', rl: '强化学习' };
  add(w, 'a', article('甲', '', '[taxonomies]\ntags = ["agent", "中文"]'));
  const uri = add(w, 'b', article('乙', '', '[taxonomies]\ntags = [\n  "中文",\n  "ag"\n]'));
  const items = w.completion(uri, at(w, uri, '"ag"', 3));
  assert(items.some(i => i.label === 'agent' && i.detail.includes('智能体')));
  assert(!items.some(i => i.label === '中文'));
  assert.equal(w.references(uri, at(w, uri, '中文'), false).length, 2);
  assert.match(w.hover(uri, at(w, uri, '中文')).contents.value, /2 篇/);
  w.update(uri, article('乙', '', '[taxonomies]\ntags = ["agent", "agent"]'));
  assert.equal(w.diagnostics(uri)[0].code, 'tag-duplicate');
});
test('filesystem index honors unsaved overlays and removes deleted files', () => {
  const dir = path.join(root, 'posts', 'disk'); fs.mkdirSync(dir, { recursive: true });
  const file = path.join(dir, 'index.md'); const uri = URI.file(file).toString();
  const w = setup(); fs.writeFileSync(file, article('磁盘')); w.scan();
  assert.equal(w.articles.get(uri).title, '磁盘');
  w.open.add(uri); w.update(uri, article('未保存'));
  fs.writeFileSync(file, article('外部')); w.scan(); assert.equal(w.articles.get(uri).title, '未保存');
  w.open.delete(uri); w.scan(); assert.equal(w.articles.get(uri).title, '外部');
  fs.unlinkSync(file); w.scan(); assert(!w.articles.has(uri));
});
test('Typst components complete IDs, navigate sources and report missing figures', () => {
  const w = setup(); const uri = add(w, 'fig', article('图形', '{{ <fig id="diagram" caption="图" /> }}'));
  const dir = path.join(root, 'posts', 'fig', 'figures'); fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'diagram.typ'), '$ x^2 $');
  assert.equal(w.diagnostics(uri).length, 0);
  assert(w.definition(uri, at(w, uri, 'diagram')).uri.endsWith('diagram.typ'));
  assert.equal(w.completion(uri, at(w, uri, 'diagram', 2))[0].label, 'diagram');
  w.update(uri, article('图形', '{{ <fig id="../secret" /> }}'));
  assert.equal(w.diagnostics(uri)[0].code, 'figure-missing');
});
