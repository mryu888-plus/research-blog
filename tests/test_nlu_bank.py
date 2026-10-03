"""Keep authored questions, page audit and published JSON in sync."""
import importlib.util
import json
from pathlib import Path
import unittest

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


if __name__ == '__main__':
    unittest.main()
