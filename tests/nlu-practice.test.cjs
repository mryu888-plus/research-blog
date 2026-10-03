'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const core = require('../site/static/nlu-practice/app.js');
const bank = { version: 'test-1', source: { totalPages: 400, lectureUrl: 'https://example.org/nlu.pdf' }, topics: [{ id: 't1' }, { id: 't2' }], forms: [{ id: 'definition' }], questions: [0, 1, 2].map(n => ({ id: 'q' + n, topic: n < 2 ? 't1' : 't2', form: 'definition', prompt: 'Question ' + n, options: ['A','B','C','D'], answer: n, explanationZh: '解释', distractorsZh: ['','','',''], refs: [{ page: n + 1 }] })) };
test('bank validation rejects duplicate ids, invalid answers and absent source pages', () => {
  assert.equal(core.validateBank(bank), bank);
  for (const mutate of [b => b.questions[1].id = 'q0', b => b.questions[0].answer = 4, b => b.questions[0].refs = [], b => b.questions[0].refs[0].page = 401]) {
    const b = structuredClone(bank); mutate(b); assert.throws(() => core.validateBank(b));
  }
});
test('filters intersect topics forms and wrongbook, without mutating bank', () => {
  assert.deepEqual(core.filterQuestions(bank, { topic: 't1', wrongOnly: true }, { q1: { wrong: true } }).map(q => q.id), ['q1']);
  assert.equal(core.filterQuestions(bank, { form: 'missing' }).length, 0); assert.equal(bank.questions.length, 3);
});
test('shuffle is a permutation and leaves input intact', () => {
  const input = [1,2,3,4]; const shuffled = core.shuffle(input, () => 0); assert.deepEqual(input, [1,2,3,4]); assert.deepEqual([...shuffled].sort(), input); assert.notDeepEqual(shuffled, input);
});
test('practice submission is counted once and correction removes wrongbook item', () => {
  const stats = {}, s = core.createSession(['q0'], 'practice', 0, bank.version);
  assert.equal(core.submitPractice(s, bank.questions[0], stats), false);
  s.answers.q0 = 1; assert.equal(core.submitPractice(s, bank.questions[0], stats), true); assert.equal(core.submitPractice(s, bank.questions[0], stats), false);
  assert.deepEqual(stats.q0, { attempts: 1, correct: 0, wrong: true });
  const second = core.createSession(['q0'], 'wrongbook', 1, bank.version); second.answers.q0 = 0; core.submitPractice(second, bank.questions[0], stats);
  assert.deepEqual(stats.q0, { attempts: 2, correct: 1, wrong: false });
});
test('exam deadline survives reload; elapsed sessions expire rather than restart', () => {
  const s = core.createSession(['q0','q1'], 'exam', 1000, bank.version); s.answers.q0 = 0;
  const restored = core.restoreState(JSON.parse(JSON.stringify({ schema: 1, bankVersion: bank.version, stats: {}, session: s })), bank).session;
  assert.equal(restored.deadline, 3601000); assert.equal(core.remainingSeconds(restored, 60000), 3541); assert.equal(core.isExpired(restored, 3601000), true);
  assert.equal(core.remainingSeconds(restored, 9999999), 0);
});
test('exam results withheld from progress until finish and unanswers are scored', () => {
  const stats = {}, s = core.createSession(['q0','q1','q2'], 'exam', 1000, bank.version); s.answers.q0 = 0; s.answers.q1 = 3;
  assert.equal(core.submitPractice(s, bank.questions[0], stats), false); assert.deepEqual(stats, {});
  assert.deepEqual(core.scoreSession(s, bank), { total: 3, answered: 2, correct: 1, wrong: 1, unanswered: 1 });
  assert.equal(core.finishSession(s, bank, stats, 2000), true); assert.equal(core.finishSession(s, bank, stats, 3000), false);
  assert.deepEqual(stats.q2, { attempts: 1, correct: 0, wrong: true }); assert.equal(stats.q0.attempts, 1);
});
test('finishing practice does not record unsubmitted answers as errors', () => {
  const stats = {}, s = core.createSession(['q0','q1'], 'practice', 0, bank.version); s.answers.q0 = 3; core.finishSession(s, bank, stats, 10); assert.deepEqual(stats, {});
});
test('restore rejects incompatible versions, broken cursor, stale questions and invalid choices', () => {
  const raw = { schema: 1, bankVersion: bank.version, stats: { q0: { attempts: 2, correct: 1, wrong: true }, gone: { attempts: 1, correct: 0, wrong: true } }, session: core.createSession(['q0'], 'practice', 0, bank.version) };
  raw.session.answers.q0 = 8;
  const restored = core.restoreState(raw, bank); assert.deepEqual(restored.session.answers, {}); assert.deepEqual(Object.keys(restored.stats), ['q0']);
  assert.equal(core.restoreState({ ...raw, bankVersion: 'old' }, bank).session, null);
  const invalid = structuredClone(raw); invalid.session.cursor = 100; assert.equal(core.restoreState(invalid, bank).session, null);
  invalid.session.cursor = 0; invalid.session.ids = ['gone']; assert.equal(core.restoreState(invalid, bank).session, null);
});
test('deep-link origin survives reload without losing later cursor or answer progress', () => {
  const session = core.createSession(['q0','q1'], 'practice', 1000, bank.version);
  session.entryQuestionId = 'q0'; session.cursor = 1; session.answers.q0 = 0; session.submitted.q0 = true;
  const restored = core.restoreState(JSON.parse(JSON.stringify({ schema: 1, bankVersion: bank.version, stats: {}, session })), bank).session;
  assert.equal(restored.entryQuestionId, 'q0'); assert.equal(restored.cursor, 1); assert.equal(restored.startedAt, 1000);
  assert.deepEqual(restored.answers, { q0: 0 }); assert.deepEqual(restored.submitted, { q0: true });
  const raw = { schema: 1, bankVersion: bank.version, stats: {}, session: { ...session, entryQuestionId: 'q2' } };
  assert.equal(core.restoreState(raw, bank).session.entryQuestionId, null, 'origin must belong to this session');
  delete raw.session.entryQuestionId; assert.equal(core.restoreState(raw, bank).session.entryQuestionId, null, 'legacy sessions remain valid');
});
test('lecture links preserve exact page and support configurable local viewer', () => {
  assert.equal(core.lectureLink(bank.source, 216), 'https://example.org/nlu.pdf#page=216');
  assert.equal(core.lectureLink({ ...bank.source, lectureUrl: '../nlu-lecture/?page={page}' }, 219, 'https://host/research-blog/nlu-practice/'), 'https://host/research-blog/nlu-lecture/?page=219');
  assert.equal(core.lectureLink({ ...bank.source, lectureUrl: 'javascript:alert(1)' }, 1), null); assert.equal(core.lectureLink(bank.source, 0), null);
});
test('text helpers escape dynamic HTML and use stable timer format', () => { assert.equal(core.escapeHtml('<img onerror="x">'), '&lt;img onerror=&quot;x&quot;&gt;'); assert.equal(core.formatTime(3600), '60:00'); assert.equal(core.formatTime(59), '00:59'); });
test('confirmation UI is nonblocking, labelled, and defaults to cancellation', () => {
  const source = fs.readFileSync(path.join(__dirname, '../site/static/nlu-practice/app.js'), 'utf8');
  assert(!/\b(?:window|view|globalThis)\.confirm\s*\(/.test(source));
  assert(source.includes("dialog.setAttribute('aria-labelledby', 'nlu-confirm-title')"));
  assert(source.includes("dialog.setAttribute('aria-describedby', 'nlu-confirm-body')"));
  assert(source.includes("dialog.addEventListener('cancel', event => { event.preventDefault(); settle(false); })"));
  assert(source.includes('dialog.showModal()'));
  assert(source.includes('if (cancelConfirmation) cancelConfirmation();'), 'exam expiry must dismiss stale confirmations');
});
const bankPath = path.join(__dirname, '../site/static/nlu-practice/bank.json');
test('published bank has 112 questions across 18 topics and exact source coverage', { skip: !fs.existsSync(bankPath) && 'Question authors have not assembled bank.json yet' }, () => {
  const real = core.validateBank(JSON.parse(fs.readFileSync(bankPath, 'utf8').replace(/^\uFEFF/, '')));
  assert.equal(real.questions.length, 112);
  assert.equal(real.topics.length, 18);
  assert.equal(real.source.totalPages, 400);
  assert.equal(real.source.lectureUrl, 'https://sentic.net/nlu-slides.pdf');
  assert.deepEqual(real.topics.flatMap(t => t.originalQuestions).sort((a, b) => a - b), Array.from({ length: 100 }, (_, i) => i + 1), 'Original questions 1–100 must belong to exactly one topic');
  for (const topic of real.topics) {
    const group = real.questions.filter(q => q.topic === topic.id);
    assert(group.length >= 5 && group.length <= 8, topic.id + ': expected 5–8 questions');
    assert(Array.isArray(topic.pages) && topic.pages.length > 0, topic.id);
    assert(topic.pages.every(p => Number.isInteger(p) && p >= 1 && p <= 400), topic.id);
    for (const q of group) {
      assert(Array.isArray(q.sourceQuestions) && q.sourceQuestions.length > 0, q.id);
      // A new question can combine concepts from multiple source topics.
      assert(q.sourceQuestions.every(n => Number.isInteger(n) && n >= 1 && n <= 100), q.id + ': invalid source question');
      assert(q.refs.every(r => Number.isInteger(Number(r.lecture)) && Number(r.lecture) >= 1 && Number(r.lecture) <= 6 && typeof r.reasonZh === 'string' && r.reasonZh.trim()), q.id + ': missing lecture number or visible reason');
      assert(q.refs.every(r => core.lectureLink(real.source, r.page).endsWith('#page=' + r.page)), q.id);
      assert(q.explanationZh.trim(), q.id);
      assert(new Set(q.options).size === 4, q.id + ': duplicate choices');
      assert(q.distractorsZh.every((text, i) => i === q.answer || typeof text === 'string' && text.trim()), q.id + ': missing distractor explanation');
    }
  }
});
