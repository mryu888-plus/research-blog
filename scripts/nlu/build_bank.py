"""Build the lecture-wide question bank and verify its page/knowledge mapping.

Run from the publishing checkout: python scripts/nlu/build_bank.py
The original option order is retained so stored answers remain valid.
"""
import argparse
import copy
import json
import random
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent
VERSION = '2026-10-03.3'
RETIRED_IDS = {
    'general-07', 'microtext-03', 'sbd-04', 'pos-03', 'concept-06',
    'anaphora-04', 'anaphora-06', 'subjectivity-04', 'sarcasm-06',
    'personality-04', 'aspect-06', 'primenet-02', 'primenet-03',
}
LECTURES = [
    {'id': 1, 'nameZh': '数据科学', 'nameEn': 'Data Science', 'pages': [6, 59]},
    {'id': 2, 'nameZh': '人工智能基础', 'nameEn': 'AI Basics', 'pages': [60, 148]},
    {'id': 3, 'nameZh': 'AI 的过去与未来', 'nameEn': 'Past and Future of AI', 'pages': [149, 205]},
    {'id': 4, 'nameZh': '行李箱模型与 NLU 任务', 'nameEn': 'Suitcase Model', 'pages': [206, 284]},
    {'id': 5, 'nameZh': '神经符号 AI 与情感计算', 'nameEn': 'Neurosymbolic AI', 'pages': [285, 400]},
]


def read(name):
    return json.loads((ROOT / name).read_text(encoding='utf-8-sig'))


def lecture_for(page):
    return next((lec['id'] for lec in LECTURES if lec['pages'][0] <= page <= lec['pages'][1]), 1)


def build():
    baseline = read('baseline-bank.json')
    bank = {**copy.deepcopy(baseline), 'version': VERSION, 'compatibleVersions': [baseline['version'], '2026-10-03.2'], 'lectures': LECTURES, 'retiredQuestionIds': sorted(RETIRED_IDS)}
    bank['questions'] = [q for q in bank['questions'] if q['id'] not in RETIRED_IDS]
    natural_stems = {q['id']: q['prompt'] for name in ('content-a.json', 'content-b.json') for q in read(name)['questions']}
    natural_stems.update({
        'primenet-01': 'Which ordering goes from the most abstract knowledge layer to the most specific?',
        'primenet-04': "A company cuts its budget. Which operation represents the change in the budget's amount?",
    })
    for q in bank['questions']:
        q['prompt'] = natural_stems.get(q['id'], q['prompt'])
        q.pop('noteZh', None)
    bank['source'] = {**baseline['source'], 'descriptionZh': '按五讲组织的英文单选练习，配中文解析、术语和逐页知识点索引。'}
    for key in ('pageConventionZh', 'examDisclaimerZh'):
        bank['source'].pop(key, None)
    for topic in bank['topics']:
        topic['lecture'] = 5 if topic['id'] in ('kr', 'primenet') else 4
    existing_ids = {q['id'] for q in bank['questions']}
    audit = {'knowledgePoints': [], 'pageAudit': []}
    for part in ('early', 'middle', 'late', 'adjustments'):
        authored = read(f'content-{part}.json')
        for q in authored['questions']:
            assert q['id'] not in existing_ids, q['id']
            order = list(range(4))
            random.Random('nlu-2026-10-03-' + q['id']).shuffle(order)
            q['options'] = [q['options'][i] for i in order]
            q['distractorsZh'] = [q['distractorsZh'][i] for i in order]
            q['answer'] = order.index(q['answer'])
            q['origin'] = 'authored'
        bank['topics'].extend(authored['topics'])
        bank['questions'].extend(authored['questions'])
        if part != 'adjustments':
            coverage = read(f'coverage-{part}.json')
            for key in audit:
                audit[key].extend(coverage[key])

    topics = {t['id']: t for t in bank['topics']}
    lessons = [lesson for part in ('early', 'tasks', 'late') for lesson in read(f'lessons-{part}.json')['lessons']]
    assert len(lessons) == len({lesson['topic'] for lesson in lessons}) == len(topics), 'Every topic needs one lesson'
    assert {lesson['topic'] for lesson in lessons} == set(topics), 'Lesson/topic mismatch'
    for lesson in lessons:
        topic = topics[lesson['topic']]
        assert lesson['introZh'].strip() and lesson['pitfallZh'].strip(), topic['id']
        assert 3 <= len(lesson['keyPoints']) <= 5, topic['id']
        assert all(point['title'].strip() and point['body'].strip() for point in lesson['keyPoints']), topic['id']
        assert lesson['example']['title'].strip() and lesson['example']['body'].strip(), topic['id']
        assert isinstance(lesson['formulas'], list) and all(formula['expression'].strip() and formula['explanationZh'].strip() for formula in lesson['formulas']), topic['id']
        assert lesson['pages'] and len(lesson['pages']) == len(set(lesson['pages'])), topic['id']
        assert all(isinstance(page, int) and 1 <= page <= 400 for page in lesson['pages']), topic['id']
        assert set(lesson['pages']) & set(topic['pages']), ('Lesson source mismatch', topic['id'])
        topic['lesson'] = {key: value for key, value in lesson.items() if key != 'topic'}
    qs = {q['id']: q for q in bank['questions']}
    assert len(topics) == len(bank['topics']), 'Duplicate topic id'
    assert len(qs) == len(bank['questions']), 'Duplicate question id'
    assert len({q['prompt'] for q in qs.values()}) == len(qs), 'Duplicate prompt'
    assert sorted(n for t in topics.values() for n in t['originalQuestions']) == list(range(1, 101))
    counts = Counter(q['topic'] for q in qs.values())
    for t in topics.values():
        assert 5 <= counts[t['id']] <= 8, (t['id'], counts[t['id']])
        assert t['lecture'] in range(1, 6), t['id']
        assert t['pages'] and all(isinstance(p, int) and 1 <= p <= 400 for p in t['pages']), t['id']
        assert t['focusZh'] and t['nameEn'] and t['nameZh'], t['id']
        t['questionCount'] = counts[t['id']]
    for q in qs.values():
        assert q['topic'] in topics and q['form'] in {f['id'] for f in bank['forms']}, q['id']
        assert len(q['options']) == len(set(q['options'])) == len(q['distractorsZh']) == 4, q['id']
        assert isinstance(q['answer'], int) and 0 <= q['answer'] < 4, q['id']
        assert all(text.strip() for i, text in enumerate(q['distractorsZh']) if i != q['answer']), q['id']
        assert q['explanationZh'].strip() and q['terms'] and q['difficulty'], q['id']
        assert all(t['en'].strip() and t['zh'].strip() for t in q['terms']), q['id']
        assert q['refs'] and all(isinstance(r['page'], int) and 1 <= r['page'] <= 400 and r['reasonZh'].strip() for r in q['refs']), q['id']
        assert isinstance(q['sourceQuestions'], list) and all(1 <= n <= 100 for n in q['sourceQuestions']), q['id']
        q['lecture'] = topics[q['topic']]['lecture']
        for ref in q['refs']:
            ref['lecture'] = lecture_for(ref['page'])

    points = {p['id']: p for p in audit['knowledgePoints']}
    rows = {p['page']: p for p in audit['pageAudit']}
    assert len(points) == len(audit['knowledgePoints']), 'Duplicate knowledge id'
    assert len(rows) == len(audit['pageAudit']) == 400 and sorted(rows) == list(range(1, 401)), 'Page audit must list 1–400 exactly once'
    for point in points.values():
        assert point['titleZh'].strip() and point['pages'] and point['questionIds'], point['id']
        assert len(point['questionIds']) == len(set(point['questionIds'])), point['id']
        assert all(p in rows for p in point['pages']), point['id']
        assert all(q in qs for q in point['questionIds']), point['id']
        assert set(point['pages']) & {r['page'] for qid in point['questionIds'] for r in qs[qid]['refs']}, ('Unrelated reference', point['id'])
        # Illustrations and background can support a concept without turning
        # their page-specific details into assessed knowledge.
        assert all(rows[p]['kind'] != 'content' or point['id'] in rows[p]['knowledgePointIds'] for p in point['pages']), ('Point missing from content page', point['id'])
    for row in rows.values():
        assert row['kind'] in ('content', 'resource', 'context', 'section', 'repeat') and row['summaryZh'].strip(), row['page']
        assert isinstance(row['knowledgePointIds'], list) and all(p in points for p in row['knowledgePointIds']), row['page']
        if row['kind'] == 'content':
            assert row['knowledgePointIds'], ('Unmapped content page', row['page'])
        if row['kind'] == 'repeat':
            assert row.get('repeatOf') in rows and row['repeatOf'] != row['page'], ('Missing repeat reference', row['page'])
        row['lecture'] = lecture_for(row['page'])
    for old in baseline['questions']:
        if old['id'] in RETIRED_IDS:
            assert old['id'] not in qs
            continue
        current = qs[old['id']]
        assert old['options'] == current['options'] and old['answer'] == current['answer'], ('Progress incompatibility', old['id'])
    bank['topics'].sort(key=lambda t: (t['lecture'], min(t['pages']), t['id']))
    topic_order = {t['id']: i for i, t in enumerate(bank['topics'])}
    bank['questions'].sort(key=lambda q: (topic_order[q['topic']], q['id']))
    audit['pageAudit'].sort(key=lambda p: p['page'])
    audit['knowledgePoints'].sort(key=lambda p: (min(p['pages']), p['id']))
    bank['coverage'] = audit
    return bank


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    default_root = ROOT.parent.parent if ROOT.parent.name == 'scripts' else ROOT
    parser.add_argument('--output', type=Path, default=default_root / 'site/static/nlu-practice/bank.json')
    args = parser.parse_args()
    bank = build()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(bank, ensure_ascii=False, indent=2) + '\n', encoding='utf8')
    print(f"PASS: {len(bank['questions'])} questions; {len(bank['topics'])} groups; {len(bank['coverage']['knowledgePoints'])} knowledge entries; all 400 pages audited")
    print('Questions by lecture:', dict(Counter(q['lecture'] for q in bank['questions'])))
    print('Answers:', dict(Counter(q['answer'] for q in bank['questions'])))
