# mfcat.py DIR : pixel and ink-centroid statistics of mfhyb.py/mfhpix.py/mfcent.py results by kind of formula (DIR = hyb_inline or hyb_display)
import sys, json, re, statistics
d = sys.argv[1]
F = {f['id']: f for f in map(json.loads, open('/home/agents/scratch/katex-harden/data/formulas.jsonl'))}
m = json.load(open(d + '/metrics_px.json'))
c = {r: json.load(open('%s/cent%d.json' % (d, r))) for r in (1, 2)}
cats = [('plain glyphs and spacing (no ^ _ \\frac \\sqrt \\text \\begin)', lambda t: not re.search(r'[\^_]|\\frac|\\sqrt|\\text|\\begin|\\binom|\\over', t)),
        ('scripts (no stacks, no \\text)', lambda t: re.search(r'[\^_]', t) and not re.search(r'\\frac|\\sqrt|\\text|\\begin|\\binom|\\over|\\sum|\\lim', t)),
        ('\\frac, \\binom, \\begin', lambda t: re.search(r'\\frac|\\binom|\\begin', t)),
        ('\\text', lambda t: '\\text' in t),
        ('all', lambda t: True)]
def q(v): v = sorted(v); return '%.3f/%.3f/%.3f' % (statistics.median(v), v[int(.9 * len(v))], v[-1])
for name, f in cats:
    rows = [o for o in m if o.get('px1') is not None and o['wk'] > 0 and f(F[int(o['l'])]['tex'])]
    n = len(rows)
    if not n: continue
    print('%s: %d formulas' % (name, n))
    for dpr in (1, 2):
        print('  dpr%d: identical %.0f%%, no pixel > 64/255 different %.0f%%, median differing pixels %d; ink centroid |dx| median/p90/max %s, |dy| %s device px' % (
            dpr, 100 * sum(o['px%d' % dpr] == 0 for o in rows) / n, 100 * sum(o['big%d' % dpr] == 0 for o in rows) / n, sorted(o['px%d' % dpr] for o in rows)[n // 2],
            q([abs(c[dpr][o['l']]['dx']) for o in rows]), q([abs(c[dpr][o['l']]['dy']) for o in rows])))
