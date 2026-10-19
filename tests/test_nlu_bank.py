"""Keep authored questions, page audit and published JSON in sync."""
import importlib.util
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
BUILDER = ROOT / 'scripts/nlu/build_bank.py'
if not BUILDER.exists():
    BUILDER = ROOT / 'build_full_bank.py'
spec = importlib.util.spec_from_file_location('nlu_bank_builder', BUILDER)
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class NluBankTests(unittest.TestCase):
    def test_generated_bank_matches_authored_sources(self):
        expected = builder.build()
        actual = json.loads((ROOT / 'site/static/nlu-practice/bank.json').read_text(encoding='utf8'))
        self.assertEqual(expected, actual)

    def test_every_lecture_has_questions_and_every_page_has_audit(self):
        bank = builder.build()
        self.assertEqual({q['lecture'] for q in bank['questions']}, {1, 2, 3, 4, 5})
        self.assertEqual([p['page'] for p in bank['coverage']['pageAudit']], list(range(1, 401)))
        self.assertFalse(set(bank['retiredQuestionIds']) & {q['id'] for q in bank['questions']})

    def test_every_question_has_traceable_teaching_evidence(self):
        bank = builder.build()
        records = builder.validate_lesson_audit(bank)
        self.assertEqual(len(records), len(bank['questions']))
        self.assertEqual({record['questionId'] for record in records}, {question['id'] for question in bank['questions']})

    def test_teaching_audit_rejects_missing_duplicate_and_untraceable_records(self):
        bank = {'questions': [{'id': 'q1', 'topic': 't1'}],
                'topics': [{'id': 't1', 'lesson': {'introZh': 'A worked example teaches the necessary inference.'}}]}
        row = {'questionId': 'q1', 'topic': 't1', 'requiredConcepts': ['inference'],
               'priorGapZh': 'Missing the inference rule', 'reasoningZh': 'Apply the stated rule',
               'evidence': ['teaches the necessary inference']}
        def run(records):
            with patch.object(builder, 'read', side_effect=[{'questions': records}, {'questions': []}, {'questions': []}]):
                return builder.validate_lesson_audit(bank)
        self.assertEqual(run([row]), [row])
        invalid = [[], [row, row]]
        for field, value in [('topic', 'other'), ('questionId', 'unknown'),
                             ('requiredConcepts', []), ('reasoningZh', ''),
                             ('evidence', ['This phrase is absent from the lesson'])]:
            altered = copy.deepcopy(row)
            altered[field] = value
            invalid.append([altered])
        for records in invalid:
            with self.subTest(records=records), self.assertRaises(AssertionError):
                run(records)


if __name__ == '__main__':
    unittest.main()
