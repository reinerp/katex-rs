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
PROPS = ['height', 'vertical-align', 'top', 'margin-left', 'margin-right', 'min-width', 'width', 'border-bottom-width', 'padding-top']
a = [x for x in sys.argv[1:] if not x.startswith('--')]
even = '--even' in sys.argv
c = collections.Counter()
GEOM = ('padding-top', 'height', 'vertical-align')
ITEM = ('top', 'margin-left', 'margin-right')
groups = {'GEOMS': collections.Counter(), 'ITEMS': collections.Counter()}
struts = collections.Counter()
pstruts = collections.Counter()
for line in open(a[0]):
    j = json.loads(line)
    if 'html' not in j or (even and j['id'] % 2): continue
    for m in re.finditer(r'<span class="U" style="([^"]*)">', j['html']):
        d = dict(x.split(':', 1) for x in m.group(1).split(';') if ':' in x)
        if set(d) <= {'height', 'vertical-align'} and 'height' in d: struts[(d['height'], d.get('vertical-align', ''))] += 1
    for m in re.finditer(r'<span class="P" style="height:([^";]*)">', j['html']): pstruts[m.group(1)] += 1
    for m in re.finditer(r' style="([^"]*)"', j['html']):
        d = dict(x.split(':', 1) for x in m.group(1).split(';') if ':' in x)
        for gname, props in (('GEOMS', GEOM), ('ITEMS', ITEM)):
            if any(p in d for p in props):
                groups[gname][tuple(d.get(p, '') for p in props)] += 1
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
    vals = [v for v, n in sorted(((v, c[(p, v)]) for v in by[p]), key=lambda t: (-t[1], key(t[0])))]
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
def table(counter, name):
    items = [(k if isinstance(k, tuple) else (k, '')) for k, n in counter.most_common() if n >= min_count]
    body = ['const %s: &[(&str, &str)] = &[' % name] + ['    ("%s", "%s"),' % k for k in items] + ['];']
    return '\n'.join(body)
def table3(counter, name):
    items = [k for k, n in counter.most_common() if n >= min_count]
    return '\n'.join(['const %s: &[[&str; 3]] = &[' % name] + ['    [%s],' % ', '.join('"%s"' % x for x in k) for k in items] + ['];'])
s = open(SRC).read()
if '--values-only' not in sys.argv or '--groups' in sys.argv:
    for name in ('GEOMS', 'ITEMS'):
        s = re.sub(r'(// %s-BEGIN\n).*?(// %s-END)' % (name, name), lambda m: m.group(1) + table3(groups[name], name) + '\n' + m.group(2), s, flags=re.S)
open(SRC, 'w').write(s)
s = open(SRC).read()
for name, counter in (() if '--values-only' in sys.argv else (('STRUTS', struts), ('PSTRUTS', pstruts))):
    s = re.sub(r'(// %s-BEGIN\n).*?(// %s-END)' % (name, name), lambda m: m.group(1) + table(counter, name) + '\n' + m.group(2), s, flags=re.S)
open(SRC, 'w').write(s)
s = open(SRC).read()
s = re.sub(r'(// VALUES-BEGIN\n).*?(// VALUES-END)', lambda m: m.group(1) + '\n'.join(lines) + '\n' + m.group(2), s, flags=re.S)
open(SRC, 'w').write(s)
print({p: len(v) for p, v in by.items()}, 'total', sum(len(v) for v in by.values()), 'struts', len([1 for n in struts.values() if n >= min_count]), 'pstruts', len([1 for n in pstruts.values() if n >= min_count]))
