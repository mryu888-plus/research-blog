/* Resolve wiki links from the same published article index used by the graph. */
(() => {
  'use strict';
  const cache = new Map();
  let revision = 0;
  let controller;
  let graphCleanup;
  let graphState;
  const excluded = 'a,code,pre,script,style,textarea,math,.katex,.math-source';

  function references(text) {
    const math = Array.from(text.matchAll(/(?<!\\)\$\$[\s\S]*?\$\$|(?<!\\)\$[^\n$]*?\$/g));
    return BlogKnowledge.parseWikiLinks(text).filter(ref => !math.some(match => ref.start >= match.index && ref.start < match.index + match[0].length));
  }

  function textNodes(root) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        return node.parentElement?.closest(excluded) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT;
      },
    });
    const nodes = [];
    while (walker.nextNode()) if (walker.currentNode.textContent.includes('[[')) nodes.push(walker.currentNode);
    return nodes;
  }

  function extract(post) {
    const old = cache.get(post.id);
    if (old?.html === post.html && old.url === post.url) return { ...post, ...old.extracted };
    // Template content is inert: indexed article images/scripts never execute.
    const template = document.createElement('template');
    template.innerHTML = post.html;
    const refs = textNodes(template.content).flatMap(node => references(node.textContent));
    const links = Array.from(template.content.querySelectorAll('a[href]')).filter(node => !node.closest('pre,code')).flatMap(node => {
      try { return [new URL(node.getAttribute('href'), post.url).href]; } catch (_) { return []; }
    });
    const extracted = { references: refs, links };
    cache.set(post.id, { html: post.html, url: post.url, extracted });
    return { ...post, ...extracted };
  }

  function applyWikiLinks(model, noteId) {
    const content = document.querySelector('.article-content');
    if (!content) return;
    // Re-resolve existing links when another article is renamed or created.
    content.querySelectorAll('[data-wiki-raw]').forEach(node => node.replaceWith(document.createTextNode(node.dataset.wikiRaw)));
    for (const node of textNodes(content)) {
      const text = node.textContent;
      const refs = references(text);
      if (!refs.length) continue;
      const fragment = document.createDocumentFragment();
      let cursor = 0;
      for (const ref of refs) {
        fragment.append(document.createTextNode(text.slice(cursor, ref.start)));
        const resolved = model.resolve(ref, noteId);
        const link = document.createElement(resolved.status === 'resolved' ? 'a' : 'span');
        link.className = `wiki-link wiki-${resolved.status}`;
        link.dataset.wikiRaw = ref.raw;
        link.textContent = ref.label || ref.target || ref.heading;
        if (resolved.status === 'resolved') {
          link.href = resolved.url;
          link.title = resolved.node.title;
        } else {
          link.tabIndex = 0;
          link.title = resolved.status === 'ambiguous' ? '有多篇同名文章，请使用文章目录名。'
            : resolved.status === 'heading-missing' ? '找不到这个章节，请检查章节标题。' : '文章尚未创建或发布，请检查标题。';
          link.setAttribute('aria-label', `${link.textContent}：${link.title}`);
        }
        fragment.append(link);
        cursor = ref.end;
      }
      fragment.append(document.createTextNode(text.slice(cursor)));
      node.replaceWith(fragment);
    }
  }

  function renderConnections(model, noteId) {
    const panel = document.getElementById('article-connections');
    if (!panel) return;
    const byId = new Map(model.nodes.map(node => [node.id, node]));
    for (const [kind, edges] of [['outgoing', model.outgoing(noteId)], ['incoming', model.incoming(noteId)]]) {
      const related = edges.map(edge => byId.get(kind === 'outgoing' ? edge.target : edge.source)).filter(Boolean);
      const list = panel.querySelector(`[data-${kind}]`);
      list.replaceChildren();
      panel.querySelector(`[data-${kind}-count]`).textContent = String(related.length);
      for (const node of related) {
        const item = document.createElement('li');
        const link = document.createElement('a');
        link.href = node.url;
        link.textContent = node.title;
        item.append(link);
        if (node.draft) { const draft = document.createElement('small'); draft.textContent = '草稿'; item.append(draft); }
        list.append(item);
      }
      if (!related.length) {
        const item = document.createElement('li');
        item.className = 'connections-empty';
        item.textContent = kind === 'outgoing' ? '还没有引用其他文章。' : '还没有文章链接到这里。';
        list.append(item);
      }
    }
    const missing = model.unresolved.filter(item => item.source === noteId);
    const status = panel.querySelector('[data-knowledge-status]');
    status.textContent = missing.length ? `${missing.length} 处双链尚未匹配，请检查正文中的虚线标记。` : '';
    status.hidden = !missing.length;
  }

  async function refresh() {
    const version = ++revision;
    controller?.abort();
    graphState = graphCleanup?.state?.() || graphState;
    graphCleanup?.();
    graphCleanup = null;
    const graph = document.getElementById('knowledge-graph');
    const noteId = document.querySelector('[data-note-id]')?.dataset.noteId;
    if (!graph && !noteId) return;
    controller = new AbortController();
    const signal = controller.signal;
    for (let attempt = 0; attempt < 7; attempt++) {
      try {
        const response = await fetch(document.body.dataset.graphUrl, { cache: 'no-cache', signal: AbortSignal.any([signal, AbortSignal.timeout(3000)]) });
        if (!response.ok) throw new Error('index unavailable');
        const text = await response.text();
        const posts = JSON.parse(text.split(/\r?\n<script>window\.LiveReloadOptions=/, 1)[0]);
        if (!Array.isArray(posts) || posts.some(post => typeof post.id !== 'string' || typeof post.html !== 'string')) throw new Error('invalid index');
        if (version !== revision) return;
        const liveIds = new Set(posts.map(post => post.id));
        for (const id of cache.keys()) if (!liveIds.has(id)) cache.delete(id);
        const model = BlogKnowledge.createKnowledgeGraph(posts.map(extract), { baseURL: document.body.dataset.graphUrl });
        if (noteId) { applyWikiLinks(model, noteId); renderConnections(model, noteId); }
        if (graph) graphCleanup = BlogGraph.render(graph, model, { focusId: new URL(location.href).searchParams.get('focus'), state: graphState });
        return;
      } catch (_) {
        if (signal.aborted || version !== revision) return;
        if (attempt < 6) {
          await new Promise(resolve => setTimeout(resolve, Math.min(25 * 2 ** attempt, 800)));
          if (signal.aborted || version !== revision) return;
          continue;
        }
        const status = document.querySelector('[data-knowledge-status]') || graph;
        if (status) {
          status.hidden = false;
          status.textContent = '关联数据暂时不可用。';
          const retry = document.createElement('button'); retry.textContent = '重新加载'; retry.onclick = refresh; status.append(retry);
        }
      }
    }
  }
  document.addEventListener('DOMContentLoaded', refresh);
  document.addEventListener('blog:preview-updated', refresh);
})();
