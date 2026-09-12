/* Progressive topic navigation. Canonical taxonomy URLs remain available without JS. */
(() => {
  'use strict';
  function buildTopics(groups, definitions = []) {
    const terms = new Map(groups.map(group => [group.id, group]));
    const assigned = new Set(), usedIds = new Set();
    function branch(definition, path) {
      if (!definition || typeof definition !== 'object') return null;
      const own = terms.get(definition.tag);
      const children = (Array.isArray(definition.children) ? definition.children : [])
        .map((child, i) => branch(child, `${path}-${i}`)).filter(Boolean);
      if (!own && !children.length) return null;
      if (own) assigned.add(own.id);
      const posts = new Map((own?.posts || []).map(post => [post.url, post]));
      children.forEach(child => child.posts.forEach(post => posts.set(post.url, post)));
      let id = String(definition.id || `topic-${path}`);
      if (usedIds.has(id)) id = `topic-${path}`;
      while (usedIds.has(id)) id += '-';
      usedIds.add(id);
      return { id, label: String(definition.label || own?.label || definition.tag || ''),
        description: own?.description || '', url: own?.url || '', children,
        posts: sortPosts([...posts.values()]) };
    }
    const tree = (Array.isArray(definitions) ? definitions : []).map((def, i) => branch(def, String(i))).filter(Boolean);
    groups.filter(group => !assigned.has(group.id)).forEach((group, i) => {
      const leaf = branch({ tag: group.id }, `extra-${i}`);
      if (leaf) tree.push(leaf);
    });
    const posts = new Map();
    groups.forEach(group => group.posts.forEach(post => posts.set(post.url, post)));
    return { tree, posts: sortPosts([...posts.values()]) };
  }
  function sortPosts(posts) {
    return posts.sort((a, b) => String(b.date).localeCompare(String(a.date)) || a.title.localeCompare(b.title, 'zh'));
  }
  function findTopic(tree, id) {
    for (const node of tree) {
      if (node.id === id) return node;
      const nested = findTopic(node.children, id);
      if (nested) return nested;
    }
    return null;
  }
  if (typeof module !== 'undefined') module.exports = { buildTopics, findTopic };
  if (typeof document === 'undefined') return;
  const states = new Map();
  let serial = 0;
  function element(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text != null) node.textContent = text;
    return node;
  }
  function init() {
    const host = document.querySelector('[data-topics]');
    if (!host || host.dataset.enhanced) return;
    const groups = [...host.querySelectorAll('[data-topic-id]')].map(group => ({
      id: group.dataset.topicId, label: group.dataset.topicLabel, description: group.dataset.topicDescription,
      url: group.dataset.topicUrl,
      posts: [...group.querySelectorAll('[data-topic-note]')].map(note => ({
        url: note.querySelector('[data-note-link]').href,
        title: note.querySelector('[data-note-link]').textContent,
        description: note.querySelector('[data-note-description]').textContent,
        date: note.querySelector('time').dateTime, dateLabel: note.querySelector('time').textContent,
      })),
    }));
    if (!groups.length) return;
    let definitions;
    try { definitions = JSON.parse(host.dataset.topicTree || '[]'); } catch (_) { definitions = []; }
    const model = buildTopics(groups, definitions), uid = `topics-${++serial}`;
    const saved = states.get(location.pathname) || {};
    let selectedId = findTopic(model.tree, saved.selectedId)?.id || '';
    const closed = new Set(saved.closed || []), buttons = new Map();
    let treeOpen = saved.treeOpen !== false, animation, branchSerial = 0;
    const workspace = element('div', 'topics-workspace');
    const sidebar = element('aside', 'topics-sidebar');
    const toggleTree = element('button', 'topics-tree-toggle'); toggleTree.type = 'button';
    toggleTree.setAttribute('aria-controls', `${uid}-tree`);
    const treeFrame = element('div', 'topics-tree-frame'); treeFrame.id = `${uid}-tree`;
    const treeNav = element('nav', 'topics-tree'); treeNav.setAttribute('aria-label', '主题目录');
    const panel = element('section', 'topics-panel'); panel.id = `${uid}-panel`;
    const panelHeader = element('div', 'topics-panel-header');
    const heading = element('h2'), count = element('span', 'topics-count');
    heading.id = `${uid}-heading`; panel.setAttribute('aria-labelledby', heading.id);
    const description = element('p', 'topics-selection-description');
    const list = element('ol', 'topic-note-list');
    const live = element('p', 'sr-only'); live.setAttribute('role', 'status'); live.setAttribute('aria-live', 'polite');
    panelHeader.append(heading, count); panel.append(panelHeader, description, list, live);
    treeFrame.append(treeNav); sidebar.append(toggleTree, treeFrame); workspace.append(sidebar, panel);
    function save() { states.set(location.pathname, { selectedId, closed: [...closed], treeOpen }); }
    function expand(button, frame, open) {
      button.setAttribute('aria-expanded', String(open));
      frame.classList.toggle('is-collapsed', !open); frame.inert = !open;
      frame.setAttribute('aria-hidden', String(!open));
    }
    function makeTree(nodes) {
      const ul = element('ul');
      for (const node of nodes) {
        const li = element('li'), row = element('div', 'topic-tree-row');
        if (node.children.length) {
          const disclosure = element('button', 'topic-disclosure', '›'); disclosure.type = 'button';
          disclosure.setAttribute('aria-label', `展开或收起${node.label}`);
          const frame = element('div', 'topic-children'); frame.id = `${uid}-branch-${++branchSerial}`;
          frame.append(makeTree(node.children));
          disclosure.setAttribute('aria-controls', frame.id);
          expand(disclosure, frame, !closed.has(node.id));
          disclosure.addEventListener('click', () => {
            const open = disclosure.getAttribute('aria-expanded') !== 'true';
            if (open) closed.delete(node.id); else closed.add(node.id);
            expand(disclosure, frame, open); save();
          });
          row.append(disclosure); li.append(row, frame);
        } else { row.append(element('span', 'topic-tree-dot', '·')); li.append(row); }
        const pick = element('button', 'topic-choice'); pick.type = 'button';
        const label = element('span', '', node.label), total = element('span', 'topic-tree-count', String(node.posts.length));
        pick.append(label, total); pick.addEventListener('click', () => { selectedId = node.id; update(true); });
        pick.setAttribute('aria-pressed', 'false'); buttons.set(node.id, pick); row.append(pick); ul.append(li);
      }
      return ul;
    }
    treeNav.append(makeTree([{ id: '', label: '全部', posts: model.posts, children: [] }, ...model.tree]));
    function update(animate) {
      const selected = findTopic(model.tree, selectedId), posts = selected?.posts || model.posts;
      heading.textContent = selected?.label || '全部手记'; count.textContent = `${posts.length} 篇`;
      description.textContent = selected?.description || ''; description.hidden = !description.textContent;
      buttons.forEach((button, id) => button.setAttribute('aria-pressed', String(id === selectedId)));
      list.replaceChildren();
      for (const post of posts) {
        const item = element('li', 'topic-note'), time = element('time', '', post.dateLabel);
        time.dateTime = post.date; const title = element('h3'), link = element('a', '', post.title); link.href = post.url;
        title.append(link); item.append(time, title);
        if (post.description) item.append(element('p', '', post.description));
        list.append(item);
      }
      live.textContent = `${heading.textContent}，${posts.length} 篇文章`;
      animation?.cancel();
      if (animate && !matchMedia('(prefers-reduced-motion: reduce)').matches && typeof list.animate === 'function') {
        animation = list.animate([{ opacity: 0, transform: 'translateY(8px)' }, { opacity: 1, transform: 'translateY(0)' }], { duration: 280, easing: 'cubic-bezier(.2,.7,.2,1)' });
      }
      save();
    }
    function updateTree() {
      toggleTree.textContent = treeOpen ? '收起分类 −' : '展开分类 +';
      expand(toggleTree, treeFrame, treeOpen); save();
    }
    toggleTree.addEventListener('click', () => { treeOpen = !treeOpen; updateTree(); });
    host.dataset.enhanced = 'true'; host.replaceChildren(workspace); updateTree(); update(false);
  }
  document.addEventListener('DOMContentLoaded', init);
  document.addEventListener('blog:preview-updated', init);
})();
