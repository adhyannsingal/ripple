import importlib.util
from pathlib import Path
import unittest
spec=importlib.util.spec_from_file_location('mutation',Path(__file__).with_name('run.py'))
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class Metrics(unittest.TestCase):
    def test_recall_and_selection_denominators_are_independent(self):
        r=m.metrics(['a','b'],['b','c'],5)
        self.assertEqual(r['recall'],{'numerator':1,'denominator':2,'fraction':.5})
        self.assertEqual(r['selection_ratio'],{'numerator':2,'denominator':5,'fraction':.4})
        self.assertEqual(r['missed'],['c'])
    def test_empty_failure_set_is_undefined(self):
        self.assertIsNone(m.metrics([],[],0)['recall']['fraction'])
        self.assertIsNone(m.fraction(0,0)['fraction'])
    def test_import_baseline_handles_cycles_and_side_effects(self):
        self.assertEqual(m.import_selection({'a':['b'],'b':['a'],'test':['a'],'dynamic':[]},['b'],['test','dynamic']),['test'])
