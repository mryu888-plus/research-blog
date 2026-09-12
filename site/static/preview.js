/* Local Zola preview only: LiveReload plugin, no production activation. */
(() => {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(location.hostname)) return;
  class BlogPreview {
    static identifier = 'blog-preview';
    static version = '1.0';
    constructor() {
      this.pending = false;
      this.running = false;
      this.failures = 0;
      this.retryTimer = null;
    }
    reload(path) {
      // Keep native LiveReload's stylesheet/image replacement behavior.
      if (/\.(css|woff2?)(?:\?|$)/i.test(path)) return false;
      if (/\.js(?:\?|$)/i.test(path) && !/(^|\/)x\.js$/.test(path)) return false;
      clearTimeout(this.retryTimer);
      this.retryTimer = null;
      this.failures = 0;
      this.pending = true;
      if (!this.running) void this.update();
      return true;
    }
    retry(message, buildError = false) {
      this.failures += 1;
      const retrying = this.failures <= 8;
      // Zola clears its in-memory routes before a full rebuild. A short 404
      // is normal here; navigating to it would discard LiveReload itself.
      if (buildError || this.failures >= 5) {
        let notice = document.getElementById('blog-preview-error');
        if (!notice) {
          notice = document.createElement('aside');
          notice.id = 'blog-preview-error';
          notice.setAttribute('role', 'status');
          notice.style.cssText = 'position:fixed;bottom:12px;right:12px;max-width:600px;max-height:25vh;overflow:auto;white-space:pre-wrap;padding:16px;background:#431717;color:white;z-index:1000;border-radius:8px;font:14px/1.5 monospace';
          document.body.appendChild(notice);
        }
        notice.textContent = '暂时保留上一次预览。' + (retrying ? '正在自动重试…\n' : '下次保存时会再次更新。\n') + message;
        if (!retrying) {
          const button = document.createElement('button');
          button.textContent = '立即重试';
          button.addEventListener('click', () => this.reload('/x.js'));
          notice.appendChild(button);
        }
      }
      if (retrying) {
        this.retryTimer = setTimeout(() => {
          this.retryTimer = null;
          this.pending = true;
          if (!this.running) void this.update();
        }, Math.min(25 * 2 ** (this.failures - 1), 1000));
      }
    }
    async update() {
      this.running = true;
      try {
        while (this.pending) {
          this.pending = false;
          const started = performance.now();
          const response = await fetch(location.href, { cache: 'no-store', signal: AbortSignal.timeout(3000) });
          if (!response.ok) {
            throw new Error(response.status === 404
              ? '文章地址暂时不可用；如果修改了文件夹名或 slug，请从文章列表打开新地址。'
              : `预览服务暂时不可用（HTTP ${response.status}）。`);
          }
          const html = await response.text();
          const next = new DOMParser().parseFromString(html, 'text/html');
          const errorHeading = Array.from(next.querySelectorAll('p')).find(p => p.textContent.trim() === 'Zola Build Error:');
          if (errorHeading) {
            const error = new Error('文章暂时无法编译：\n' + (errorHeading.nextElementSibling?.textContent || '请检查文章格式。'));
            error.buildError = true;
            throw error;
          }
          if (this.pending) continue; // A newer change is already waiting.
          const currentMain = document.querySelector('main');
          const nextMain = next.querySelector('main');
          if (!nextMain || next.querySelector('[data-preview-not-found]') ||
              (document.querySelector('.article') && !next.querySelector('.article')) ||
              ['nav.nav', '.site-footer'].some(selector => document.querySelector(selector) && !next.querySelector(selector))) {
            throw new Error('预览页面尚未就绪，请稍候。');
          }
          this.failures = 0;
          document.getElementById('blog-preview-error')?.remove();
          if (!currentMain) { location.reload(); return; }
          // Structural/template changes deserve a normal reload.
          for (const selector of ['nav.nav', '.site-footer']) {
            if (document.querySelector(selector)?.outerHTML !== next.querySelector(selector)?.outerHTML) {
              location.reload(); return;
            }
          }
          const assets = doc => Array.from(doc.querySelectorAll('head script[src],head link[rel="stylesheet"]')).map(node => {
            const url = new URL(node.getAttribute('src') || node.getAttribute('href'), location.href);
            // LiveReload adds this cache buster when replacing a stylesheet.
            url.searchParams.delete('livereload');
            return url.href;
          }).join('|');
          if (assets(document) !== assets(next) ||
              JSON.stringify(document.body.dataset) !== JSON.stringify(next.body.dataset) ||
              Boolean(document.getElementById('agent-panel')) !== Boolean(next.getElementById('agent-panel'))) {
            location.reload(); return;
          }
          const x = scrollX, y = scrollY;
          const tocOpen = document.querySelector('.toc details')?.open;
          const stamp = Date.now();
          for (const img of nextMain.querySelectorAll('img[src]')) {
            const url = new URL(img.getAttribute('src'), location.href);
            if (url.origin === location.origin) { url.searchParams.set('_preview', stamp); img.src = url.href; }
          }
          const figurePreview = document.querySelector('#figure-dialog[open] img');
          if (figurePreview?.src) {
            const url = new URL(figurePreview.src);
            if (url.origin === location.origin) { url.searchParams.set('_preview', stamp); figurePreview.src = url.href; }
          }
          document.querySelectorAll('.ask-trigger').forEach(button => button.remove());
          currentMain.replaceChildren(...nextMain.childNodes);
          document.title = next.title;
          for (const name of ['description', 'og:title', 'og:description', 'og:url']) {
            const selector = `meta[name="${name}"],meta[property="${name}"]`;
            const original = document.querySelector(selector), replacement = next.querySelector(selector);
            if (original && replacement) original.content = replacement.content;
          }
          const toc = document.querySelector('.toc details');
          if (toc && tocOpen !== undefined) toc.open = tocOpen;
          document.dispatchEvent(new Event('blog:preview-updated'));
          window.scrollTo(x, y);
          currentMain.dataset.previewUpdates = String(Number(currentMain.dataset.previewUpdates || 0) + 1);
          currentMain.dataset.previewMs = (performance.now() - started).toFixed(2);
        }
      } catch (error) {
        // A newer save should be tried immediately, without waiting for backoff.
        if (!this.pending) this.retry(error.message || '预览连接暂时中断。', error.buildError);
      } finally {
        this.running = false;
        if (this.pending) void this.update();
      }
    }
  }
  window.LiveReloadPluginBlogPreview = BlogPreview;
  if (window.LiveReload) window.LiveReload.addPlugin(BlogPreview);
})();
