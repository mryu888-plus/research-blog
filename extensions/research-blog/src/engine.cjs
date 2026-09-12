'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { URI } = require('vscode-uri');
const { TextDocument } = require('vscode-languageserver-textdocument');
const { parse: parseToml } = require('smol-toml');
const MarkdownIt = require('markdown-it');
const { parseWikiLinks, createKnowledgeGraph } = require('../../../site/static/knowledge-core.js');
const markdown = new MarkdownIt({ html: true });
const key = value => String(value).trim().normalize('NFC').toLowerCase();
const decode = value => { try { return decodeURIComponent(value); } catch { return value; } };
const escapeMarkdown = value => String(value).replace(/[\\`*_{}[\]()<>#+.!|]/g, '\\$&');
const strings = value => Array.isArray(value) && value.every(item => typeof item === 'string');
const range = (doc, start, end = start) => ({ start: doc.positionAt(start), end: doc.positionAt(end) });
function diagnostic(doc, start, end, message, code, severity = 1) {
  return { range: range(doc, start, end), message, code, severity, source: 'research-blog' };
}
function mask(text, start, end) {
  return text.slice(0, start) + text.slice(start, end).replace(/[^\r\n]/g, ' ') + text.slice(end);
}
function parseArticle(uri, text, root, version = 0) {
  const doc = TextDocument.create(uri, 'markdown', version, text);
  const file = URI.parse(uri).fsPath;
  const id = path.relative(root, file).replace(/\\/g, '/');
  const lines = text.split('\n');
  const offsets = []; let offset = 0;
  for (const line of lines) { offsets.push(offset); offset += line.length + 1; }
  const errors = []; let metadata = {}, bodyStart = 0, fmEnd = 0;
  if (lines[0]?.replace(/^\uFEFF/, '').trim() === '+++') {
    const end = lines.findIndex((line, i) => i > 0 && line.trim() === '+++');
    if (end < 0) {
      errors.push(diagnostic(doc, 0, lines[0].length, 'TOML frontmatter 缺少结束标记 +++。', 'frontmatter-unclosed'));
      bodyStart = text.length; fmEnd = text.length;
    } else {
      fmEnd = offsets[end]; bodyStart = offsets[end] + lines[end].length + 1;
      try { metadata = parseToml(lines.slice(1, end).join('\n').replace(/\r\n?/g, '\n')); }
      catch (error) {
        const line = Math.min(end - 1, Math.max(1, error.line || 1));
        const at = (offsets[line] || 0) + Math.max(0, (error.column || 1) - 1);
        errors.push(diagnostic(doc, at, Math.min(at + 1, text.length), `TOML 格式错误：${error.message}`, 'toml-syntax'));
      }
    }
  } else {
    errors.push(diagnostic(doc, 0, lines[0]?.length || 0, '主题文章需要以 +++ 包围 TOML frontmatter。', 'frontmatter-missing'));
  }
  function fieldOffset(field) {
    const pattern = new RegExp(`^\\s*${field}\\s*=`);
    const index = lines.findIndex((line, i) => offsets[i] < fmEnd && pattern.test(line));
    return index < 0 ? 0 : offsets[index];
  }
  if (!errors.length) {
    const checks = [
      ['title', typeof metadata.title === 'string' && !!metadata.title.trim(), 'title 必须是非空字符串。'],
      ['description', metadata.description === undefined || typeof metadata.description === 'string', 'description 必须是字符串。'],
      ['draft', metadata.draft === undefined || typeof metadata.draft === 'boolean', 'draft 必须为 true 或 false。'],
      ['tags', metadata.taxonomies?.tags === undefined || strings(metadata.taxonomies.tags), 'taxonomies.tags 必须是字符串数组。'],
      ['aliases', metadata.extra?.aliases === undefined || strings(metadata.extra.aliases), 'extra.aliases 必须是字符串数组。'],
    ];
    for (const [field, valid, message] of checks) {
      if (!valid) errors.push(diagnostic(doc, fieldOffset(field), fieldOffset(field) + field.length, message, 'metadata-type'));
    }
  }
  // Retain source offsets while removing content the website does not turn into links.
  let visible = mask(text, 0, bodyStart);
  const tokens = markdown.parse(visible, {});
  for (const token of tokens) {
    if (['fence', 'code_block'].includes(token.type) && token.map) {
      visible = mask(visible, offsets[token.map[0]], offsets[token.map[1]] ?? text.length);
    }
  }
  for (const expression of [/<(?:pre|code|script|style|textarea)\b[^>]*>[\s\S]*?<\/(?:pre|code|script|style|textarea)>/gi,
    /<!--[\s\S]*?-->/g, /(`+)[\s\S]*?\1(?!`)/g,
    /(?<!\\)\$\$[\s\S]*?(?<!\\)\$\$/g, /(?<![\\$])\$(?!\$)[^\n$]*?(?<!\\)\$/g,
    /\\\[[\s\S]*?\\\]/g, /\\\([\s\S]*?\\\)/g]) {
    for (const match of visible.matchAll(expression)) visible = mask(visible, match.index, match.index + match[0].length);
  }
  const headings = [];
  for (let i = 0; i < tokens.length; i++) {
    if (tokens[i].type !== 'heading_open') continue;
    const inline = tokens[i + 1];
    const explicit = inline.content.match(/\s*\{#([^}\s]+)(?:\s+[^}]*)?\}\s*$/);
    const title = (inline.children || []).map(child => ['text', 'code_inline', 'image'].includes(child.type) ? child.content : child.type === 'softbreak' ? ' ' : '').join('')
      .replace(/\s*\{#[^}]+\}\s*$/, '').trim();
    const start = offsets[tokens[i].map[0]];
    const asciiAnchor = /^[\x00-\x7f]*$/.test(title) ? title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') : '';
    const baseId = explicit?.[1] || asciiAnchor || `__source_heading_${headings.length}`;
    let headingId = baseId, duplicate = 0;
    while (headings.some(h => h.id === headingId)) headingId = `${baseId}-${++duplicate}`;
    headings.push({ title, id: headingId, explicit: !!explicit,
      level: Number(tokens[i].tag.slice(1)), range: range(doc, start, (offsets[tokens[i].map[1]] ?? text.length) - 1) });
  }
  const figures = [];
  for (const match of visible.matchAll(/\{\{\s*<fig\b[\s\S]*?\/>\s*\}\}/g)) {
    const attr = /\bid\s*=\s*(["'])(.*?)\1/.exec(match[0]);
    if (!attr) continue;
    const start = match.index + attr.index + attr[0].indexOf(attr[1]) + 1;
    figures.push({ id: attr[2], start, end: start + attr[2].length });
  }
  const tagItems = [];
  const taxonomy = /\[taxonomies\][^\S\n]*\r?\n([\s\S]*?)(?=^\s*\[[^\]]+\]|(?![\s\S]))/m.exec(text.slice(0, fmEnd));
  if (taxonomy) {
    const array = /\btags\s*=\s*\[([\s\S]*?)\]/.exec(taxonomy[1]);
    if (array) {
      const arrayStart = taxonomy.index + taxonomy[0].indexOf(taxonomy[1]) + array.index + array[0].indexOf('[') + 1;
      for (const item of array[1].matchAll(/"(?:[^"\\]|\\.)*"|'[^']*'/g)) {
        try {
          const value = parseToml(`value = ${item[0]}`).value;
          tagItems.push({ value, start: arrayStart + item.index, end: arrayStart + item.index + item[0].length });
        } catch { /* A syntax diagnostic already identifies malformed TOML. */ }
      }
    }
  }
  const slug = typeof metadata.slug === 'string' ? metadata.slug : path.basename(file) === 'index.md' ? path.basename(path.dirname(file)) : path.basename(file, '.md');
  const urlPath = typeof metadata.path === 'string' ? metadata.path.replace(/^\/+|\/+$/g, '') : id.replace(/(?:index|_index)\.md$/, '').replace(/\.md$/, '').replace(/\/+$/, '');
  return { uri, file, id, doc, text, metadata, fmEnd, bodyStart, visible, headings, figures, tagItems, errors,
    title: typeof metadata.title === 'string' ? metadata.title : id,
    titleRange: range(doc, fieldOffset('title'), fieldOffset('title') + (lines[doc.positionAt(fieldOffset('title')).line]?.length || 0)),
    aliases: strings(metadata.extra?.aliases) ? metadata.extra.aliases : [],
    tags: strings(metadata.taxonomies?.tags) ? metadata.taxonomies.tags : [],
    draft: metadata.draft === true, slug, url: `https://research-blog.invalid/${urlPath}/`,
    references: parseWikiLinks(visible), isPost: id.startsWith('posts/') && path.basename(file) !== '_index.md',
  };
}
class BlogWorkspace {
  constructor(root) { this.root = path.resolve(root); this.articles = new Map(); this.open = new Set(); this.graphCache = null; this.tagLabels = {}; }
  accepts(uri) {
    if (URI.parse(uri).scheme !== 'file') return false;
    const relative = path.relative(this.root, URI.parse(uri).fsPath);
    return relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative) && /\.md$/i.test(relative);
  }
  update(uri, text, version = 0) {
    if (!this.accepts(uri)) return;
    this.articles.set(uri, parseArticle(uri, text, this.root, version)); this.graphCache = null;
  }
  scan() {
    try { this.tagLabels = parseToml(fs.readFileSync(path.join(this.root, '..', 'config.toml'), 'utf8')).extra?.tag_labels || {}; }
    catch { this.tagLabels = {}; }
    const seen = new Set();
    const walk = dir => {
      let entries;
      try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch (error) { if (error.code === 'ENOENT') return; throw error; }
      for (const entry of entries) {
        const file = path.join(dir, entry.name);
        if (entry.isSymbolicLink()) continue;
        if (entry.isDirectory()) walk(file);
        else if (entry.isFile() && /\.md$/i.test(file)) {
          const uri = URI.file(file).toString(); seen.add(uri);
          if (!this.open.has(uri)) {
            const text = fs.readFileSync(file, 'utf8');
            if (this.articles.get(uri)?.text !== text) this.update(uri, text);
          }
        }
      }
    };
    walk(this.root);
    for (const uri of this.articles.keys()) if (!seen.has(uri) && !this.open.has(uri)) { this.articles.delete(uri); this.graphCache = null; }
  }
  graph() {
    if (!this.graphCache) this.graphCache = createKnowledgeGraph([...this.articles.values()].filter(a => a.isPost));
    return this.graphCache;
  }
  resolve(article, reference) {
    const result = this.graph().resolve(reference, article.id);
    if (result.node) {
      result.article = this.articles.get(result.node.uri);
      if (reference.heading) result.heading = result.article.headings.find(h => key(h.id) === key(decode(reference.heading))) ||
        result.article.headings.find(h => key(h.title) === key(decode(reference.heading)));
    }
    return result;
  }
  figurePath(article, id) {
    // Components produce bundle-relative assets; path traversal is not a valid figure ID.
    if (!id || /[\\/\x00-\x1f]/.test(id) || id === '.' || id === '..') return null;
    return path.join(path.dirname(article.file), 'figures', `${id}.typ`);
  }
  diagnostics(uri) {
    const article = this.articles.get(uri); if (!article) return [];
    const result = [...article.errors];
    const tags = new Set();
    for (const tag of article.tagItems) {
      if (tags.has(tag.value)) result.push(diagnostic(article.doc, tag.start, tag.end, `标签「${tag.value}」重复。`, 'tag-duplicate', 2));
      tags.add(tag.value);
    }
    for (const ref of article.references) {
      const target = this.resolve(article, ref);
      const messages = { missing: `找不到文章「${ref.target}」。`, ambiguous: `「${ref.target}」匹配多篇文章，请使用内容路径区分。`, 'heading-missing': `文章中找不到章节「${ref.heading}」。` };
      if (target.status !== 'resolved') result.push(diagnostic(article.doc, ref.start, ref.end, messages[target.status], `link-${target.status}`, 2));
      else if (!article.draft && target.article.draft) result.push(diagnostic(article.doc, ref.start, ref.end, '目标文章仍是草稿，公开网站不会包含此链接目标。', 'link-draft', 2));
    }
    for (const figure of article.figures) {
      const file = this.figurePath(article, figure.id);
      if (!file || !fs.existsSync(file)) result.push(diagnostic(article.doc, figure.start, figure.end, `找不到图形源文件 figures/${figure.id}.typ。`, 'figure-missing', 2));
    }
    return result;
  }
  at(uri, position) {
    const article = this.articles.get(uri); if (!article) return {};
    const offset = article.doc.offsetAt(position);
    const reference = article.references.find(ref => offset >= ref.start && offset < ref.end);
    const figure = article.figures.find(fig => offset >= fig.start && offset <= fig.end);
    const tag = article.tagItems.find(item => offset >= item.start && offset < item.end);
    return { article, offset, reference, figure, tag, target: reference ? this.resolve(article, reference) : undefined };
  }
  definition(uri, position) {
    const { article, target, figure } = this.at(uri, position);
    if (target?.status === 'resolved') return { uri: target.article.uri, range: target.heading?.range || target.article.titleRange };
    if (figure) {
      const file = this.figurePath(article, figure.id);
      if (file && fs.existsSync(file)) return { uri: URI.file(file).toString(), range: range(article.doc, 0) };
    }
    return null;
  }
  hover(uri, position) {
    const { article, target, reference, figure, tag } = this.at(uri, position);
    if (tag) {
      const uses = [...this.articles.values()].filter(a => a.tags.includes(tag.value));
      return { range: range(article.doc, tag.start, tag.end), contents: { kind: 'plaintext', value:
        `标签：${tag.value}${typeof this.tagLabels[tag.value] === 'string' ? `（${this.tagLabels[tag.value]}）` : ''}\n${uses.length} 篇文章使用\n${uses.slice(0, 12).map(a => a.title).join('\n')}` } };
    }
    if (target?.status === 'resolved') {
      const a = target.article;
      return { range: range(article.doc, reference.start, reference.end), contents: { kind: 'markdown', value:
        `**${escapeMarkdown(a.title)}**${a.draft ? ' · 草稿' : ''}\n\n${escapeMarkdown(a.metadata.description || '')}\n\n${escapeMarkdown(a.id)}${target.heading ? `\n\n章节：${escapeMarkdown(target.heading.title)}` : ''}` } };
    }
    if (figure) return { contents: { kind: 'plaintext', value: `Typst 图形：figures/${figure.id}.typ\n构建后生成 ${figure.id}.svg` } };
    return null;
  }
  references(uri, position, includeDeclaration = false) {
    const context = this.at(uri, position);
    if (!context.article) return [];
    if (context.tag) return [...this.articles.values()].flatMap(a => a.tagItems.filter(tag => tag.value === context.tag.value)
      .map(tag => ({ uri: a.uri, range: range(a.doc, tag.start, tag.end) })));
    let target = context.target?.status === 'resolved' ? context.target.article : undefined;
    let heading = context.target?.heading;
    if (!context.reference && !context.figure) {
      const p = position;
      const h = context.article.headings.find(h => p.line >= h.range.start.line && p.line <= h.range.end.line);
      if (h || (p.line >= context.article.titleRange.start.line && p.line <= context.article.titleRange.end.line)) {
        target = context.article; heading = h;
      }
    }
    if (!target) return [];
    const found = [];
    for (const a of this.articles.values()) for (const ref of a.references) {
      const resolved = this.resolve(a, ref);
      if (resolved.status === 'resolved' && resolved.article.uri === target.uri && (!heading || resolved.heading === heading)) found.push({ uri: a.uri, range: range(a.doc, ref.start, ref.end) });
    }
    if (includeDeclaration) found.unshift({ uri: target.uri, range: heading?.range || target.titleRange });
    return found;
  }
  symbols(uri) {
    const a = this.articles.get(uri); if (!a) return [];
    return a.headings.map(h => ({ name: h.title, kind: 15, range: h.range, selectionRange: h.range }));
  }
  completion(uri, position) {
    const a = this.articles.get(uri); if (!a) return [];
    const offset = a.doc.offsetAt(position);
    const lineStart = a.doc.offsetAt({ line: position.line, character: 0 });
    const before = a.text.slice(lineStart, offset);
    if (offset > 3 && offset <= a.fmEnd) {
      const prior = a.text.slice(0, lineStart);
      const tables = [...prior.matchAll(/^\s*\[([^\]\n]+)\]\s*$/gm)];
      const table = tables.at(-1)?.[1] || '';
      const fmPrefix = a.text.slice(0, offset);
      const tagArray = /\btags\s*=\s*\[([^\]]*)$/.exec(fmPrefix);
      if (table === 'taxonomies' && tagArray) {
        const tokens = [...tagArray[1].matchAll(/"(?:[^"\\]|\\.)*"|'[^']*'|"(?:[^"\\]|\\.)*$|'[^']*$/g)];
        const last = tokens.at(-1);
        const open = last && (last[0].length === 1 || last[0].at(-1) !== last[0][0]);
        if (open || !last && !tagArray[1].trim() || /,\s*$/.test(tagArray[1])) {
          const used = new Set(tokens.slice(0, open ? -1 : undefined).flatMap(item => { try { return [parseToml(`v=${item[0]}`).v]; } catch { return []; } }));
          const tags = new Set([...Object.keys(this.tagLabels), ...[...this.articles.values()].flatMap(article => article.tags)]);
          const start = open ? offset - last[0].length + 1 : offset;
          const quote = open ? last[0][0] : '"';
          const tail = a.text.slice(offset).match(quote === '"' ? /^[^"\]\r\n]*/ : /^[^'\]\r\n]*/)?.[0] || '';
          const hasClosing = a.text[offset + tail.length] === quote;
          return [...tags].filter(tag => !used.has(tag)).map(tag => ({ label: tag, kind: 12,
            detail: `${this.tagLabels[tag] || tag} · ${[...this.articles.values()].filter(a => a.tags.includes(tag)).length} 篇文章`,
            filterText: `${tag} ${this.tagLabels[tag] || ''}`,
            textEdit: { range: range(a.doc, start, open ? offset + tail.length : offset),
              newText: open ? (quote === '"' ? JSON.stringify(tag).slice(1, -1) : tag) + (hasClosing ? '' : quote) : JSON.stringify(tag) } }));
        }
      }
      if (!/^\s*[\w.]*$/.test(before)) return [];
      const fields = table === 'extra' ? { author: '"${1:Researcher}"', aliases: '["${1:简称}"]' } : table === 'taxonomies' ? { tags: '["${1:主题}"]' } : table === '' ? {
        title: '"${1:文章标题}"', date: new Date().toISOString().slice(0, 10), description: '"${1:一句话摘要}"', draft: '${1|true,false|}', slug: '"${1:slug}"', template: '"${1:page.html}"',
      } : {};
      const wordStart = offset - (before.match(/[\w.]*$/)?.[0].length || 0);
      const suffix = a.text.slice(offset, a.doc.offsetAt({ line: position.line + 1, character: 0 }));
      return Object.entries(fields).map(([name, value]) => ({ label: name, kind: 10, insertTextFormat: 2,
        textEdit: { range: range(a.doc, wordStart, offset), newText: /^\s*=/.test(suffix) ? name : `${name} = ${value}` } }));
    }
    const active = a.visible.slice(lineStart, offset);
    const link = /\[\[([^\]\n|]*)$/.exec(active);
    if (link) {
      const start = offset - link[1].length;
      const hash = link[1].indexOf('#');
      const remainder = a.text.slice(offset).match(/^[^\]\r\n|]*/)?.[0] || '';
      const replacement = range(a.doc, hash < 0 ? start : start + hash + 1, offset + remainder.length);
      const closing = /^[|\]]/.test(a.text.slice(offset + remainder.length)) ? '' : ']]';
      if (hash >= 0) {
        const destination = link[1].slice(0, hash).trim();
        const target = destination ? this.resolve(a, { target: destination, heading: '' }).article : a;
        if (!target) return [];
        return target.headings.map(h => ({ label: h.title, kind: 15, detail: target.title,
          textEdit: { range: replacement, newText: (h.explicit ? h.id : h.title) + closing } }));
      }
      return this.graph().nodes.flatMap(node => {
        const resolved = this.resolve(a, { target: node.title, heading: '' });
        const title = resolved.status === 'resolved' && resolved.article.uri === node.uri && !/[#|\[\]\n]/.test(node.title) ? node.title : node.id;
        const options = [title, ...node.aliases.filter(alias => this.resolve(a, { target: alias, heading: '' }).article?.uri === node.uri && !/[#|\[\]\n]/.test(alias))];
        return [...new Set(options)].map(label => ({ label, kind: 18, detail: `${node.title} · ${node.id}${node.draft ? ' · 草稿' : ''}`,
          filterText: `${label} ${node.slug} ${node.id}`, textEdit: { range: replacement, newText: label + closing } }));
      });
    }
    const figure = /\{\{\s*<fig\b[^\n]*\bid\s*=\s*"([^"\n]*)$/.exec(active);
    if (figure) {
      let entries = [];
      try { entries = fs.readdirSync(path.join(path.dirname(a.file), 'figures'), { withFileTypes: true }); } catch { return []; }
      return entries.filter(entry => entry.isFile() && entry.name.endsWith('.typ')).map(entry => ({ label: entry.name.slice(0, -4), kind: 17,
        textEdit: { range: range(a.doc, offset - figure[1].length, offset + (a.text.slice(offset).match(/^[^"\r\n]*/)?.[0].length || 0)), newText: entry.name.slice(0, -4) } }));
    }
    return [];
  }
}
module.exports = { BlogWorkspace, parseArticle };
