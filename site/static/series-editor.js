/* Local preview only. The editor service owns validation and atomic writes. */
(() => {
  'use strict';
  if (!['127.0.0.1', 'localhost'].includes(location.hostname)) return;
  const configUrl = document.getElementById('series-editor-script')?.dataset.configUrl;
  if (!configUrl) return;
  let configPromise;
  function element(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text != null) node.textContent = text;
    return node;
  }
  async function init() {
    const host = document.querySelector('[data-series-view]');
    if (!host || host.dataset.editorLoading) return;
    host.dataset.editorLoading = 'true';
    configPromise ||= (async () => {
      // Zola briefly clears static routes during a rebuild. Do not permanently
      // disable editing when that window overlaps the initial page request.
      for (let attempt = 0; attempt < 4; attempt++) {
        try {
          const response = await fetch(configUrl, { cache: 'no-store', signal: AbortSignal.timeout(2000) });
          if (response.ok) return await response.json();
        } catch (_) { /* Retry a transient rebuild or connection failure. */ }
        if (attempt < 3) await new Promise(resolve => setTimeout(resolve, 150 * 2 ** attempt));
      }
      return null;
    })();
    const config = await configPromise;
    if (!config) { configPromise = null; delete host.dataset.editorLoading; return; }
    if (!host.isConnected) return;
    const toolbar = element('div', 'series-edit-toolbar');
    toolbar.append(element('p', '', '拖动 ⋮⋮ 调整顺序或移到其他系列，也可使用「移动」菜单。'));
    const status = element('p', 'series-edit-status', '正在读取系列目录…');
    status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite');
    toolbar.append(status); host.before(toolbar);
    const links = new Map([...document.querySelector('#series-preview-links')?.content.querySelectorAll('[data-note-key]') || []]
      .map(link => [link.dataset.noteKey, link.href]));
    let model, busy = false, draggedKey = null;
    async function request(route, data) {
      const response = await fetch(config.endpoint + route, {
        method: data ? 'POST' : 'GET', cache: 'no-store', signal: AbortSignal.timeout(5000),
        headers: { 'X-Preview-Token': config.token, ...(data ? { 'Content-Type': 'application/json' } : {}) },
        ...(data ? { body: JSON.stringify(data) } : {}),
      });
      const body = await response.json();
      if (!response.ok) { const error = new Error(body.error || '保存失败，请重试。'); error.status = response.status; throw error; }
      return body;
    }
    async function move(key, targetId, beforeKey = null) {
      if (busy) return;
      busy = true; host.setAttribute('aria-busy', 'true');
      status.textContent = '正在保存…'; status.classList.remove('is-error');
      try {
        model = await request('/move', { key, targetId: targetId || null, beforeKey, revision: model.revision });
        render(); status.textContent = '已保存';
      } catch (error) {
        if (error.status === 409) {
          try { model = await request('/library'); render(); } catch (_) { /* Keep the last known state. */ }
          status.textContent = '目录已在其他窗口更新，请再移动一次。';
        } else { render(); status.textContent = `未保存：${error.message}`; }
        status.classList.add('is-error');
      } finally { busy = false; host.removeAttribute('aria-busy'); }
    }
    function render() {
      if (!host.isConnected) return;
      const articles = new Map(model.articles.map(article => [article.key, article]));
      const assigned = new Set(model.catalog.series.flatMap(series => [series, ...series.groups]).flatMap(item => item.articles || []));
      const destinations = [{ id: '', title: '未加入系列' }, ...model.catalog.series.flatMap(series => [
        { id: series.id, title: series.title }, ...series.groups.map(group => ({ id: group.id, title: `${series.title} / ${group.title}` })),
      ])];
      function list(keys, targetId, label) {
        const visibleKeys = keys.filter(key => articles.has(key));
        const ol = element('ol', 'series-articles series-edit-list');
        ol.dataset.dropTarget = targetId; ol.setAttribute('aria-label', label);
        if (!visibleKeys.length) ol.append(element('li', 'series-drop-empty', targetId ? '拖到这里加入文章' : '拖到这里移出系列'));
        visibleKeys.forEach((key, index) => {
          const article = articles.get(key), row = element('li', 'series-edit-article');
          row.dataset.articleKey = key;
          const handle = element('button', 'series-drag-handle', '⋮⋮'); handle.type = 'button'; handle.draggable = true;
          handle.setAttribute('aria-label', `拖动《${article.title}》`); handle.title = '拖动文章';
          const body = element('div', 'series-edit-article-body'), heading = element('h3');
          const link = element(links.has(key) ? 'a' : 'span', '', article.title);
          if (links.has(key)) link.href = links.get(key);
          heading.append(link);
          if (article.draft) heading.append(element('span', 'series-draft', '草稿'));
          const actions = element('details', 'series-article-actions'); actions.append(element('summary', '', '移动'));
          const select = element('select'); select.setAttribute('aria-label', `移动《${article.title}》到`);
          for (const destination of destinations) {
            const option = element('option', '', destination.title); option.value = destination.id; select.append(option);
          }
          select.value = targetId;
          select.addEventListener('change', () => void move(key, select.value));
          actions.append(select);
          if (targetId) {
            for (const [title, delta] of [['上移', -1], ['下移', 1]]) {
              const button = element('button', '', title); button.type = 'button';
              button.setAttribute('aria-label', `${title}《${article.title}》`);
              button.disabled = index + delta < 0 || index + delta >= visibleKeys.length;
              button.addEventListener('click', () => void move(key, targetId, delta < 0 ? visibleKeys[index - 1] : (visibleKeys[index + 2] || null)));
              actions.append(button);
            }
          }
          body.append(heading, actions); row.append(handle, body); ol.append(row);
        });
        return ol;
      }
      const sidebar = element('aside', 'series-editor-sidebar');
      const nav = element('nav', 'series-directory'); nav.setAttribute('aria-label', '系列目录');
      nav.append(element('p', 'eyebrow', '系列目录'));
      const directory = element('ol');
      for (const series of model.catalog.series) {
        const item = element('li'), link = element('a', '', series.title); link.href = `#${series.id}`; item.append(link); directory.append(item);
      }
      nav.append(directory); sidebar.append(nav);
      const unassigned = element('section', 'series-unassigned');
      unassigned.append(element('h2', '', '未加入系列'), list(model.articles.filter(article => !assigned.has(article.key)).map(article => article.key), '', '未加入系列的文章'));
      sidebar.append(unassigned);
      const collections = element('div', 'series-collections');
      model.catalog.series.forEach((series, index) => {
        const section = element('section', 'series-collection'); section.id = series.id;
        const header = element('header', 'series-heading'); header.append(element('p', 'eyebrow', `/ 系列 ${index + 1}`), element('h2', '', series.title));
        if (series.description) header.append(element('p', '', series.description));
        section.append(header, list(series.articles || [], series.id, series.title));
        for (const group of series.groups) {
          const details = element('details', 'series-group'); details.id = group.id; details.open = true;
          const summary = element('summary'); summary.append(element('span', '', group.title), element('span', 'series-count', `${group.articles.filter(key => articles.has(key)).length} 篇 ⌄`));
          details.append(summary);
          if (group.description) details.append(element('p', 'series-group-description', group.description));
          details.append(list(group.articles, group.id, `${series.title} / ${group.title}`)); section.append(details);
        }
        collections.append(section);
      });
      if (!model.catalog.series.length) collections.append(element('p', 'series-empty', '先在编辑器中新建一个系列，就可以把文章拖进来。'));
      host.classList.add('series-editing'); host.replaceChildren(sidebar, collections);
    }
    function clearDrop() {
      host.querySelectorAll('.is-drop-target, .is-drop-before').forEach(node => node.classList.remove('is-drop-target', 'is-drop-before'));
    }
    function destination(event) {
      const list = event.target.closest('[data-drop-target]');
      if (!list || !host.contains(list)) return null;
      const rows = [...list.querySelectorAll('[data-article-key]')].filter(row => row.dataset.articleKey !== draggedKey);
      const before = rows.find(row => { const rect = row.getBoundingClientRect(); return event.clientY < rect.top + rect.height / 2; });
      return { list, before };
    }
    host.addEventListener('dragstart', event => {
      const handle = event.target.closest('.series-drag-handle');
      if (!handle || busy) { event.preventDefault(); return; }
      const row = handle.closest('[data-article-key]'); draggedKey = row.dataset.articleKey;
      event.dataTransfer.effectAllowed = 'move'; event.dataTransfer.setData('text/plain', draggedKey);
      row.classList.add('is-dragging');
    });
    host.addEventListener('dragover', event => {
      if (!draggedKey || busy) return;
      const target = destination(event); if (!target) return;
      event.preventDefault(); event.dataTransfer.dropEffect = 'move'; clearDrop();
      target.list.classList.add('is-drop-target'); target.before?.classList.add('is-drop-before');
    });
    host.addEventListener('drop', event => {
      if (!draggedKey || busy) return;
      const target = destination(event); if (!target) return;
      event.preventDefault(); const key = draggedKey; draggedKey = null; clearDrop();
      void move(key, target.list.dataset.dropTarget, target.list.dataset.dropTarget ? target.before?.dataset.articleKey || null : null);
    });
    host.addEventListener('dragend', () => {
      draggedKey = null; clearDrop(); host.querySelectorAll('.is-dragging').forEach(node => node.classList.remove('is-dragging'));
    });
    try { model = await request('/library'); render(); status.textContent = '修改会自动保存'; }
    catch (error) { status.textContent = `编辑服务暂不可用：${error.message}。请刷新重试。`; status.classList.add('is-error'); }
  }
  document.addEventListener('DOMContentLoaded', () => void init());
  document.addEventListener('blog:preview-updated', () => void init());
})();
