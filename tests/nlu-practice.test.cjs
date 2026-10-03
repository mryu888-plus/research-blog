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

test('lecture filters intersect topics and forms, including numeric and string values', () => {
  const b = structuredClone(bank);
  b.questions.forEach((q, i) => { q.lecture = i < 2 ? 1 : 5; });
  assert.deepEqual(core.filterQuestions(b, { lecture: '1' }).map(q => q.id), ['q0', 'q1']);
  assert.deepEqual(core.filterQuestions(b, { lecture: 5, topic: 't2', form: 'definition' }).map(q => q.id), ['q2']);
  assert.equal(core.filterQuestions(b, { lecture: 1, topic: 't2' }).length, 0);
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

test('an explicitly compatible expansion preserves progress, choices, cursor and exam deadline', () => {
  const b = { ...bank, version: 'test-2', compatibleVersions: [bank.version] };
  const s = core.createSession(['q0', 'q1'], 'exam', 1000, bank.version);
  s.cursor = 1; s.answers.q0 = 2;
  const raw = { schema: 1, bankVersion: bank.version, stats: { q2: { attempts: 3, correct: 2, wrong: false } }, session: s };
  const migrated = core.restoreState(raw, b);
  assert.deepEqual(migrated.stats, raw.stats);
  assert.equal(migrated.session.cursor, 1);
  assert.deepEqual(migrated.session.answers, { q0: 2 });
  assert.equal(migrated.session.deadline, 3601000);
  assert.equal(migrated.bankVersion, 'test-2');
  assert.equal(migrated.session.bankVersion, 'test-2');
});

test('compatible retirement preserves retained answers, submissions, cursor and deadline', () => {
  const b = { ...bank, version: 'test-2', compatibleVersions: [bank.version], retiredQuestionIds: ['retired'] };
  const s = core.createSession(['q0', 'retired', 'q1'], 'exam', 1000, bank.version);
  s.cursor = 2; s.answers = { q0: 2, retired: 1, q1: 1 }; s.submitted = { q0: true, retired: true }; s.entryQuestionId = 'q0';
  const raw = { schema: 1, bankVersion: bank.version, stats: { q0: { attempts: 2, correct: 1, wrong: true }, retired: { attempts: 1, correct: 0, wrong: true } }, session: s };
  const migrated = core.restoreState(raw, b);
  assert.equal(migrated.retiredQuestionCount, 1);
  assert.deepEqual(migrated.session.ids, ['q0', 'q1']);
  assert.equal(migrated.session.cursor, 1);
  assert.deepEqual(migrated.session.answers, { q0: 2, q1: 1 });
  assert.deepEqual(migrated.session.submitted, { q0: true });
  assert.equal(migrated.session.entryQuestionId, 'q0');
  assert.equal(migrated.session.startedAt, 1000);
  assert.equal(migrated.session.deadline, 3601000);
  assert.deepEqual(Object.keys(migrated.stats), ['q0']);
  assert.deepEqual(s.ids, ['q0', 'retired', 'q1'], 'migration must not mutate saved input');
  const reloaded = core.restoreState(JSON.parse(JSON.stringify(migrated)), b);
  assert.deepEqual(reloaded.session, migrated.session);
  assert.equal(reloaded.retiredQuestionCount, undefined, 'migration notice is only raised on the migration');
});

test('retiring the current question selects the next retained question, or the final previous one', () => {
  const b = { ...bank, version: 'test-2', compatibleVersions: [bank.version], retiredQuestionIds: ['retired'] };
  const s = core.createSession(['q0', 'retired', 'q1'], 'practice', 1000, bank.version);
  s.cursor = 1; s.entryQuestionId = 'retired';
  const raw = { schema: 1, bankVersion: bank.version, stats: {}, session: s };
  let migrated = core.restoreState(raw, b);
  assert.equal(migrated.session.ids[migrated.session.cursor], 'q1');
  assert.equal(migrated.session.entryQuestionId, null);
  s.ids = ['q0', 'q1', 'retired']; s.cursor = 2;
  migrated = core.restoreState(raw, b);
  assert.equal(migrated.session.ids[migrated.session.cursor], 'q1');
});

test('fully retired sessions clear safely while unknown IDs and current-version corruption are rejected', () => {
  const b = { ...bank, version: 'test-2', compatibleVersions: [bank.version], retiredQuestionIds: ['retired'] };
  const raw = { schema: 1, bankVersion: bank.version, stats: { q0: { attempts: 1, correct: 1, wrong: false } }, session: core.createSession(['retired'], 'practice', 1000, bank.version) };
  let restored = core.restoreState(raw, b);
  assert.equal(restored.session, null); assert.equal(restored.retiredQuestionCount, 1);
  assert.deepEqual(restored.stats, raw.stats);
  raw.session.ids = ['q0', 'unknown', 'retired'];
  restored = core.restoreState(raw, b);
  assert.equal(restored.session, null); assert.equal(restored.retiredQuestionCount, undefined);
  raw.bankVersion = b.version; raw.session.bankVersion = b.version; raw.session.ids = ['q0', 'retired'];
  assert.equal(core.restoreState(raw, b).session, null, 'retirements only apply to a compatible older session');
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
test('lecture-wide bank has five lectures, 5–8 questions per group and valid lecture links', { skip: !fs.existsSync(bankPath) && 'Question authors have not assembled bank.json yet' }, () => {
  const real = core.validateBank(JSON.parse(fs.readFileSync(bankPath, 'utf8').replace(/^\uFEFF/, '')));
  assert(real.questions.length > 112);
  assert(real.topics.length > 18);
  assert.deepEqual(real.lectures.map(l => l.id), [1, 2, 3, 4, 5]);
  for (const l of real.lectures) assert(real.questions.filter(q => q.lecture === l.id).length >= 20, 'Missing lecture ' + l.id);
  assert.equal(real.source.totalPages, 400);
  assert.equal(real.source.lectureUrl, 'https://sentic.net/nlu-slides.pdf');
  assert.deepEqual(real.topics.flatMap(t => t.originalQuestions).sort((a, b) => a - b), Array.from({ length: 100 }, (_, i) => i + 1), 'Original questions 1–100 must belong to exactly one topic');
  for (const topic of real.topics) {
    const group = real.questions.filter(q => q.topic === topic.id);
    assert(group.length >= 5 && group.length <= 8, topic.id + ': expected 5–8 questions');
    assert(Array.isArray(topic.pages) && topic.pages.length > 0, topic.id);
    assert(topic.pages.every(p => Number.isInteger(p) && p >= 1 && p <= 400), topic.id);
    for (const q of group) {
      assert(Array.isArray(q.sourceQuestions), q.id);
      // A new question can combine concepts from multiple source topics.
      assert(q.sourceQuestions.every(n => Number.isInteger(n) && n >= 1 && n <= 100), q.id + ': invalid source question');
      assert.equal(q.lecture, topic.lecture, q.id);
      assert(q.refs.every(r => Number.isInteger(r.lecture) && r.lecture >= 1 && r.lecture <= 5 && typeof r.reasonZh === 'string' && r.reasonZh.trim()), q.id + ': missing lecture number or reason');
      assert(q.refs.every(r => core.lectureLink(real.source, r.page).endsWith('#page=' + r.page)), q.id);
      assert(q.explanationZh.trim(), q.id);
      assert(new Set(q.options).size === 4, q.id + ': duplicate choices');
      assert(q.distractorsZh.every((text, i) => i === q.answer || typeof text === 'string' && text.trim()), q.id + ': missing distractor explanation');
    }
  }
});

test('all 400 pages have an explicit audit; background pages need no rote-recall question', () => {
  const b = JSON.parse(fs.readFileSync(bankPath, 'utf8'));
  const { pageAudit, knowledgePoints } = b.coverage;
  assert.equal(pageAudit.length, 400);
  assert.deepEqual(pageAudit.map(p => p.page), Array.from({ length: 400 }, (_, i) => i + 1));
  const pointMap = new Map(knowledgePoints.map(p => [p.id, p]));
  const qMap = new Map(b.questions.map(q => [q.id, q]));
  assert.equal(pointMap.size, knowledgePoints.length);
  for (const row of pageAudit) {
    assert(['content', 'resource', 'context', 'section', 'repeat'].includes(row.kind), row.page);
    assert(row.summaryZh.trim(), row.page);
    assert(row.knowledgePointIds.every(id => pointMap.has(id)), row.page);
    if (row.kind === 'content') assert(row.knowledgePointIds.length > 0, row.page);
    if (row.kind === 'repeat') assert(Number.isInteger(row.repeatOf) && row.repeatOf >= 1 && row.repeatOf <= 400 && row.repeatOf !== row.page, row.page);
  }
  for (const point of knowledgePoints) {
    assert(point.titleZh && point.pages.length > 0 && point.questionIds.length > 0, point.id);
    assert(point.questionIds.every(id => qMap.has(id)), point.id);
    assert(point.pages.every(p => pageAudit[p - 1].kind !== 'content' || pageAudit[p - 1].knowledgePointIds.includes(point.id)), point.id);
    const refPages = new Set(point.questionIds.flatMap(id => qMap.get(id).refs.map(r => r.page)));
    assert(point.pages.some(p => refPages.has(p)), point.id + ': missing source overlap');
  }
});

test('retained original answers remain compatible and retired detail questions are absent', () => {
  const paths = [path.join(__dirname, '../scripts/nlu/baseline-bank.json'), path.join(__dirname, '../baseline-bank.json')];
  const previous = JSON.parse(fs.readFileSync(paths.find(p => fs.existsSync(p)), 'utf8'));
  const current = JSON.parse(fs.readFileSync(bankPath, 'utf8'));
  const byId = new Map(current.questions.map(q => [q.id, q]));
  assert(current.compatibleVersions.includes(previous.version));
  assert.equal(previous.questions.length, 112);
  for (const old of previous.questions) {
    if (current.retiredQuestionIds.includes(old.id)) { assert(!byId.has(old.id), old.id); continue; }
    assert.deepEqual(byId.get(old.id).options, old.options, old.id);
    assert.equal(byId.get(old.id).answer, old.answer, old.id);
  }
});

test('a real legacy 100-question exam survives removal of detail-recall questions', () => {
  const previous = JSON.parse(fs.readFileSync(path.join(__dirname, '../scripts/nlu/baseline-bank.json'), 'utf8'));
  const current = JSON.parse(fs.readFileSync(bankPath, 'utf8'));
  const known = new Set(current.questions.map(q => q.id));
  const s = core.createSession(previous.questions.slice(0, 100).map(q => q.id), 'exam', 1000, previous.version);
  s.cursor = 50;
  for (const id of s.ids) s.answers[id] = previous.questions.find(q => q.id === id).answer;
  const expectedIds = s.ids.filter(id => known.has(id));
  assert(expectedIds.length < 100, 'fixture must include at least one retired question');
  const migrated = core.restoreState({ schema: 1, bankVersion: previous.version, stats: {}, session: s }, current);
  assert.deepEqual(migrated.session.ids, expectedIds);
  assert.equal(migrated.retiredQuestionCount, 100 - expectedIds.length);
  assert.equal(migrated.session.deadline, s.deadline);
  assert.equal(migrated.session.finishedAt, null);
  assert.equal(migrated.session.ids[migrated.session.cursor], s.ids.slice(s.cursor).find(id => known.has(id)));
  assert.equal(core.scoreSession(migrated.session, current).correct, expectedIds.length);
  assert(core.isExpired(migrated.session, s.deadline), 'migration must not grant extra exam time');
});

test('question stems ask knowledge rather than slide-specific recall', () => {
  const b = JSON.parse(fs.readFileSync(bankPath, 'utf8'));
  const recall = /\b(?:lecture\s+(?:page|slide)|on\s+(?:lecture\s+)?page\s+\d|table\s+\d|figure\s+\d|mock\s+(?:quiz|q\d)|according to the (?:lecture|caption)|in the (?:lecture|slide)'s|how many .* (?:shown|listed))\b/i;
  for (const q of b.questions) assert(!recall.test(q.prompt), q.id + ': slide-recall wording');
});
