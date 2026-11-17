# mfunits.py OLD.map OLD.jsonl NEW.map NEW.jsonl : glyph positions in font units computed from the code points (sum of advances), old against new
import sys, json, collections, statistics
def load(mp):
    m = json.load(open(mp)); U = m['unitsPerEm']; info = {}
    for f, d in m['glyphs'].items():
        for ch, e in d.items():
            info[e['cp']] = ('g', (f, int(ch), 1.0, 0.0), 0, round(e['adv'] * U))
            if 'noic' in e: info[e['noic']] = ('g', (f, int(ch), 1.0, 0.0), 0, round((e['adv'] - e.get('ic', 0)) * U))
    for v in m['variants']:
        for k, (cp, adv) in v['glyphs'].items():
            f, ch = k.rsplit(':', 1); info[cp] = ('g', (f, int(ch), v['size'], v['shift']), 0, round(adv * U))
    for b in m.get('baked', []):
        f, ch, size, shift, L, R, cp, adv = b; info[cp] = ('g', (f, ch, size, shift), L, round(adv * U))
    for v in m['spacers']['named'].values(): info[v[0]] = ('s', None, 0, round(v[1] * U))
    for k, cp in m['spacers']['pos'].items(): info[cp] = ('s', None, 0, int(k))
    for k, cp in m['spacers']['neg'].items(): info[cp] = ('s', None, 0, -int(k))
    return info
def glyphs(info, html):
    x = 0; out = []
    for c in html:
        e = info.get(ord(c))
        if e is None: continue
        if e[0] == 'g': out.append((e[1], x + e[2]))
        x += e[3]
    return out, x
oi, ni = load(sys.argv[1]), load(sys.argv[3])
O = {json.loads(l)['id']: json.loads(l).get('html') for l in open(sys.argv[2])}; N = {json.loads(l)['id']: json.loads(l).get('html') for l in open(sys.argv[4])}
worst = []; diffs = collections.Counter()
for i in O:
    if not O[i] or not N.get(i): continue
    # the runs of a formula are separated by tags: compare run by run
    import re
    ro = re.split(r'<[^>]*>', O[i]); rn = re.split(r'<[^>]*>', N[i])
    ro = [t for t in ro if any(ord(c) in oi for c in t)]; rn = [t for t in rn if any(ord(c) in ni for c in t)]
    if len(ro) != len(rn): worst.append((i, 'runs', len(ro), len(rn))); continue
    for a, b in zip(ro, rn):
        ga, wa = glyphs(oi, a); gb, wb = glyphs(ni, b)
        sa = sorted(ga, key=lambda g: (g[0], g[1])); sb = sorted(gb, key=lambda g: (g[0], g[1]))
        if [g[0] for g in sa] != [g[0] for g in sb]: worst.append((i, 'glyphs')); continue
        for x, y in zip(sa, sb): diffs[y[1] - x[1]] += 1
        diffs['w%d' % (wb - wa)] += 1
print(sorted(((k, v) for k, v in diffs.items() if not isinstance(k, str)), key=lambda t: t[0]))
print({k: v for k, v in diffs.items() if isinstance(k, str)})
print(worst[:10])
