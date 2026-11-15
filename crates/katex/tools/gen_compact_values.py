#!/usr/bin/env python3
"""Choose the inline declarations that get a class of their own in the compact output.

usage: gen_compact_values.py OUTPUT.jsonl MIN_COUNT [--even]

OUTPUT.jsonl is compact output rendered without the VALUE_CLASSES step (one JSON object per
line with an `id` and `html`). Every `property: value` pair that occurs at least MIN_COUNT times
(summed over formulas; with --even, over the even-numbered ones only, so the odd ones are held
out) is written to the VALUES table in src/compact.rs. Standard library only.
"""
import collections, json, os, re, sys

SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'src', 'compact.rs')
PROPS = ['height', 'vertical-align', 'top', 'margin-left', 'margin-right', 'min-width', 'width', 'border-bottom-width']
a = [x for x in sys.argv[1:] if not x.startswith('--')]
even = '--even' in sys.argv
c = collections.Counter()
for line in open(a[0]):
    j = json.loads(line)
    if 'html' not in j or (even and j['id'] % 2): continue
    for m in re.finditer(r' style="([^"]*)"', j['html']):
        for decl in m.group(1).split(';'):
            if ':' not in decl: continue
            p, v = decl.split(':', 1)
            if p in PROPS and re.fullmatch(r'-?(\d+\.?\d*|\.\d+)(em)?', v): c[(p, v)] += 1
min_count = int(a[1])
by = collections.defaultdict(list)
for (p, v), n in c.items():
    if n >= min_count: by[p].append(v)
def key(v): return (float(v[:-2] if v.endswith('em') else v), v)
lines = ['const VALUES: &[(&str, &[&str])] = &[']
for p in PROPS:
    vals = sorted(by[p], key=key)
    if not vals: continue
    lines.append('    (')
    lines.append('        "%s",' % p)
    lines.append('        &[')
    row = '            '
    for v in vals:
        item = '"%s", ' % v
        if len(row) + len(item) > 96: lines.append(row.rstrip()); row = '            '
        row += item
    lines.append(row.rstrip()); lines.append('        ],'); lines.append('    ),')
lines.append('];')
s = open(SRC).read()
s = re.sub(r'(// VALUES-BEGIN\n).*?(// VALUES-END)', lambda m: m.group(1) + '\n'.join(lines) + '\n' + m.group(2), s, flags=re.S)
open(SRC, 'w').write(s)
print({p: len(v) for p, v in by.items()}, 'total', sum(len(v) for v in by.values()))
