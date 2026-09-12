/* Shared, DOM-free wikilink resolution and article graph. */
(() => {
  'use strict';

  const asText = value => typeof value === 'string' ? value : '';
  const decode = value => {
    try { return decodeURIComponent(value); } catch (_) { return value; }
  };
  const nameKey = value => asText(value).trim().normalize('NFC').toLocaleLowerCase();
  const pathKey = value => nameKey(decode(asText(value))).replace(/\\/g, '/')
    .replace(/^@\//, '').replace(/^\.\//, '').replace(/^\/+|\/+$/g, '');

  function parseWikiLinks(text) {
    if (typeof text !== 'string') return [];
    const references = [];
    let cursor = 0;
    while (cursor < text.length) {
      const start = text.indexOf('[[', cursor);
      if (start < 0) break;
      const close = text.indexOf(']]', start + 2);
      if (close < 0) break;
      cursor = close + 2;
      let backslashes = 0;
      for (let i = start - 1; i >= 0 && text[i] === '\\'; i--) backslashes++;
      const inner = text.slice(start + 2, close);
      if (backslashes % 2 || /[\[\]\r\n]/.test(inner) || text[close + 2] === ']') continue;
      const separator = inner.indexOf('|');
      const destination = (separator < 0 ? inner : inner.slice(0, separator)).trim();
      const alias = separator < 0 ? null : inner.slice(separator + 1).trim();
      if (alias === '' || alias?.includes('|')) continue;
      const hash = destination.indexOf('#');
      const target = (hash < 0 ? destination : destination.slice(0, hash)).trim();
      const heading = hash < 0 ? '' : destination.slice(hash + 1).trim();
      if ((!target && !heading) || (hash >= 0 && !heading)) continue;
      references.push({
        start, end: cursor, raw: text.slice(start, cursor), target, heading,
        label: alias ?? (target ? destination : heading),
      });
    }
    return references;
  }

  function safeURL(value, base) {
    try {
      const url = new URL(value, base);
      if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) return null;
      return url;
    } catch (_) { return null; }
  }

  function canonicalURL(url) {
    // Decode segments separately: an encoded slash must not become a path separator.
    const path = url.pathname.split('/').map(part => encodeURIComponent(decode(part).normalize('NFC')))
      .join('/').replace(/\/index\.html$/i, '/').replace(/\/+$/, '') || '/';
    return url.origin + path;
  }

  function flattenHeadings(headings) {
    const result = [];
    for (const heading of Array.isArray(headings) ? headings : []) {
      if (!heading || typeof heading !== 'object') continue;
      result.push(heading);
      result.push(...flattenHeadings(heading.children));
    }
    return result;
  }

  function createKnowledgeGraph(posts, options = {}) {
    const input = Array.isArray(posts) ? posts : [];
    const browserBase = typeof location === 'object' ? location.href : undefined;
    const firstAbsolute = input.map(post => safeURL(post?.url)).find(Boolean);
    const base = safeURL(options.baseURL || browserBase) || firstAbsolute;
    const nodes = [], byId = new Map(), byURL = new Map(), paths = new Map(), names = new Map();
    const edges = [], unresolved = [], edgeMap = new Map();
    const incomingEdges = new Map(), outgoingEdges = new Map();

    function add(index, key, node) {
      if (!key) return;
      if (!index.has(key)) index.set(key, new Set());
      index.get(key).add(node);
    }

    for (const post of input) {
      if (!post || typeof post !== 'object' || typeof post.url !== 'string') continue;
      const url = safeURL(post.url, base?.href);
      if (!url || (base && url.origin !== base.origin)) continue;
      url.hash = ''; url.search = '';
      const id = asText(post.id) || canonicalURL(url);
      if (byId.has(id)) continue;
      const node = {
        ...post, id, url: url.href, title: asText(post.title) || id,
        tags: Array.isArray(post.tags) ? post.tags.filter(tag => typeof tag === 'string') : [],
        aliases: Array.isArray(post.aliases) ? post.aliases.filter(alias => typeof alias === 'string') : [],
        headings: flattenHeadings(post.headings),
      };
      nodes.push(node);
      byId.set(id, node);
      add(byURL, canonicalURL(url), node);
      for (const path of [id, post.path, post.slug, url.pathname]) {
        const key = pathKey(path);
        add(paths, key, node);
        if (key.endsWith('.md')) add(paths, key.slice(0, -3), node);
        const folder = key.replace(/\/(?:index|_index)\.md$/, '').replace(/\/index\.html$/, '');
        add(paths, folder, node);
        if (folder) add(paths, folder.split('/').at(-1), node);
      }
      add(names, nameKey(node.title), node);
      for (const alias of node.aliases) add(names, nameKey(alias), node);
      incomingEdges.set(id, []); outgoingEdges.set(id, []);
    }

    function referenceValue(reference) {
      if (typeof reference === 'string') {
        const parsed = parseWikiLinks(`[[${reference}]]`);
        return parsed.length === 1 ? parsed[0] : { target: reference, heading: '', label: reference };
      }
      const ref = reference && typeof reference === 'object' ? reference : {};
      let target = asText(ref.target).trim(), heading = asText(ref.heading).trim();
      const hash = target.indexOf('#');
      if (hash >= 0) {
        if (!heading) heading = target.slice(hash + 1).trim();
        target = target.slice(0, hash).trim();
      }
      return { ...ref, target, heading, label: asText(ref.label) || target || heading };
    }

    function resolve(reference, sourceId) {
      const ref = referenceValue(reference);
      let matches;
      if (!ref.target) {
        if (ref.heading && byId.has(sourceId)) matches = new Set([byId.get(sourceId)]);
      } else if (/^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(ref.target)) {
        const targetURL = safeURL(ref.target, base?.href);
        if (targetURL && (!base || targetURL.origin === base.origin)) matches = byURL.get(canonicalURL(targetURL));
      } else {
        // Explicit relative paths resolve from the current Markdown file's directory.
        if (/^\.\.?[\/\\]/.test(ref.target) && byId.has(sourceId)) {
          const source = asText(byId.get(sourceId).id).replace(/\\/g, '/');
          const parts = source.split('/').slice(0, -1);
          for (const part of ref.target.replace(/\\/g, '/').split('/')) {
            if (part === '..') parts.pop();
            else if (part && part !== '.') parts.push(part);
          }
          matches = paths.get(pathKey(parts.join('/')));
        }
        matches ||= paths.get(pathKey(ref.target));
        matches ||= names.get(nameKey(ref.target));
      }
      if (!matches?.size) return { status: 'missing', label: ref.label };
      if (matches.size !== 1) return { status: 'ambiguous', label: ref.label };
      const node = matches.values().next().value;
      let url = node.url;
      if (ref.heading) {
        const needle = nameKey(decode(ref.heading));
        const heading = node.headings.find(item => nameKey(decode(asText(item.id))) === needle) ||
          node.headings.find(item => nameKey(asText(item.title)) === needle);
        if (!heading) return { status: 'heading-missing', node, label: ref.label };
        const permalink = safeURL(heading.permalink, node.url);
        const fragment = asText(heading.id) ||
          (permalink && canonicalURL(permalink) === canonicalURL(new URL(node.url)) ? decode(permalink.hash.slice(1)) : '');
        if (!fragment) return { status: 'heading-missing', node, label: ref.label };
        url += `#${encodeURIComponent(decode(fragment))}`;
      }
      return { status: 'resolved', node, url, label: ref.label || node.title };
    }

    function addEdge(source, target, reference) {
      if (source.id === target.id) return;
      const key = JSON.stringify([source.id, target.id]);
      let edge = edgeMap.get(key);
      if (!edge) {
        edge = { source: source.id, target: target.id, count: 0, references: [] };
        edgeMap.set(key, edge); edges.push(edge);
        outgoingEdges.get(source.id).push(edge); incomingEdges.get(target.id).push(edge);
      }
      edge.count++;
      edge.references.push(reference);
    }

    for (const node of nodes) {
      for (const reference of Array.isArray(node.references) ? node.references : []) {
        const result = resolve(reference, node.id);
        if (result.status !== 'resolved') unresolved.push({ source: node.id, reference, reason: result.status });
        else addEdge(node, result.node, { ...referenceValue(reference), url: result.url, label: result.label, kind: 'wiki' });
      }
      for (const link of Array.isArray(node.links) ? node.links : []) {
        const url = safeURL(asText(link), node.url);
        if (!url || url.origin !== new URL(node.url).origin) continue;
        const matches = byURL.get(canonicalURL(url));
        if (matches?.size === 1) {
          const target = matches.values().next().value;
          addEdge(node, target, { target: target.id, url: url.href, label: target.title, kind: 'markdown' });
        }
      }
    }

    return {
      nodes, edges, unresolved, resolve,
      outgoing: id => (outgoingEdges.get(id) || []).slice(),
      incoming: id => (incomingEdges.get(id) || []).slice(),
    };
  }

  const api = { parseWikiLinks, createKnowledgeGraph };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  if (typeof window !== 'undefined') window.BlogKnowledge = api;
})();
