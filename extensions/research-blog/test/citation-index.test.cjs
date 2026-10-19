'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { URI } = require('vscode-uri');
const { BlogWorkspace } = require('../src/engine.cjs');
const { extractCitations, readReferences, authorText, yearText } = require('../src/reference-index.cjs');
const { TextDocument } = require('vscode-languageserver-textdocument');

function fixture(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'blog-citations-'));
  t.after(()=>fs.rmSync(directory,{recursive:true,force:true}));
  const root = path.join(directory,'site/content');
  fs.mkdirSync(path.join(root,'posts/a'),{recursive:true});
  fs.mkdirSync(path.join(directory,'site/data'),{recursive:true});
  const item = {id:'zotero-ABCD2345',citekey:'smith2024',title:'中文研究',author:[{family:'Smith',given:'Alice'}],issued:{'date-parts':[[2024]]},DOI:'10.1000/test'};
  fs.writeFileSync(path.join(directory,'site/data/references.json'),JSON.stringify({version:1,items:[item]},null,2));
  const w = new BlogWorkspace(root);
  const uri = URI.file(path.join(root,'posts/a/index.md')).toString();
  const add = body=>w.update(uri,`+++\ntitle = "文献笔记"\n+++\n${body}`);
  return {w,uri,add,root,directory,item};
}
test('citations support grouped keys and locators while ignoring code, math and escaped brackets',t=>{
  const {w,uri,add}=fixture(t);
  add('[@smith2024, p. 3; @missing]\n`[@code]`\n```text\n[@fence]\n```\n$[@math]$\n\\[@escaped]\n');
  const a=w.articles.get(uri);
  assert.deepEqual(a.citations.map(c=>c.key),['smith2024','missing']);
  assert.equal(a.citations[0].locator,'p. 3');
  assert.deepEqual(w.diagnostics(uri).filter(d=>d.code==='citation-missing').map(d=>d.message),['找不到文献「missing」。请从文献工作台插入引用，或补充 refs.bib。']);
  const p=a.doc.positionAt(a.text.indexOf('smith2024')+2);
  assert.match(w.hover(uri,p).contents.value,/中文研究\nSmith, Alice\n2024/);
  const definition=w.definition(uri,p);
  assert(definition.uri.endsWith('/data/references.json'));
  assert.equal(w.references(uri,p).length,1);
});

test('malformed citation groups are ignored as a whole, while valid whitespace and locators retain offsets', () => {
  for (const text of ['[@a; bad; @b]', '[@a; @]', '[@a;; @b]', '[@a b; @c]', '[@email@example.org]', '[@a\n; @b]']) {
    assert.deepEqual(extractCitations(text), [], text);
  }
  const text = '正文 [ @论文二 ; @paper.a, pp. 3-5 ]，再引 [@paper.a]。';
  const citations = extractCitations(text);
  assert.deepEqual(citations.map(c => [c.key, c.locator]), [['论文二', ''], ['paper.a', 'pp. 3-5'], ['paper.a', '']]);
  for (const citation of citations) assert.equal(text.slice(citation.start, citation.end), citation.key);
  assert.deepEqual(extractCitations(String.raw`\[@escape] ![@image](x.png) [@link](https://example.org/)`), []);
  assert.deepEqual(extractCitations(String.raw`\\[@even]`).map(c => c.key), ['even']);
});
test('completion replaces an entire partial key without duplicating brackets and tracks snapshot updates',t=>{
  const {w,uri,add,directory,item}=fixture(t);
  add('[@smXYZ]\n[@smith2024; @sm');
  let a=w.articles.get(uri);
  let position=a.doc.positionAt(a.text.indexOf('smXYZ')+2);
  let completion=w.completion(uri,position).find(c=>c.label==='smith2024');
  assert.equal(completion.textEdit.newText,'smith2024');
  assert.equal(a.doc.offsetAt(completion.textEdit.range.end),a.text.indexOf('smXYZ')+5);
  position=a.doc.positionAt(a.text.length);
  completion=w.completion(uri,position).find(c=>c.label==='smith2024');
  assert.equal(completion.textEdit.newText,'smith2024]');
  fs.writeFileSync(path.join(directory,'site/data/references.json'),JSON.stringify({version:1,items:[{...item,citekey:'newkey'}]}));
  w.open.add(uri);w.scan();
  assert(w.completion(uri,position).some(c=>c.label==='newkey'));
  add('```md\n[@\n```');a=w.articles.get(uri);
  assert.deepEqual(w.completion(uri,a.doc.positionAt(a.text.indexOf('[@')+2)),[]);
});
test('article-local BibTeX keys provide completion and F12 without requiring Zotero',t=>{
  const {w,uri,add,root}=fixture(t);
  fs.writeFileSync(path.join(root,'posts/a/refs.bib'),'@article{local2025, title={Local Paper}}');
  add('[@local2025]\n[@lo');
  const a=w.articles.get(uri);
  assert(!w.diagnostics(uri).some(d=>d.code==='citation-missing'));
  assert(w.completion(uri,a.doc.positionAt(a.text.length)).some(c=>c.label==='local2025'));
  assert(w.definition(uri,a.doc.positionAt(a.text.indexOf('local2025')+1)).uri.endsWith('/refs.bib'));
});

test('legal but invalid snapshot shapes never crash workspace scanning or citation UI', t => {
  const { w, uri, add, root, directory } = fixture(t);
  const snapshot = path.join(directory, 'site/data/references.json');
  add('[@smith2024]\n[@sm');
  w.open.add(uri);
  for (const data of [null, false, 'text', 12, [], { version: 1, items: null }, { version: 1, items: [null, [], false, { title: 'Bad', citekey: 12 }] }]) {
    fs.writeFileSync(snapshot, JSON.stringify(data));
    assert.doesNotThrow(() => w.scan());
    assert.equal(w.bibliography.size, 0);
    assert.equal(readReferences(root).size, 0);
    const article = w.articles.get(uri);
    assert.equal(w.definition(uri, article.doc.positionAt(article.text.indexOf('smith2024') + 1)), null);
    assert.equal(w.diagnostics(uri).filter(d => d.code === 'citation-missing').length, 1);
  }
});

test('malformed authors are normalized and hover/completion remain usable', t => {
  const { w, uri, add, directory, item } = fixture(t);
  add('[@smith2024]\n[@sm');
  w.open.add(uri);
  const snapshot = path.join(directory, 'site/data/references.json');
  for (const author of ['invalid', 42, null, [null, 'wrong', [], { family: 8 }, { given: 'Valid' }, { literal: 'Organization', note: 'PRIVATE' }]]) {
    fs.writeFileSync(snapshot, JSON.stringify({ version: 1, items: [{ ...item, author, issued: { 'date-parts': [[{ private: true }]] }, annotations: 'PRIVATE' }] }));
    w.scan();
    const article = w.articles.get(uri);
    const position = article.doc.positionAt(article.text.indexOf('smith2024') + 1);
    assert.doesNotThrow(() => w.hover(uri, position));
    assert.match(w.hover(uri, position).contents.value, /中文研究/);
    assert(w.completion(uri, article.doc.positionAt(article.text.length)).some(c => c.label === 'smith2024'));
    const entry = w.bibliography.get('smith2024');
    assert(Array.isArray(entry.author));
    assert.equal(entry.annotations, undefined);
    assert(!authorText(entry).includes('[object Object]'));
    assert.equal(yearText(entry), '');
  }
  assert.equal(authorText({ author: 'invalid' }), '');
  assert.equal(authorText({ author: [null, { family: 8 }, { literal: 'Organization' }] }), 'Organization');
});

test('F12 identifies the declaring property value even when titles and nested values repeat the key', t => {
  const { w, uri, add, directory } = fixture(t);
  const file = path.join(directory, 'site/data/references.json');
  const text = '\uFEFF{\n"version":1,\n"items":[\nnull,\n{"title":"samekey","author":[{"literal":"samekey"}],"citekey":"samekey","id":"zotero-A"},\n{"title":"fallbackkey","author":[{"literal":"fallbackkey"}],"id":"fallbackkey"}\n]\n}';
  fs.writeFileSync(file, text);
  w.open.add(uri); w.scan();
  add('[@samekey; @fallbackkey]');
  const article = w.articles.get(uri);
  const snapshot = TextDocument.create(URI.file(file).toString(), 'json', 0, text);
  for (const [key, property] of [['samekey', 'citekey'], ['fallbackkey', 'id']]) {
    const definition = w.definition(uri, article.doc.positionAt(article.text.indexOf(key) + 1));
    const expected = text.indexOf(`"${property}":"${key}"`) + property.length + 3;
    assert.equal(snapshot.offsetAt(definition.range.start), expected);
    assert.equal(snapshot.getText(definition.range), JSON.stringify(key));
  }
});

test('find references respects article-local declaration identity and global snapshot priority', t => {
  const { w, uri, add, root, directory } = fixture(t);
  fs.writeFileSync(path.join(directory, 'site/data/references.json'), JSON.stringify({ version: 1, items: [] }));
  w.open.add(uri); w.scan();
  fs.writeFileSync(path.join(root, 'posts/a/refs.bib'), '@article{samekey,title={First Paper}}');
  fs.mkdirSync(path.join(root, 'posts/b'), { recursive: true });
  fs.writeFileSync(path.join(root, 'posts/b/refs.bib'), '@article{samekey,title={Different Paper}}');
  const otherUri = URI.file(path.join(root, 'posts/b/index.md')).toString();
  const sharedUri = URI.file(path.join(root, 'posts/a/shared.md')).toString();
  add('[@samekey]\n[@samekey]');
  w.update(otherUri, '+++\ntitle = "Other"\n+++\n[@samekey]');
  w.update(sharedUri, '+++\ntitle = "Shared"\n+++\n[@samekey]');
  w.open.add(otherUri); w.open.add(sharedUri);
  let article = w.articles.get(uri);
  let position = article.doc.positionAt(article.text.indexOf('samekey') + 1);
  assert.deepEqual(w.references(uri, position).map(location => location.uri), [uri, uri, sharedUri]);
  const declared = w.references(uri, position, true);
  assert.equal(declared.length, 4);
  assert(declared[0].uri.endsWith('/posts/a/refs.bib'));
  const other = w.articles.get(otherUri);
  assert.deepEqual(w.references(otherUri, other.doc.positionAt(other.text.indexOf('samekey') + 1)).map(location => location.uri), [otherUri]);
  fs.writeFileSync(path.join(directory, 'site/data/references.json'), JSON.stringify({ version: 1, items: [{ id: 'zotero-GLOBAL', citekey: 'samekey', title: 'Global paper' }] }));
  w.scan();
  article = w.articles.get(uri); position = article.doc.positionAt(article.text.indexOf('samekey') + 1);
  assert.deepEqual(w.references(uri, position).map(location => location.uri), [uri, uri, otherUri, sharedUri]);
  assert(w.references(uri, position, true)[0].uri.endsWith('/data/references.json'));
});
