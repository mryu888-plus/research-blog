"use strict";
let currentSelection = "";
let currentArticleId = "";
let activeRequest;

// Buffer complete SSE events: a network chunk is not an event boundary.
function createSseParser(onData) {
  let pending = "";
  return {
    push(chunk) {
      pending += chunk;
      let boundary;
      while ((boundary = /\r?\n\r?\n/.exec(pending))) {
        const event = pending.slice(0, boundary.index);
        pending = pending.slice(boundary.index + boundary[0].length);
        const data = event.split(/\r?\n/).filter(line => line.startsWith("data:"))
          .map(line => line.slice(5).replace(/^ /, "")).join("\n");
        if (data && data !== "[DONE]") onData(JSON.parse(data));
      }
    },
  };
}

function searchPosts(posts, query) {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return [];
  return posts.map(post => {
    const title = post.title.toLocaleLowerCase();
    const text = `${post.title} ${post.tags.join(" ")} ${post.description} ${post.body}`.toLocaleLowerCase();
    return { post, score: terms.every(term => text.includes(term)) ?
      terms.reduce((score, term) => score + (title.includes(term) ? 10 : 1), 0) : 0 };
  }).filter(item => item.score).sort((a, b) => b.score - a.score).slice(0, 30).map(item => item.post);
}

if (typeof module !== "undefined") module.exports = { createSseParser, searchPosts };
if (typeof document !== "undefined") document.addEventListener("DOMContentLoaded", () => {
  document.getElementById("theme-toggle").onclick = () => {
    const theme = document.documentElement.dataset.theme === "light" ? "dark" : "light";
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem("blog-theme", theme); } catch (_) {}
  };
  document.querySelectorAll("[data-close-dialog]").forEach(button => {
    button.onclick = () => button.closest("dialog").close();
  });
  document.querySelectorAll("dialog").forEach(dialog => dialog.addEventListener("click", event => {
    if (event.target !== dialog) return;
    const rect = dialog.getBoundingClientRect();
    if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) dialog.close();
  }));
  initSearch();
  initFigures();
  document.querySelectorAll("pre > code").forEach(code => {
    const button = document.createElement("button");
    button.className = "copy-code";
    button.textContent = "复制代码";
    button.onclick = async () => {
      try { await navigator.clipboard.writeText(code.textContent); button.textContent = "已复制"; }
      catch (_) { button.textContent = "请手动选择复制"; }
      setTimeout(() => { button.textContent = "复制代码"; }, 2000);
    };
    code.parentElement.appendChild(button);
  });
  if (typeof renderMathInElement === "function") {
    const content = document.querySelector(".article-content");
    if (content) renderMathInElement(content, { delimiters: [
      { left: "$$", right: "$$", display: true }, { left: "$", right: "$", display: false },
    ], throwOnError: false });
  }
  const article = document.querySelector(".article");
  if (article && document.body.dataset.agentEndpoint) {
    currentArticleId = article.dataset.articleId;
    initAgent();
  }
});

function initSearch() {
  const dialog = document.getElementById("search-dialog");
  const input = document.getElementById("search-input");
  const status = document.getElementById("search-status");
  const results = document.getElementById("search-results");
  let indexPromise;
  let revision = 0;
  function open() { dialog.showModal(); input.focus(); }
  document.getElementById("search-toggle").onclick = open;
  document.addEventListener("keydown", event => {
    if (event.key === "/" && !event.ctrlKey && !event.metaKey && !event.altKey &&
      !event.target.closest("input,textarea,[contenteditable=true]") && !document.querySelector("dialog[open]")) {
      event.preventDefault(); open();
    }
  });
  input.addEventListener("input", async () => {
    const version = ++revision;
    const query = input.value.trim();
    results.replaceChildren();
    if (!query) { status.textContent = "输入关键词开始搜索"; return; }
    status.textContent = "正在搜索…";
    try {
      if (!indexPromise) indexPromise = fetch(document.body.dataset.searchUrl).then(response => {
        if (!response.ok) throw new Error("index unavailable");
        return response.json();
      }).catch(error => { indexPromise = null; throw error; });
      const posts = await indexPromise;
      if (version !== revision) return;
      const matches = searchPosts(posts, query);
      status.textContent = matches.length ? `找到 ${matches.length} 篇笔记` : "没有找到匹配的笔记，试试其他关键词。";
      for (const post of matches) {
        const link = document.createElement("a");
        link.className = "search-result";
        link.href = post.url;
        const title = document.createElement("strong"); title.textContent = post.title;
        const description = document.createElement("p"); description.textContent = post.description;
        link.append(title, description); results.appendChild(link);
      }
    } catch (_) { if (version === revision) status.textContent = "搜索暂时不可用，请检查网络后重新输入。"; }
  });
}

function initFigures() {
  const dialog = document.getElementById("figure-dialog");
  document.querySelectorAll(".figure-wrapper img").forEach(img => {
    img.tabIndex = 0;
    img.setAttribute("role", "button");
    img.setAttribute("aria-label", `放大：${img.alt}`);
    function show() {
      const preview = dialog.querySelector("img"); preview.src = img.src; preview.alt = img.alt;
      dialog.querySelector("p").textContent = img.closest("figure").querySelector("figcaption")?.textContent || img.alt;
      dialog.showModal();
    }
    img.onclick = show;
    img.onkeydown = event => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); show(); } };
  });
}

function initAgent() {
  const panel = document.getElementById("agent-panel");
  const input = document.getElementById("question-input");
  const submit = document.getElementById("submit-question");
  document.getElementById("ask-article").onclick = () => openAgentPanel();
  document.getElementById("close-panel").onclick = () => {
    activeRequest?.abort(); panel.classList.add("hidden"); document.getElementById("ask-article").focus();
  };
  submit.onclick = async () => {
    const question = input.value.trim();
    if (!question || activeRequest) return;
    input.value = "";
    await sendQuestion(question, currentSelection);
    currentSelection = "";
  };
  input.onkeydown = event => { if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) submit.click(); };
  let askButton;
  document.querySelector(".article-content").addEventListener("mouseup", () => {
    askButton?.remove();
    const selection = window.getSelection();
    const text = selection?.toString().trim();
    if (!text || text.length > 500 || !selection.rangeCount) return;
    const rect = selection.getRangeAt(0).getBoundingClientRect();
    askButton = document.createElement("button"); askButton.className = "ask-trigger"; askButton.textContent = "询问";
    askButton.style.left = `${Math.min(rect.left + scrollX, document.documentElement.clientWidth - 95)}px`;
    askButton.style.top = `${rect.bottom + scrollY + 8}px`;
    askButton.onclick = () => { currentSelection = text; openAgentPanel("请解释选中的内容"); askButton.remove(); };
    document.body.appendChild(askButton);
  });
}

function openAgentPanel(question = "") {
  document.getElementById("agent-panel").classList.remove("hidden");
  const input = document.getElementById("question-input"); input.value = question; input.focus();
}

async function sendQuestion(question, selection) {
  const output = document.getElementById("agent-output");
  const submit = document.getElementById("submit-question");
  const user = document.createElement("div"); user.className = "message user-message"; user.textContent = question;
  const answer = document.createElement("div"); answer.className = "message agent-message"; answer.textContent = "思考中…";
  output.append(user, answer);
  activeRequest = new AbortController(); submit.disabled = true;
  const timeout = setTimeout(() => activeRequest?.abort(), 90000);
  let received = false;
  try {
    const response = await fetch(document.body.dataset.agentEndpoint, {
      method: "POST", headers: { "Content-Type": "application/json" }, signal: activeRequest.signal,
      body: JSON.stringify({ article_id: currentArticleId, question, selection, mode: "reader" }),
    });
    if (!response.ok || !response.body) throw new Error(`助手暂时无法响应（${response.status}）`);
    const reader = response.body.getReader(); const decoder = new TextDecoder();
    const parser = createSseParser(data => {
      if (data.error) throw new Error("助手生成失败，请稍后重试。");
      const token = data.type === "token" ? data.text : data.content;
      if (token) { if (!received) answer.textContent = ""; received = true; answer.textContent += token; }
      output.scrollTop = output.scrollHeight;
    });
    while (true) { const { done, value } = await reader.read(); if (done) break; parser.push(decoder.decode(value, { stream: true })); }
    parser.push(decoder.decode());
    if (!received) throw new Error("助手没有返回内容，请重试。");
  } catch (error) {
    answer.classList.add("error-message");
    answer.textContent += `\n${error.name === "AbortError" ? "请求已停止或超时。" : error.message}`;
  } finally { clearTimeout(timeout); activeRequest = null; submit.disabled = false; output.scrollTop = output.scrollHeight; }
}
