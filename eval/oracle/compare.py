"""Independent, semantic-pair comparison. No SQL row IDs enter metrics."""
import collections
import json


def stable(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':'))


def ratio(n, d):
    return {'numerator': n, 'denominator': d, 'value': n / d if d else None}


def score(expected, resolved, candidates=()):
    expected, resolved, candidates = set(expected), set(resolved), set(candidates)
    tp, fp, fn = expected & resolved, resolved - expected, expected - resolved
    return {'precision': ratio(len(tp), len(resolved)), 'recall': ratio(len(tp), len(expected)),
            'candidate_coverage': ratio(len(expected & candidates), len(expected)),
            'true_positive': sorted(tp), 'false_positive': sorted(fp), 'false_negative': sorted(fn)}


def location(record):
    loc = record.get('provenance', record.get('location', record))
    return (loc['file'], loc['span']['start'].get('byte') if isinstance(loc['span']['start'], dict) else loc['span']['start'],
            loc['span']['end'].get('byte') if isinstance(loc['span']['end'], dict) else loc['span']['end'])


def compare(oracle, ripple):
    symbols = ripple['symbols']
    mapping, exclusions = {}, []
    for s in symbols:
        identity = s['identity']
        matches = [o for o in oracle['symbols'] if o['file'] == identity['path'] and o['kind'] == identity['kind']
                   and o['span'] == {'start': s['span']['start']['byte'], 'end': s['span']['end']['byte']}]
        if len(matches) == 1:
            mapping[stable(identity)] = matches[0]['key']
        else:
            exclusions.append({'identity': identity, 'reason': 'unmatched_definition' if not matches else 'ambiguous_definition'})

    def endpoint(node):
        if node['kind'] == 'FILE':
            return json.dumps([node['identity']['path'], 'FILE'], ensure_ascii=False, separators=(',', ':'))
        return mapping.get(stable(node['identity'])) if node['kind'] == 'SYMBOL' else None

    expected = {stable([e['source'], e['target'], e['kind']]): e for e in oracle['edges']}
    eligible = {location(o): o for o in oracle['occurrences']}
    predicted, candidate, out_of_scope, rules = set(), set(), [], collections.defaultdict(set)
    mapping_edges = []
    for e in ripple['edges']:
        if e['kind'] not in ('CALLS', 'REFERENCES'):
            continue
        if location(e) not in eligible:
            out_of_scope.append(e)
            continue
        source, target = endpoint(e['source']), endpoint(e['target'])
        if source is None or target is None:
            mapping_edges.append(e)
            continue
        key = stable([source, target, e['kind']])
        if e['confidence'] == 'RESOLVED':
            predicted.add(key)
            rules[key].add(e['resolution_rule'])
        elif e['confidence'] == 'CANDIDATE':
            candidate.add(key)
    metrics = score(expected, predicted, candidate)
    unresolved = {location(u): u['reason'] for u in ripple['unresolved']}
    refs = {location({'provenance': {'file': f['path'], 'span': r['span']}}) for f in ripple['files'] for r in f['references']}
    mismatches = []
    for key in metrics['false_negative']:
        e = expected[key]
        reasons = sorted({unresolved[location(o)] for o in e['occurrences'] if location(o) in unresolved})
        missing_facts = all(location(o) not in refs for o in e['occurrences'])
        category = 'unresolved: ' + ' | '.join(reasons) if reasons else 'missing_syntax_reference' if missing_facts else 'missing_semantic_edge'
        mismatches.append({'type': 'missed', 'edge': json.loads(key), 'category': category, 'occurrences': e['occurrences']})
    for key in metrics['false_positive']:
        mismatches.append({'type': 'false_resolved', 'edge': json.loads(key), 'category': 'rule: ' + ','.join(sorted(rules[key]))})
    categories = collections.Counter(m['category'] for m in mismatches)
    return {'format_version': 1, 'policy': oracle['policy'], 'metrics': metrics,
            'counts': {'oracle_edges': len(expected), 'resolved_comparable_edges': len(predicted), 'candidate_comparable_edges': len(candidate),
                       'unresolved_occurrences': len(ripple['unresolved']), 'oracle_excluded_occurrences': len(oracle['exclusions']),
                       'unmapped_ripple_definitions': len(exclusions), 'unmapped_eligible_ripple_edges': len(mapping_edges), 'out_of_scope_ripple_edges': len(out_of_scope)},
            'mismatches': sorted(mismatches, key=stable), 'ranked_categories': [{'category': k, 'count': v} for k, v in sorted(categories.items(), key=lambda x: (-x[1], x[0]))],
            'mapping_exclusions': exclusions, 'unmapped_edges': mapping_edges, 'out_of_scope_edges': out_of_scope}


def markdown(report):
    lines = ['# Resolver comparison', '', '| Metric | Numerator | Denominator | Value |', '|---|---:|---:|---:|']
    for name in ('precision', 'recall', 'candidate_coverage'):
        m = report['metrics'][name]
        value = 'undefined (empty denominator)' if m['value'] is None else f"{m['value']:.6f}"
        lines.append(f"| {name} | {m['numerator']} | {m['denominator']} | {value} |")
    lines.extend(['', 'Counts: `' + stable(report['counts']) + '`', '', '## Mismatch categories', ''])
    lines.extend(f"- {c['category']}: {c['count']}" for c in report['ranked_categories'])
    lines.extend(['', 'This fixed source-projection result is not full TypeScript accuracy or a test-sufficiency guarantee. Mapping loss and excluded scope remain in JSON; missing oracle edges stay in recall. Performance is not measured.', ''])
    return '\n'.join(lines)
