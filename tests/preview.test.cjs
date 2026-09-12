const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '../site/static/preview.js'), 'utf8');

// Only model the DOM surfaces used by the plugin. HTTP responses select fixed
// page fixtures, while the real plugin owns fetching, retries and DOM updates.
class Element {
  constructor(tagName, text = '') {
    this.tagName = tagName;
    this._text = text;
    this.childNodes = [];
    this.dataset = {};
    this.style = {};
    this.listeners = {};
    this.attributes = {};
  }
  get textContent() { return this._text + this.childNodes.map(node => node.textContent).join(''); }
  set textContent(value) { this._text = value; this.childNodes = []; }
  setAttribute(name, value) { this.attributes[name] = value; }
  getAttribute(name) { return this.attributes[name] ?? null; }
  appendChild(node) { node.parentNode = this; this.childNodes.push(node); return node; }
  replaceChildren(...nodes) {
    this._text = '';
    this.childNodes = [];
    for (const node of nodes) this.appendChild(node);
  }
  remove() {
    this.parentNode.childNodes = this.parentNode.childNodes.filter(node => node !== this);
    this.parentNode = null;
  }
  addEventListener(name, listener) { this.listeners[name] = listener; }
  click() { this.listeners.click?.(); }
  querySelectorAll() { return []; }
}

function page({ text = '上一版正文', article = true, main = true, complete = true, notFound = false, buildError = false, stylesheet = '/style.css' } = {}) {
  const body = new Element('body');
  const mainNode = main ? body.appendChild(new Element('main')) : null;
  const content = mainNode?.appendChild(new Element(article ? 'article' : 'div', text));
  const error = new Element('p', 'Zola Build Error:');
  error.nextElementSibling = new Element('pre', 'invalid TOML');
  const nav = { outerHTML: '<nav class="nav">博客</nav>' };
  const footer = { outerHTML: '<footer class="site-footer">笔记</footer>' };
  const css = new Element('link');
  css.setAttribute('href', stylesheet);
  return {
    body,
    title: text,
    events: [],
    querySelector(selector) {
      if (selector === 'main') return mainNode;
      if (selector === '.article') return article ? content : null;
      if (selector === '[data-preview-not-found]') return notFound ? content : null;
      if (selector === 'nav.nav') return nav;
      if (selector === '.site-footer') return complete ? footer : null;
      return null;
    },
    querySelectorAll(selector) {
      if (selector === 'head script[src],head link[rel="stylesheet"]') return [css];
      return selector === 'p' && buildError ? [error] : [];
    },
    getElementById(id) { return body.childNodes.find(node => node.id === id) ?? null; },
    createElement(tagName) { return new Element(tagName); },
    dispatchEvent(event) { this.events.push(event.type); },
  };
}

function response(status = 200, fixture = 'updated') {
  return { ok: status >= 200 && status < 300, status, text: async () => fixture };
}

function harness(steps, initialPage) {
  let now = 0;
  let timerId = 0;
  let stepIndex = 0;
  let navigations = 0;
  const requests = [];
  const timers = new Map();
  const document = initialPage || page();
  const fixtures = {
    updated: () => page({ text: '刚输入的新正文' }),
    malformed: () => page({ main: false }),
    partialArticle: () => page({ complete: false }),
    notFound: () => page({ article: false, notFound: true, text: '404' }),
    wrongPage: () => page({ article: false, text: '其他页面' }),
    buildError: () => page({ main: false, buildError: true }),
  };
  const context = {
    document,
    location: {
      hostname: '127.0.0.1', origin: 'http://127.0.0.1:1111',
      href: 'http://127.0.0.1:1111/posts/chinese-draft/',
      reload: () => { navigations += 1; },
    },
    fetch: async (url, options) => {
      requests.push({ url, options, time: now });
      const step = steps[Math.min(stepIndex++, steps.length - 1)];
      if (step instanceof Error) throw step;
      return typeof step === 'function' ? step() : step;
    },
    DOMParser: class { parseFromString(key) { return fixtures[key](); } },
    setTimeout: (callback, delay) => {
      timers.set(++timerId, { callback, at: now + delay });
      return timerId;
    },
    clearTimeout: id => timers.delete(id),
    AbortSignal: { timeout: () => undefined },
    performance: { now: () => now },
    Event: class { constructor(type) { this.type = type; } },
    scrollX: 0, scrollY: 123, scrollTo: () => {},
    URL, console,
  };
  context.window = context;
  vm.runInNewContext(source, context, { filename: 'preview.js' });
  const plugin = new context.LiveReloadPluginBlogPreview();
  // Drain promise chains without real retry delays, keeping timing deterministic.
  const settle = () => new Promise(resolve => setImmediate(resolve));
  return {
    plugin, document, requests, timers, settle,
    get navigations() { return navigations; },
    get text() { return document.querySelector('main').textContent; },
    get notice() { return document.getElementById('blog-preview-error'); },
    async nextTimer() {
      assert.ok(timers.size, 'expected a scheduled recovery attempt');
      const [id, timer] = [...timers.entries()].sort((a, b) => a[1].at - b[1].at)[0];
      timers.delete(id);
      now = timer.at;
      timer.callback();
      await settle();
      return now;
    },
  };
}

test('a transient 404 keeps the last page and recovers without another save', async () => {
  const h = harness([response(404), response()]);
  h.plugin.reload('/x.js');
  await h.settle();
  assert.equal(h.text, '上一版正文');
  assert.equal(h.navigations, 0);
  assert.equal(h.requests.length, 1);
  assert.equal(await h.nextTimer(), 25);
  assert.equal(h.text, '刚输入的新正文');
  assert.equal(h.requests.length, 2);
  assert.equal(h.timers.size, 0);
  assert.equal(h.navigations, 0);
  assert.deepEqual(h.document.events, ['blog:preview-updated']);
});

test('a network failure keeps the last page and retries automatically', async () => {
  const h = harness([new Error('Failed to fetch'), response()]);
  h.plugin.reload('/x.js');
  await h.settle();
  assert.equal(h.text, '上一版正文');
  assert.equal(h.navigations, 0);
  await h.nextTimer();
  assert.equal(h.text, '刚输入的新正文');
  assert.equal(h.navigations, 0);
});

test('persistent 404 has bounded retries and a working manual retry button', async () => {
  const steps = [response(404)];
  const h = harness(steps);
  h.plugin.reload('/x.js');
  await h.settle();
  let attempts = 0;
  while (h.timers.size && attempts < 20) { await h.nextTimer(); attempts += 1; }
  assert.equal(attempts, 8);
  assert.equal(h.requests.length, 9);
  assert.equal(h.timers.size, 0, 'must stop retrying an address that stays unavailable');
  assert.equal(h.text, '上一版正文');
  assert.equal(h.navigations, 0);
  const button = h.notice.childNodes.find(node => node.tagName === 'button');
  assert.ok(button, 'the retained page should offer a manual retry');
  assert.equal(button.textContent, '立即重试');
  steps.push(response());
  button.click();
  await h.settle();
  assert.equal(h.text, '刚输入的新正文');
  assert.equal(h.notice, null);
  assert.equal(h.navigations, 0);
});

test('a new save interrupts backoff immediately and cancels the obsolete timer', async () => {
  const h = harness([response(404), response(404), response()]);
  h.plugin.reload('/x.js');
  await h.settle();
  await h.nextTimer();
  assert.equal(h.requests.length, 2);
  assert.equal(h.timers.size, 1);
  h.plugin.reload('/x.js');
  await h.settle();
  assert.equal(h.requests.length, 3);
  assert.equal(h.requests[2].time, h.requests[1].time, 'new event must not wait for retry delay');
  assert.equal(h.text, '刚输入的新正文');
  assert.equal(h.timers.size, 0);
  assert.equal(h.navigations, 0);
});

for (const fixture of ['malformed', 'partialArticle', 'notFound', 'wrongPage', 'buildError']) {
  test(`HTTP 200 ${fixture} preserves the last page until valid HTML arrives`, async () => {
    const h = harness([response(200, fixture), response()]);
    h.plugin.reload('/x.js');
    await h.settle();
    assert.equal(h.text, '上一版正文');
    assert.equal(h.navigations, 0);
    if (fixture === 'buildError') {
      assert.ok(h.notice);
      assert.match(h.notice.textContent, /invalid TOML/);
    }
    await h.nextTimer();
    assert.equal(h.text, '刚输入的新正文');
    assert.equal(h.notice, null);
    assert.equal(h.navigations, 0);
  });
}

test('healthy updates patch immediately without timers or page navigation', async () => {
  const h = harness([response()]);
  assert.equal(h.plugin.reload('/x.js'), true);
  await h.settle();
  assert.equal(h.requests.length, 1);
  assert.equal(h.requests[0].options.cache, 'no-store');
  assert.equal(h.text, '刚输入的新正文');
  assert.equal(h.document.title, '刚输入的新正文');
  assert.equal(h.document.querySelector('main').dataset.previewUpdates, '1');
  assert.equal(h.timers.size, 0);
  assert.equal(h.navigations, 0);
});

test('a previous CSS live reload does not force navigation on the next article edit', async () => {
  const h = harness([response()], page({ stylesheet: '/style.css?livereload=123456' }));
  h.plugin.reload('/x.js');
  await h.settle();
  assert.equal(h.text, '刚输入的新正文');
  assert.equal(h.navigations, 0);
  assert.equal(h.timers.size, 0);
});

test('a save arriving during a failed request is fetched immediately', async () => {
  let finishFirst;
  const firstResponse = new Promise(resolve => { finishFirst = resolve; });
  const h = harness([() => firstResponse, response()]);
  h.plugin.reload('/x.js');
  await h.settle();
  h.plugin.reload('/x.js');
  finishFirst(response(404));
  await h.settle();
  assert.equal(h.requests.length, 2);
  assert.equal(h.text, '刚输入的新正文');
  assert.equal(h.timers.size, 0);
  assert.equal(h.navigations, 0);
});
