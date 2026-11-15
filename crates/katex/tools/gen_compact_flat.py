#!/usr/bin/env python3
"""Choose the struts of whole formulas (FSTRUTS) and the offsets of shifted items (SHIFTS) that get a class.

usage: gen_compact_flat.py OUTPUT.jsonl MIN_COUNT [--even]

OUTPUT.jsonl is compact output rendered without VALUE_CLASSES and COMBO (one JSON object per
line with an `id` and `html`) and with the FLAT and SINGLE steps on. Standard library only. The
other tables (gen_compact_values.py) are not touched, so the output of the first design does not
change.
"""
import collections, json, os, re, sys

SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'src', 'compact.rs')
a = [x for x in sys.argv[1:] if not x.startswith('--')]
even = '--even' in sys.argv
min_count = int(a[1])
struts = collections.Counter()
shifts = collections.Counter()
for line in open(a[0]):
    j = json.loads(line)
    if 'html' not in j or (even and j['id'] % 2): continue
    h = j['html']
    for m in re.finditer(r'<span class="katex f"[^>]*? style="([^"]*)"', h):
        d = dict(x.split(':', 1) for x in m.group(1).split(';') if ':' in x)
        struts[(d['--h'], d.get('--v', ''))] += 1
    for m in re.finditer(r' style="([^"]*line-height:0[^"]*)"', h):
        d = dict(x.split(':', 1) for x in m.group(1).split(';') if ':' in x)
        shifts[tuple(d.get(p, '') for p in ('vertical-align', 'margin-left', 'margin-right'))] += 1
s = open(SRC).read()
body = ['const FSTRUTS: &[(&str, &str)] = &['] + ['    ("%s", "%s"),' % k for k, n in struts.most_common() if n >= min_count] + ['];']
s = re.sub(r'(// FSTRUTS-BEGIN\n).*?(// FSTRUTS-END)', lambda m: m.group(1) + '\n'.join(body) + '\n' + m.group(2), s, flags=re.S)
body = ['const SHIFTS: &[[&str; 3]] = &['] + ['    [%s],' % ', '.join('"%s"' % x for x in k) for k, n in shifts.most_common() if n >= min_count] + ['];']
s = re.sub(r'(// SHIFTS-BEGIN\n).*?(// SHIFTS-END)', lambda m: m.group(1) + '\n'.join(body) + '\n' + m.group(2), s, flags=re.S)
open(SRC, 'w').write(s)
print('fstruts', sum(1 for n in struts.values() if n >= min_count), 'of', len(struts), 'shifts', sum(1 for n in shifts.values() if n >= min_count), 'of', len(shifts))
