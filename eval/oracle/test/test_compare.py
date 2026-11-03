import copy
import sys
from pathlib import Path
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare import compare, ratio, score


class MetricsTests(unittest.TestCase):
    def test_known_true_false_missed_and_candidate_counts(self):
        m = score(['a', 'b', 'c'], ['a', 'b', 'x', 'x'], ['c'])
        self.assertEqual(m['precision'], ratio(2, 3))
        self.assertEqual(m['recall'], ratio(2, 3))
        self.assertEqual(m['candidate_coverage'], ratio(1, 3))
        self.assertEqual(m['false_positive'], ['x'])
        self.assertEqual(m['false_negative'], ['c'])

    def test_empty_denominators_are_undefined(self):
        self.assertIsNone(score([], [])['precision']['value'])
        self.assertEqual(score(['a'], [])['recall']['value'], 0)
        self.assertIsNone(score([], ['a'])['recall']['value'])

    def test_missing_mapping_never_erases_oracle_recall_denominator(self):
        oracle = {'policy': 'test', 'symbols': [], 'edges': [{'source': 's', 'target': 't', 'kind': 'CALLS', 'occurrences': [{'file': 'a.ts', 'span': {'start': 1, 'end': 2}}]}], 'occurrences': [], 'exclusions': []}
        ripple = {'symbols': [], 'edges': [], 'unresolved': [], 'files': []}
        m = compare(oracle, ripple)
        self.assertEqual(m['metrics']['recall'], ratio(0, 1))
        self.assertEqual(m['ranked_categories'][0]['category'], 'missing_syntax_reference')
        self.assertEqual(compare(copy.deepcopy(oracle), ripple), m)


    def test_wrong_target_is_false_positive_and_miss_candidates_separate(self):
        def sym(name, start):
            return {'identity': {'path': 'a.ts', 'kind': 'FUNCTION', 'qualified_name': [name]}, 'span': {'start': {'byte': start}, 'end': {'byte': start + 9}}}
        syms = [sym('f', 0), sym('g', 10), sym('h', 20)]
        osyms = [{'file': 'a.ts', 'kind': 'FUNCTION', 'span': {'start': s['span']['start']['byte'], 'end': s['span']['end']['byte']}, 'key': s['identity']['qualified_name'][0]} for s in syms]
        loc = {'file': 'a.ts', 'span': {'start': 3, 'end': 4}}
        oracle = {'policy': 'test', 'symbols': osyms, 'edges': [{'source': 'f', 'target': 'g', 'kind': 'CALLS', 'occurrences': [loc]}], 'occurrences': [{'location': loc}], 'exclusions': []}
        edge = {'source': {'kind': 'SYMBOL', 'identity': syms[0]['identity']}, 'target': {'kind': 'SYMBOL', 'identity': syms[2]['identity']}, 'kind': 'CALLS', 'confidence': 'RESOLVED', 'provenance': loc, 'resolution_rule': 'bad_rule'}
        candidate = copy.deepcopy(edge)
        candidate['target']['identity'] = syms[1]['identity']
        candidate['confidence'] = 'CANDIDATE'
        ripple = {'symbols': syms, 'edges': [edge, candidate, edge], 'unresolved': [], 'files': []}
        result = compare(oracle, ripple)
        self.assertEqual(result['metrics']['precision'], ratio(0, 1))
        self.assertEqual(result['metrics']['recall'], ratio(0, 1))
        self.assertEqual(result['metrics']['candidate_coverage'], ratio(1, 1))
        self.assertEqual(len(result['mismatches']), 2)
        # Ambiguous endpoint mapping is visible and does not remove the expected edge.
        ripple['symbols'].append(syms[0])
        oracle['symbols'].append(osyms[0])
        result = compare(oracle, ripple)
        self.assertGreater(result['counts']['unmapped_eligible_ripple_edges'], 0)
        self.assertEqual(result['metrics']['recall']['denominator'], 1)


if __name__ == '__main__':
    unittest.main()
