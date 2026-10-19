'use strict';
const fs = require('node:fs');
const path = require('node:path');

// This receives the offset-preserving visible Markdown from the existing parser.
// Code, mathematics, comments and frontmatter have already been masked there.
function extractCitations(visible) {
  const citations = [];
  for (const cluster of visible.matchAll(/\[([ \t]*@[^\]\r\n]+)\]/gu)) {
    let preceding = cluster.index - 1;
    while (preceding >= 0 && visible[preceding] === '\\') preceding -= 1;
    if ((cluster.index - preceding - 1) % 2 || visible[cluster.index - 1] === '[' || visible[cluster.index - 1] === '!') continue;
    if (['(', '['].includes(visible[cluster.index + cluster[0].length])) continue;
    const parts = cluster[1].split(';');
    const matches = parts.map(part => /^[ \t]*@([^\s@;,\[\]{}<>"\\]+)[ \t]*(?:,[ \t]*(.*))?[ \t]*$/u.exec(part));
    // A malformed group is prose, rather than a collection of independently
    // salvageable citations. The blogctl publisher and the Rust IR do the same.
    if (matches.some(match => !match)) continue;
    let offset = cluster.index + 1;
    for (let i = 0; i < parts.length; i++) {
      const match = matches[i];
      const start = offset + parts[i].indexOf(match[1]);
      citations.push({ key: match[1], locator: match[2]?.trim() || '', start, end: start + match[1].length, clusterStart: cluster.index, clusterEnd: cluster.index + cluster[0].length });
      offset += parts[i].length + 1;
    }
  }
  return citations;
}

// Index string-valued properties of each object in the root items array. JSON
// has already been parsed, so this offset-only token walk need not interpret
// numbers or booleans. Keeping object/array scopes prevents a title or nested
// property with the same text from becoming an F12 declaration accidentally.
function referenceLocations(text) {
  const stack = [];
  let items;
  for (const token of text.matchAll(/"(?:[^"\\]|\\.)*"|[{}\[\]:,]/g)) {
    const value = token[0];
    const parent = stack.at(-1);
    if (value === '{' || value === '[') {
      const frame = { type: value, key: null, expectKey: value === '{', expectValue: false, index: 0, strings: new Map() };
      if (value === '[' && stack.length === 1 && parent.type === '{' && parent.key === 'items' && parent.expectValue) {
        frame.items = new Map(); items = frame.items;
      }
      if (value === '{' && parent?.items) parent.items.set(parent.index, frame.strings);
      if (parent) parent.expectValue = false;
      stack.push(frame);
    } else if (value === '}' || value === ']') {
      stack.pop();
    } else if (value === ':') {
      if (parent) parent.expectValue = true;
    } else if (value === ',') {
      if (parent?.type === '{') { parent.expectKey = true; parent.expectValue = false; }
      else if (parent) parent.index += 1;
    } else if (parent?.type === '{') {
      if (parent.expectKey) {
        parent.key = JSON.parse(value); parent.expectKey = false;
      } else if (parent.expectValue) {
        parent.strings.set(parent.key, { value: JSON.parse(value), start: token.index, end: token.index + value.length });
        parent.expectValue = false;
      }
    }
  }
  return items || new Map();
}

function referenceRange(text, declaration) {
  const prior = text.slice(0, declaration.start);
  const line = prior.split('\n').length - 1;
  const character = prior.length - prior.lastIndexOf('\n') - 1;
  return { start: { line, character }, end: { line, character: character + declaration.end - declaration.start } };
}

function normalizedAuthors(value) {
  if (!Array.isArray(value)) return [];
  return value.flatMap(author => {
    if (!author || typeof author !== 'object' || Array.isArray(author)) return [];
    if (typeof author.literal === 'string' && author.literal.trim()) return [{ literal: author.literal }];
    const name = {};
    for (const property of ['family', 'given']) if (typeof author[property] === 'string' && author[property].trim()) name[property] = author[property];
    return Object.keys(name).length ? [name] : [];
  });
}

function readReferences(contentRoot) {
  const file = path.join(path.dirname(contentRoot), 'data', 'references.json');
  const references = new Map();
  try {
    const text = fs.readFileSync(file, 'utf8');
    const data = JSON.parse(text.replace(/^\uFEFF/, ''));
    if (!data || typeof data !== 'object' || Array.isArray(data) || data.version !== 1 || !Array.isArray(data.items)) return references;
    const locations = referenceLocations(text);
    for (const [index, item] of data.items.entries()) {
      if (!item || typeof item !== 'object' || Array.isArray(item) || typeof item.title !== 'string') continue;
      const field = typeof item.citekey === 'string' && item.citekey ? 'citekey' : 'id';
      const citekey = item[field];
      if (typeof citekey !== 'string' || !/^[\p{L}\p{N}_:.+\/-]+$/u.test(citekey)) continue;
      const declaration = locations.get(index)?.get(field);
      if (!declaration || declaration.value !== citekey) continue;
      const entry = { title: item.title, citekey, file, location: referenceRange(text, declaration), author: normalizedAuthors(item.author) };
      for (const field of ['id', 'type', 'container-title', 'DOI', 'URL', 'volume', 'issue', 'page', 'publisher']) {
        if (typeof item[field] === 'string') entry[field] = item[field];
      }
      const year = item.issued?.['date-parts']?.[0]?.[0];
      if (Number.isInteger(year)) entry.issued = { 'date-parts': [[year]] };
      references.set(citekey, entry);
    }
  } catch (error) {
    if (error.code !== 'ENOENT' && !(error instanceof SyntaxError)) throw error;
  }
  return references;
}

function bibliographyKeys(articleFile) {
  const file = path.join(path.dirname(articleFile), 'refs.bib');
  const references = new Map();
  try {
    const text = fs.readFileSync(file, 'utf8');
    // Only index declarations here. The build pipeline is the BibTeX parser.
    for (const match of text.matchAll(/^\s*@(?!comment\b|string\b|preamble\b)[\w-]+\s*[{(]\s*([^,\s]+)\s*,/gmi)) {
      const at = match.index + match[0].indexOf(match[1]);
      const prior = text.slice(0, at);
      const line = prior.split('\n').length - 1;
      const character = prior.length - prior.lastIndexOf('\n') - 1;
      references.set(match[1], { id: match[1], citekey: match[1], title: match[1], file, location: { start: { line, character }, end: { line, character: character + match[1].length } } });
    }
  } catch (error) { if (error.code !== 'ENOENT') throw error; }
  return references;
}
function authorText(item) {
  return normalizedAuthors(item?.author).map(author => author.literal || [author.family, author.given].filter(Boolean).join(', ')).join('; ');
}
function yearText(item) { const year = item?.issued?.['date-parts']?.[0]?.[0]; return Number.isInteger(year) ? year : ''; }
module.exports = { extractCitations, readReferences, bibliographyKeys, authorText, yearText };
