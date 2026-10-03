(function (root, factory) {
  'use strict';
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else { root.NluPractice = api; if (root.document) api.boot(root.document); }
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  'use strict';
  const STORAGE_KEY = 'research-blog:nlu-practice:v1';
  const has = (o, key) => Object.prototype.hasOwnProperty.call(o, key);
  const validChoice = n => Number.isInteger(n) && n >= 0 && n < 4;
  function validateBank(bank) {
    if (!bank || !bank.version || !bank.source || !Number.isInteger(bank.source.totalPages) || bank.source.totalPages < 1 ||
        !Array.isArray(bank.topics) || !Array.isArray(bank.forms) || !Array.isArray(bank.questions) || !bank.questions.length) throw new Error('题库结构不完整。');
    const topics = new Set(bank.topics.map(t => t.id)), forms = new Set(bank.forms.map(f => f.id)), ids = new Set();
    if (topics.size !== bank.topics.length || forms.size !== bank.forms.length) throw new Error('题库分类编号重复。');
    for (const q of bank.questions) {
      if (!q || typeof q.id !== 'string' || !q.id || ids.has(q.id) || !topics.has(q.topic) || !forms.has(q.form) ||
          typeof q.prompt !== 'string' || !q.prompt.trim() || !Array.isArray(q.options) || q.options.length !== 4 ||
          q.options.some(o => typeof o !== 'string' || !o.trim()) || !validChoice(q.answer) ||
          typeof q.explanationZh !== 'string' || !Array.isArray(q.distractorsZh) || q.distractorsZh.length !== 4 ||
          !Array.isArray(q.refs) || !q.refs.length || q.refs.some(r => !Number.isInteger(r.page) || r.page < 1 || r.page > bank.source.totalPages)) throw new Error('题目校验失败：' + (q && q.id || '无编号'));
      ids.add(q.id);
    }
    return bank;
  }
  function shuffle(items, random = Math.random) {
    const result = items.slice();
    for (let i = result.length - 1; i > 0; i--) { const j = Math.floor(random() * (i + 1)); [result[i], result[j]] = [result[j], result[i]]; }
    return result;
  }
  function filterQuestions(bank, { lecture = '', topic = '', form = '', wrongOnly = false } = {}, stats = {}) {
    return bank.questions.filter(q => (!lecture || String(q.lecture) === String(lecture)) && (!topic || q.topic === topic) && (!form || q.form === form) && (!wrongOnly || !!stats[q.id]?.wrong));
  }
  function createSession(ids, mode = 'practice', now = Date.now(), bankVersion = '') {
    if (!ids.length || !['practice', 'wrongbook', 'exam'].includes(mode)) throw new Error('没有可练习的题目。');
    return { ids: [...new Set(ids)], mode, cursor: 0, answers: {}, submitted: {}, startedAt: now, deadline: mode === 'exam' ? now + 3600000 : null, finishedAt: null, recorded: false, bankVersion, entryQuestionId: null };
  }
  function remainingSeconds(session, now = Date.now()) { return session && session.mode === 'exam' ? Math.max(0, Math.ceil((session.deadline - now) / 1000)) : null; }
  function isExpired(session, now = Date.now()) { return !!session && session.mode === 'exam' && session.finishedAt === null && now >= session.deadline; }
  function scoreSession(session, bank) {
    const byId = new Map(bank.questions.map(q => [q.id, q]));
    let correct = 0, answered = 0;
    for (const id of session.ids) { if (validChoice(session.answers[id])) { answered++; if (session.answers[id] === byId.get(id)?.answer) correct++; } }
    return { total: session.ids.length, answered, correct, wrong: answered - correct, unanswered: session.ids.length - answered };
  }
  function recordAttempt(stats, q, choice) {
    const old = stats[q.id] || { attempts: 0, correct: 0 };
    stats[q.id] = { attempts: old.attempts + 1, correct: old.correct + (choice === q.answer ? 1 : 0), wrong: choice !== q.answer };
  }
  function submitPractice(session, question, stats) {
    if (session.mode === 'exam' || session.finishedAt !== null || session.submitted[question.id] || !validChoice(session.answers[question.id])) return false;
    session.submitted[question.id] = true; recordAttempt(stats, question, session.answers[question.id]); return true;
  }
  function finishSession(session, bank, stats, now = Date.now()) {
    if (session.finishedAt !== null) return false;
    if (session.mode === 'exam' && !session.recorded) {
      const byId = new Map(bank.questions.map(q => [q.id, q]));
      for (const id of session.ids) { recordAttempt(stats, byId.get(id), session.answers[id]); session.submitted[id] = true; }
      session.recorded = true;
    }
    session.finishedAt = now; return true;
  }
  function restoreState(raw, bank) {
    const clean = { schema: 1, bankVersion: bank.version, stats: {}, session: null };
    const compatible = version => version === bank.version || bank.compatibleVersions?.includes(version);
    if (!raw || raw.schema !== 1 || !compatible(raw.bankVersion)) return clean;
    const known = new Set(bank.questions.map(q => q.id));
    for (const [id, item] of Object.entries(raw.stats || {})) if (known.has(id) && item && Number.isInteger(item.attempts) && item.attempts >= 0 && Number.isInteger(item.correct) && item.correct >= 0 && item.correct <= item.attempts) clean.stats[id] = { attempts: item.attempts, correct: item.correct, wrong: item.wrong === true };
    const s = raw.session;
    if (!s || !compatible(s.bankVersion) || !['practice', 'wrongbook', 'exam'].includes(s.mode) || !Array.isArray(s.ids) || !s.ids.length || new Set(s.ids).size !== s.ids.length || !Number.isInteger(s.cursor) || s.cursor < 0 || s.cursor >= s.ids.length || !Number.isFinite(s.startedAt) || (s.finishedAt !== null && !Number.isFinite(s.finishedAt)) || (s.mode === 'exam' && (!Number.isFinite(s.deadline) || s.deadline < s.startedAt))) return clean;
    const retired = new Set(s.bankVersion !== bank.version ? bank.retiredQuestionIds || [] : []);
    if (s.ids.some(id => !known.has(id) && !retired.has(id))) return clean;
    const ids = s.ids.filter(id => known.has(id));
    const removed = s.ids.length - ids.length;
    if (removed) clean.retiredQuestionCount = removed;
    if (!ids.length) return clean;
    // Keep the current question when possible; otherwise move forward to the
    // next retained question, or back to the last one if none follows.
    const nextId = s.ids.slice(s.cursor).find(id => known.has(id));
    const cursor = nextId === undefined ? ids.length - 1 : ids.indexOf(nextId);
    const answers = {}, submitted = {};
    for (const id of ids) { if (validChoice(s.answers?.[id])) answers[id] = s.answers[id]; if (s.submitted?.[id] === true && (has(answers, id) || s.finishedAt !== null)) submitted[id] = true; }
    clean.session = { ids, mode: s.mode, cursor, answers, submitted, startedAt: s.startedAt, deadline: s.mode === 'exam' ? s.deadline : null, finishedAt: s.finishedAt, recorded: s.recorded === true, bankVersion: bank.version, entryQuestionId: typeof s.entryQuestionId === 'string' && ids.includes(s.entryQuestionId) ? s.entryQuestionId : null };
    return clean;
  }
  function lectureLink(source, page, base = 'https://example.invalid/') {
    if (!Number.isInteger(page) || page < 1 || page > source.totalPages || !source.lectureUrl) return null;
    try {
      const raw = source.lectureUrl.includes('{page}') ? source.lectureUrl.replaceAll('{page}', String(page)) : source.lectureUrl.replace(/#.*$/, '') + '#page=' + page;
      const url = new URL(raw, base); return ['https:', 'http:'].includes(url.protocol) ? url.href : null;
    } catch (_) { return null; }
  }
  function formatTime(seconds) { return String(Math.floor(seconds / 60)).padStart(2, '0') + ':' + String(seconds % 60).padStart(2, '0'); }
  function escapeHtml(value) { return String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c])); }

  function topicLessonHtml(topic, source, { session = null, open = true, base, preview = false } = {}) {
    if (!topic?.lesson || (session?.mode === 'exam' && session.finishedAt === null)) return '';
    const lesson = topic.lesson, esc = escapeHtml;
    const links = lesson.pages.map(page => {
      const href = lectureLink(source, page, base);
      return href ? '<a href="' + esc(href) + '" target="_blank" rel="noopener noreferrer">p. ' + page + ' ↗</a>' : '';
    }).join('');
    return '<details id="nlu-lesson" class="nlu-lesson" data-lesson-topic="' + esc(topic.id) + '"' + (open ? ' open' : '') + '><summary><span class="nlu-eyebrow">主题讲解</span><span class="nlu-lesson-title">' + esc(topic.nameZh) + '<span lang="en">' + esc(topic.nameEn) + '</span></span></summary><div class="nlu-lesson-body"><p class="nlu-lesson-intro">' + esc(lesson.introZh) + '</p><div class="nlu-lesson-points">' + lesson.keyPoints.map(point => '<section><h3>' + esc(point.title) + '</h3><p>' + esc(point.body) + '</p></section>').join('') + '</div>' + lesson.formulas.map(formula => '<div class="nlu-lesson-formula"><p class="nlu-formula">' + esc(formula.expression) + '</p><p>' + esc(formula.explanationZh) + '</p></div>').join('') + '<section class="nlu-lesson-example"><h3>' + esc(lesson.example.title) + '</h3><p>' + esc(lesson.example.body) + '</p></section><p class="nlu-lesson-pitfall"><strong>易混淆点：</strong>' + esc(lesson.pitfallZh) + '</p><div class="nlu-sources"><span>相关讲义</span>' + links + '</div><div class="nlu-lesson-actions">' + (preview ? '<button class="nlu-primary" data-action="topic" data-topic="' + esc(topic.id) + '">开始本主题练习</button>' : '<button data-action="skip-lesson">开始做题 ↓</button>') + '</div></div></details>';
  }

  async function boot(doc) {
    const app = doc.getElementById('nlu-app'); if (!app || app.dataset.booted) return;
    app.dataset.booted = 'true';
    const content = doc.getElementById('nlu-content'), status = doc.getElementById('nlu-status');
    const view = doc.defaultView || globalThis;
    const say = message => { status.textContent = message; };
    let bank;
    try { const response = await view.fetch(app.dataset.bankUrl); if (!response.ok) throw new Error('HTTP ' + response.status); bank = validateBank(await response.json()); }
    catch (error) { say('题库载入失败，请刷新重试。' + error.message); return; }
    let raw = null, storageOk = true;
    try { raw = JSON.parse(view.localStorage.getItem(STORAGE_KEY) || 'null'); } catch (_) { storageOk = false; }
    let state = restoreState(raw, bank), review = false, lecture = '', topic = '', form = '', navOnly = 'all', coveragePage = 1, previewTopic = '';
    const lessonExpanded = new Map();
    let migrationNotice = state.retiredQuestionCount ? '题库更新：本轮已移除 ' + state.retiredQuestionCount + ' 道细节题。' + (state.session ? '其余作答进度已保留。' : '本轮已无可用题目，其余累计记录已保留。') : '';
    const questions = new Map(bank.questions.map(q => [q.id, q]));
    const topics = new Map(bank.topics.map(t => [t.id, t]));
    const forms = new Map(bank.forms.map(t => [t.id, t]));
    const esc = escapeHtml;
    let cancelConfirmation = null;
    function askConfirmation(message, title = '确认操作', confirmLabel = '继续') {
      if (cancelConfirmation) return Promise.resolve(false);
      return new Promise(resolve => {
        const previousFocus = doc.activeElement;
        const dialog = doc.createElement('dialog');
        dialog.className = 'nlu-confirm';
        dialog.setAttribute('aria-labelledby', 'nlu-confirm-title');
        dialog.setAttribute('aria-describedby', 'nlu-confirm-body');
        dialog.innerHTML = '<h2 id="nlu-confirm-title">' + esc(title) + '</h2><p id="nlu-confirm-body">' + esc(message) + '</p><div class="nlu-confirm-actions"><button type="button" data-confirm="cancel" autofocus>取消</button><button type="button" data-confirm="continue" class="nlu-primary">' + esc(confirmLabel) + '</button></div>';
        let settled = false;
        const settle = approved => {
          if (settled) return;
          settled = true; cancelConfirmation = null;
          if (dialog.open) dialog.close();
          dialog.remove();
          if (previousFocus?.isConnected) previousFocus.focus({ preventScroll: true });
          resolve(approved);
        };
        cancelConfirmation = () => settle(false);
        dialog.addEventListener('cancel', event => { event.preventDefault(); settle(false); });
        dialog.addEventListener('close', () => settle(false));
        dialog.addEventListener('click', event => {
          const button = event.target.closest('button[data-confirm]');
          if (button) settle(button.dataset.confirm === 'continue');
        });
        app.append(dialog);
        try { dialog.showModal(); dialog.querySelector('[data-confirm="cancel"]')?.focus(); }
        catch (_) { settle(false); say('无法显示确认窗口，操作已取消。请更新浏览器后重试。'); }
      });
    }
    function persist() { try { view.localStorage.setItem(STORAGE_KEY, JSON.stringify(state)); } catch (_) { storageOk = false; say('进度保存失败，请检查浏览器存储设置。'); } }
    if (migrationNotice) persist();
    function sourceLinks(refs, label = '回到讲义') {
      return '<div class="nlu-sources"><span>' + label + '</span>' + refs.map(r => {
        const href = lectureLink(bank.source, r.page, view.location.href);
        return href ? '<a href="' + esc(href) + '" target="_blank" rel="noopener noreferrer">' + (r.lecture ? 'Lec ' + esc(r.lecture) + ' · ' : '') + 'p. ' + r.page + ' ↗</a>' : '<span>p. ' + r.page + '</span>';
      }).join('') + '</div>';
    }
    function summary() {
      const s = state.session, result = scoreSession(s, bank);
      const submitted = s.ids.filter(id => s.submitted[id]).length;
      const scoredCorrect = s.ids.filter(id => s.submitted[id] && s.answers[id] === questions.get(id).answer).length;
      return '<section class="nlu-summary" aria-labelledby="nlu-summary-title"><p class="nlu-eyebrow">ROUND COMPLETE</p><h2 id="nlu-summary-title" tabindex="-1">这一轮，先到这里</h2><div class="nlu-metrics"><div><strong>' + (s.mode === 'exam' ? result.correct : scoredCorrect) + '<small> / ' + result.total + '</small></strong><span>答对</span></div><div><strong>' + (s.mode === 'exam' ? result.answered : submitted) + '</strong><span>' + (s.mode === 'exam' ? '已作答' : '已提交') + '</span></div><div><strong>' + (s.mode === 'exam' ? result.unanswered : result.total - submitted) + '</strong><span>' + (s.mode === 'exam' ? '未作答' : '未提交') + '</span></div></div><p>' + (s.mode === 'exam' ? '答错和未答的题目已加入错题本。' : '已保存本轮提交的答案。') + '</p><div class="nlu-actions"><button class="nlu-primary" data-action="review">逐题复盘</button><button data-action="wrongbook">再练错题</button><button data-action="practice">开始新一轮</button></div></section>';
    }
    function explanation(q, s) {
      const selected = s.answers[q.id], answered = validChoice(selected), correct = selected === q.answer;
      const outcome = s.mode !== 'exam' && !s.submitted[q.id] ? '— 本题未提交' : answered ? (correct ? '✓ 回答正确' : '× 本题答错') : '— 本题未作答';
      return '<section class="nlu-explanation" aria-labelledby="nlu-answer-title"><h3 id="nlu-answer-title">' + outcome + ' · 正确答案 ' + 'ABCD'[q.answer] + '</h3><p>' + esc(q.explanationZh) + '</p>' + (q.noteZh ? '<p class="nlu-note"><strong>辨析：</strong>' + esc(q.noteZh) + '</p>' : '') + '<details><summary>逐项辨析 / Option notes</summary><ul>' + q.distractorsZh.map((text, i) => text ? '<li><strong>' + 'ABCD'[i] + '.</strong> ' + esc(text) + '</li>' : '').join('') + '</ul></details>' + (q.terms?.length ? '<dl class="nlu-terms">' + q.terms.map(t => '<div><dt lang="en">' + esc(t.en) + '</dt><dd>' + esc(t.zh) + '</dd></div>').join('') + '</dl>' : '') + sourceLinks(q.refs) + '</section>';
    }
    function questionPanel() {
      const s = state.session, q = questions.get(s.ids[s.cursor]);
      const finished = s.finishedAt !== null, exam = s.mode === 'exam' && !finished, revealed = !exam && (finished || !!s.submitted[q.id]);
      const answered = validChoice(s.answers[q.id]);
      const completed = s.mode === 'exam' ? s.ids.filter(id => validChoice(s.answers[id])).length : s.ids.filter(id => s.submitted[id]).length;
      return '<section class="nlu-workspace" aria-labelledby="nlu-question"><div class="nlu-session-line"><span>' + (exam ? '限时模拟' : finished ? '复盘模式' : s.mode === 'wrongbook' ? '错题重练' : '自由练习') + ' · ' + completed + ' / ' + s.ids.length + (exam ? ' 已答' : ' 已提交') + '</span>' + (exam ? '<span id="nlu-timer" class="nlu-timer" role="timer" aria-label="剩余时间">' + formatTime(remainingSeconds(s)) + '</span>' : '') + '</div><progress max="' + s.ids.length + '" value="' + completed + '" aria-label="本轮完成进度"></progress>' + topicLessonHtml(topics.get(q.topic), bank.source, { session: s, open: lessonExpanded.get(q.topic) !== false, base: view.location.href }) + '<div class="nlu-question-meta"><span>QUESTION ' + String(s.cursor + 1).padStart(2, '0') + ' / ' + s.ids.length + '</span><span>' + esc(topics.get(q.topic)?.nameZh) + ' · ' + esc(forms.get(q.form)?.nameZh) + ' · ' + esc(q.difficulty) + '</span></div><h2 id="nlu-question" lang="en" tabindex="-1">' + esc(q.prompt) + '</h2><fieldset class="nlu-options"><legend class="nlu-sr">选择一个答案 / Select one answer</legend>' + q.options.map((option, i) => {
        const classes = ['nlu-option', s.answers[q.id] === i ? 'is-selected' : '', revealed && i === q.answer ? 'is-correct' : '', revealed && i === s.answers[q.id] && i !== q.answer ? 'is-wrong' : ''].filter(Boolean).join(' ');
        return '<label class="' + classes + '"><input type="radio" name="nlu-answer" value="' + i + '"' + (s.answers[q.id] === i ? ' checked' : '') + (revealed ? ' disabled' : '') + '><span class="nlu-letter">' + 'ABCD'[i] + '</span><span lang="en">' + esc(option) + '</span>' + (revealed && i === q.answer ? '<span class="nlu-answer-tag">正确答案</span>' : '') + (revealed && i === s.answers[q.id] && i !== q.answer ? '<span class="nlu-answer-tag">你的选择</span>' : '') + '</label>';
      }).join('') + '</fieldset><div class="nlu-question-actions">' + (!exam && !revealed ? '<button class="nlu-primary" data-action="submit"' + (!answered ? ' disabled' : '') + '>提交并看解析</button>' : exam ? '<p>选择即保存；交卷后统一展示答案和解析。</p>' : '') + '<div class="nlu-step"><button data-action="prev"' + (s.cursor === 0 ? ' disabled' : '') + '>← 上一题</button><button data-action="next"' + (s.cursor === s.ids.length - 1 ? ' disabled' : '') + '>下一题 →</button></div></div>' + (revealed ? explanation(q, s) : sourceLinks(q.refs, '相关讲义')) + (q.sourceQuestions?.length ? '<p class="nlu-provenance">对应 Mock 题号：' + esc(q.sourceQuestions.map(n => '#' + n).join('、')) + '</p>' : '') + '<div class="nlu-round-actions">' + (finished ? '<button data-action="summary">返回本轮小结</button>' : '<button data-action="finish">' + (exam ? '交卷并查看结果' : '结束本轮并查看小结') + '</button>') + '</div></section>' + navigator();
    }
    function navigator() {
      const s = state.session, finished = s.finishedAt !== null;
      const exam = s.mode === 'exam';
      return '<details class="nlu-navigator" open><summary>题目导航 / Question navigator</summary><div class="nlu-nav-filters"><button data-action="nav-all" aria-pressed="' + (navOnly === 'all') + '">全部</button><button data-action="nav-unanswered" aria-pressed="' + (navOnly === 'unanswered') + '">' + (exam ? '未作答' : '未提交') + '</button></div><div class="nlu-number-grid">' + s.ids.map((id, index) => {
        const done = exam ? validChoice(s.answers[id]) : !!s.submitted[id];
        if (navOnly === 'unanswered' && done) return '';
        const revealed = exam ? finished : !!s.submitted[id];
        const correct = revealed && s.answers[id] === questions.get(id).answer;
        const text = revealed ? (correct ? '答对' : done ? '答错' : '未完成') : done ? '已答' : '未答';
        return '<button data-action="jump" data-index="' + index + '" class="' + (done ? 'is-done ' : '') + (revealed ? correct ? 'is-correct' : 'is-wrong' : '') + '" aria-label="第 ' + (index + 1) + ' 题，' + text + '"' + (index === s.cursor ? ' aria-current="step"' : '') + '>' + (index + 1) + '</button>';
      }).join('') + '</div><p class="nlu-muted">' + (exam && !finished ? '填充表示已作答。' : '带勾为答对，带叉为答错；边框标出当前题。') + '</p></details>';
    }
    function topicCard(t) {
      const group = bank.questions.filter(q => q.topic === t.id);
      const done = group.filter(q => state.stats[q.id]?.attempts > 0).length;
      const canRead = t.lesson && !(state.session?.mode === 'exam' && state.session.finishedAt === null);
      return '<section><h3><button data-action="topic" data-topic="' + esc(t.id) + '">' + esc(t.nameZh) + ' <span lang="en">' + esc(t.nameEn) + '</span></button></h3><p>' + esc(t.focusZh) + '</p><p class="nlu-muted">' + group.length + ' 道练习 · 已练 ' + done + '</p>' + (canRead ? '<button class="nlu-read-lesson" data-action="lesson" data-topic="' + esc(t.id) + '">阅读主题讲解</button>' : '') + sourceLinks((t.pages || []).map(page => ({ page, lecture: t.lecture })), '讲义') + '</section>';
    }
    function pageAudit(page) {
      const row = bank.coverage?.pageAudit.find(item => item.page === page);
      if (!row) return '<p class="nlu-muted">暂无记录。</p>';
      const points = bank.coverage.knowledgePoints.filter(point => row.knowledgePointIds.includes(point.id));
      const types = { section: '封面 / 导读', repeat: '复习 / 重复页', content: '知识页', resource: '资源页', context: '背景 / 插图' };
      return '<div class="nlu-page-heading"><strong>p. ' + page + '</strong><span>' + esc(types[row.kind] || row.kind) + '</span>' + sourceLinks([{ page }], '') + '</div><p>' + esc(row.summaryZh) + '</p>' + (row.repeatOf ? '<p class="nlu-muted">复用 p. ' + row.repeatOf + ' 的知识点。</p>' : '') + '<ul class="nlu-point-list">' + points.map(point => '<li><span>' + esc(point.titleZh) + '</span><div>' + point.questionIds.map(id => '<button data-action="question" data-question="' + esc(id) + '">' + esc(topics.get(questions.get(id)?.topic)?.nameZh || '') + ' ' + esc(id.match(/\d+$/)?.[0] || id) + '</button>').join('') + '</div></li>').join('') + '</ul>';
    }
    function coveragePanel() {
      const groups = bank.lectures || [{ id: null, nameZh: '全部主题', nameEn: '', pages: [1, 400] }];
      return '<details class="nlu-coverage"><summary>按讲复习 · 知识点与页码</summary>' + groups.map(group => {
        const items = bank.topics.filter(t => group.id === null || t.lecture === group.id);
        const count = bank.questions.filter(q => group.id === null || q.lecture === group.id).length;
        return '<details class="nlu-lecture-group"><summary>' + (group.id ? 'Lec ' + group.id + ' · ' : '') + esc(group.nameZh) + '<span>p. ' + group.pages[0] + '–' + group.pages[1] + ' · ' + count + ' 题</span></summary><div class="nlu-topic-list">' + items.map(topicCard).join('') + '</div></details>';
      }).join('') + (bank.coverage ? '<details class="nlu-page-explorer"><summary>按页查知识点 · ' + bank.coverage.pageAudit.length + ' 页</summary><label for="nlu-page">PDF 页码</label><input id="nlu-page" type="number" min="1" max="400" value="' + coveragePage + '"><div id="nlu-page-result" aria-live="polite">' + pageAudit(coveragePage) + '</div></details>' : '') + '</details>';
    }
    function dashboard(pool, practiced, wrong) {
      const visibleTopics = bank.topics.filter(t => !lecture || String(t.lecture) === lecture);
      return '<section class="nlu-dashboard" aria-label="练习设置"><div class="nlu-topline"><span><strong>' + bank.questions.length + '</strong> 道题 · <strong>' + bank.topics.length + '</strong> 个知识点组</span><span>已练 ' + practiced + ' · 错题 ' + wrong + '</span></div><div class="nlu-controls"><label>讲次 / Lecture<select id="nlu-lecture"><option value="">全部讲次</option>' + (bank.lectures || []).map(l => '<option value="' + l.id + '"' + (String(l.id) === lecture ? ' selected' : '') + '>Lec ' + l.id + ' · ' + esc(l.nameZh) + '</option>').join('') + '</select></label><label>知识主题 / Topic<select id="nlu-topic"><option value="">全部主题</option>' + visibleTopics.map(t => '<option value="' + esc(t.id) + '"' + (t.id === topic ? ' selected' : '') + '>' + esc(t.nameZh + ' / ' + t.nameEn) + '</option>').join('') + '</select></label><label>题型 / Form<select id="nlu-form"><option value="">全部题型</option>' + bank.forms.map(f => '<option value="' + esc(f.id) + '"' + (f.id === form ? ' selected' : '') + '>' + esc(f.nameZh) + '</option>').join('') + '</select></label></div><div class="nlu-actions"><button data-action="practice" class="nlu-primary">开始练习（' + pool.length + '）</button><button data-action="wrongbook">错题重练（' + filterQuestions(bank, { lecture, topic, form, wrongOnly: true }, state.stats).length + '）</button><button data-action="exam">' + Math.min(100, bank.questions.length) + ' 题 / 60 分钟模拟</button><button data-action="reset" class="nlu-quiet">清除本地记录</button></div><p class="nlu-muted">模拟从全题库抽取 100 题。</p></section>';
    }
    function render(focusQuestion = false, focusLesson = false) {
      const wrong = Object.values(state.stats).filter(item => item.wrong).length;
      const practiced = Object.values(state.stats).filter(item => item.attempts > 0).length;
      const pool = filterQuestions(bank, { lecture, topic, form });
      const studyTopic = previewTopic || (!state.session ? topic : '');
      const study = studyTopic ? topicLessonHtml(topics.get(studyTopic), bank.source, { session: state.session, open: true, base: view.location.href, preview: true }) : '';
      content.innerHTML = dashboard(pool, practiced, wrong) + (study ? '<section class="nlu-study-preview">' + (state.session ? '<button data-action="close-lesson" class="nlu-quiet">← 返回当前练习</button>' : '') + study + '</section>' : state.session ? state.session.finishedAt !== null && !review ? summary() : questionPanel() : '<section class="nlu-empty"><p class="nlu-eyebrow">READY WHEN YOU ARE</p><h2>选一讲，先读讲解，再做练习。</h2><p>英文题干 · 中文解析 · 逐题回到讲义</p></section>') + coveragePanel() + '<p class="nlu-footnote">题库版本 ' + esc(bank.version) + ' · 进度自动保存在当前浏览器。' + (!storageOk ? ' 当前无法持久保存。' : '') + '</p>';
      if (focusQuestion || focusLesson) {
        const heading = (focusLesson && doc.querySelector('#nlu-lesson > summary')) || doc.getElementById('nlu-question');
        heading?.focus({ preventScroll: true });
        heading?.scrollIntoView({ block: 'start', behavior: 'instant' });
      }
    }
    async function confirmReplace() { return !state.session || state.session.finishedAt !== null || await askConfirmation(state.session.mode === 'exam' ? '当前模拟尚未交卷。开始新一轮将放弃这次模拟，确定继续？' : '开始新一轮会替换当前进度；已提交记录会保留。继续吗？', '开始新一轮', '继续开始'); }
    async function start(mode, selectedTopic, selectedQuestion) {
      const nextTopic = selectedTopic !== undefined ? selectedTopic : topic;
      const nextForm = selectedTopic !== undefined ? '' : form;
      const nextLecture = selectedTopic !== undefined ? String(topics.get(selectedTopic)?.lecture || '') : lecture;
      let pool = mode === 'exam' ? shuffle(bank.questions).slice(0, 100) : filterQuestions(bank, { lecture: nextLecture, topic: nextTopic, form: nextForm, wrongOnly: mode === 'wrongbook' }, state.stats);
      if (!pool.length) { say(mode === 'wrongbook' ? '当前筛选下没有错题。可以更换主题或先做一轮练习。' : '当前筛选没有题目，请调整主题或题型。'); render(); return; }
      if (!await confirmReplace()) return;
      lecture = nextLecture; topic = nextTopic; form = nextForm;
      state.session = createSession(pool.map(q => q.id), mode, Date.now(), bank.version);
      if (selectedQuestion && state.session.ids.includes(selectedQuestion)) { state.session.cursor = state.session.ids.indexOf(selectedQuestion); state.session.entryQuestionId = selectedQuestion; }
      review = false; previewTopic = ''; navOnly = 'all'; persist(); render(true, mode !== 'exam'); say(mode === 'exam' ? '模拟已开始，交卷后查看解析。' : '主题讲解已就绪，可以先读再练。');
    }
    function expire() {
      if (!isExpired(state.session)) return false;
      if (cancelConfirmation) cancelConfirmation();
      finishSession(state.session, bank, state.stats); review = false; persist(); render(); say(migrationNotice + '60 分钟已到，已自动交卷。未作答题计入本轮失分和错题本。'); migrationNotice = ''; return true;
    }
    content.addEventListener('toggle', event => {
      if (content.contains(event.target) && event.target.matches('details[data-lesson-topic]')) lessonExpanded.set(event.target.dataset.lessonTopic, event.target.open);
    }, true);
    content.addEventListener('input', event => {
      if (event.target.id !== 'nlu-page') return;
      const page = Number(event.target.value);
      if (!Number.isInteger(page) || page < 1 || page > 400) return;
      coveragePage = page;
      doc.getElementById('nlu-page-result').innerHTML = pageAudit(coveragePage);
    });
    content.addEventListener('change', event => {
      if (event.target.id === 'nlu-page') { coveragePage = Math.max(1, Math.min(400, Math.trunc(Number(event.target.value)) || 1)); event.target.value = coveragePage; doc.getElementById('nlu-page-result').innerHTML = pageAudit(coveragePage); return; }
      if (event.target.id === 'nlu-lecture') { lecture = event.target.value; topic = ''; previewTopic = ''; render(); return; }
      if (event.target.id === 'nlu-topic' || event.target.id === 'nlu-form') { topic = doc.getElementById('nlu-topic').value; form = doc.getElementById('nlu-form').value; if (previewTopic) previewTopic = topic; render(); return; }
      if (event.target.name !== 'nlu-answer' || !state.session || expire()) return;
      const s = state.session, id = s.ids[s.cursor];
      if (s.finishedAt !== null || (s.mode !== 'exam' && s.submitted[id])) return;
      const choice = Number(event.target.value); if (!validChoice(choice)) return;
      s.answers[id] = choice; persist(); render();
      const radio = content.querySelector('input[name="nlu-answer"][value="' + choice + '"]'); radio?.focus({ preventScroll: true });
    });
    content.addEventListener('click', async event => {
      const button = event.target.closest('button[data-action]'); if (!button || button.disabled) return;
      const action = button.dataset.action;
      if (expire()) return;
      if (['practice', 'wrongbook', 'exam'].includes(action)) { await start(action); return; }
      if (action === 'lesson') { if (state.session?.mode === 'exam' && state.session.finishedAt === null) return; previewTopic = button.dataset.topic; topic = previewTopic; lecture = String(topics.get(topic)?.lecture || ''); form = ''; render(false, true); return; }
      if (action === 'close-lesson') { previewTopic = ''; const q = questions.get(state.session?.ids[state.session.cursor]); if (q) { topic = q.topic; lecture = String(q.lecture); form = ''; } render(true); return; }
      if (action === 'skip-lesson') { const q = questions.get(state.session?.ids[state.session.cursor]); if (q) lessonExpanded.set(q.topic, false); render(true); return; }
      if (action === 'topic') { await start('practice', button.dataset.topic); return; }
      if (action === 'question') { const q = questions.get(button.dataset.question); if (q) await start('practice', q.topic, q.id); return; }
      if (action === 'reset') {
        if (!await askConfirmation('清除本浏览器中的全部 NLU 作答记录、错题和当前练习？此操作无法撤销，其他博客设置不会受影响。', '清除本地记录', '确认清除')) return;
        state = restoreState(null, bank); review = false; previewTopic = ''; persist(); render(); say('练习记录已清除。'); return;
      }
      const s = state.session; if (!s) return;
      if (action === 'submit') { if (submitPractice(s, questions.get(s.ids[s.cursor]), state.stats)) { persist(); render(); doc.getElementById('nlu-answer-title')?.scrollIntoView({ block: 'nearest' }); say(s.answers[s.ids[s.cursor]] === questions.get(s.ids[s.cursor]).answer ? '回答正确。解析已展开。' : '已记录到错题本。解析已展开。'); } return; }
      if (action === 'finish') {
        const r = scoreSession(s, bank);
        if (!await askConfirmation(s.mode === 'exam' ? '确定交卷？还有 ' + r.unanswered + ' 道未作答，交卷后不能再修改。' : '结束本轮并查看成绩？', s.mode === 'exam' ? '确认交卷' : '结束本轮', s.mode === 'exam' ? '确认交卷' : '结束并查看小结')) return;
        finishSession(s, bank, state.stats); review = false; persist(); render(); doc.getElementById('nlu-summary-title')?.focus(); say('本轮已结束，可以逐题复盘或再练错题。'); return;
      }
      if (action === 'review') { review = true; s.cursor = 0; navOnly = 'all'; render(true, true); return; }
      if (action === 'summary') { review = false; render(); return; }
      if (action === 'nav-all' || action === 'nav-unanswered') { navOnly = action === 'nav-all' ? 'all' : 'unanswered'; render(); return; }
      if (action === 'prev' || action === 'next' || action === 'jump') { const oldTopic = questions.get(s.ids[s.cursor]).topic; s.cursor = action === 'jump' ? Math.max(0, Math.min(s.ids.length - 1, Number(button.dataset.index))) : Math.max(0, Math.min(s.ids.length - 1, s.cursor + (action === 'next' ? 1 : -1))); persist(); render(true, oldTopic !== questions.get(s.ids[s.cursor]).topic); }
    });
    const params = new URLSearchParams(view.location.search), deepId = params.get('question'), deepTopic = params.get('topic');
    const activeExam = state.session?.mode === 'exam' && state.session.finishedAt === null;
    if (!activeExam && deepId && questions.has(deepId)) {
      const q = questions.get(deepId); topic = q.topic;
      if (state.session?.entryQuestionId !== deepId) {
        const ids = filterQuestions(bank, { topic }).map(item => item.id);
        state.session = createSession(ids, 'practice', Date.now(), bank.version); state.session.cursor = ids.indexOf(deepId); state.session.entryQuestionId = deepId; persist();
      }
    } else if (deepTopic && topics.has(deepTopic)) topic = deepTopic;
    if (topic) lecture = String(topics.get(topic)?.lecture || '');
    else if ((bank.lectures || []).some(l => String(l.id) === params.get('lecture'))) lecture = params.get('lecture');
    if (!expire()) { render(); say(migrationNotice + (storageOk ? activeExam ? '已恢复进行中的模拟；倒计时继续。' : state.session ? '已恢复本浏览器中的练习进度。' : '题库已就绪。选一个主题开始吧。' : '本次可以练习，但浏览器存储不可用，进度可能无法保留。')); migrationNotice = ''; }
    view.setInterval(() => {
      if (expire()) return;
      if (state.session?.mode === 'exam' && state.session.finishedAt === null) {
        const timer = doc.getElementById('nlu-timer'); if (timer) { const left = remainingSeconds(state.session); timer.textContent = formatTime(left); timer.classList.toggle('is-urgent', left <= 300); }
      }
    }, 1000);
    doc.addEventListener('visibilitychange', () => { if (!doc.hidden) expire(); });
  }
  return { STORAGE_KEY, validateBank, shuffle, filterQuestions, createSession, remainingSeconds, isExpired, scoreSession, submitPractice, finishSession, restoreState, lectureLink, formatTime, escapeHtml, topicLessonHtml, boot };
});
