# mfclasses.py RUNS.txt... : the gaps before and after the glyphs of the classes TeX spaces (binary operators, relations, punctuation, from KaTeX's symbol table) in the runs the Rust
# encoder wrote (MATHRUN_RUNS=1 of the harness); NAME = TeX's thin (T), medium (M), thick (K) space, the script space (S) and their sums, as the font units they are
import sys, collections, json, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mfgaps import load, adv, FONTS, UPEM
S = json.load(open('/home/agents/workspaces/katex-rs-font/crates/katex/data/symbols.json'))
cls = {}
for e in S:
    if e['mode'] != 'math' or e['font'] not in ('main', 'ams'): continue
    r = e['replace']
    if not r: continue
    if r.startswith('\\u'): r = chr(int(r[2:], 16))
    if len(r) != 1: continue
    cls[('Main-Regular' if e['font'] == 'main' else 'AMS-Regular', ord(r))] = e['group']
NAMES = ((667, 'T'), (889, 'M'), (1111, 'K'), (200, 'S'), (1089, 'M+S'), (1311, 'K+S'), (867, 'T+.2'), (8667, 'qq+T'), (1000, 'sp'), (4000, 'quad'))
def snap(g):
    for n, name in NAMES:
        if abs(g - n) <= 1: return name
    return g
cnt = collections.defaultdict(collections.Counter)
for gl, w in load(sys.argv[1:]):
    xs = [round(g[4] * UPEM) for g in gl]; W = round(w * UPEM)
    for k, g in enumerate(gl):
        if abs(g[2] - 1) > 1e-6 or abs(g[3]) > 1e-4: continue
        c = cls.get((FONTS[g[0]], g[1]))
        if c not in ('bin', 'rel', 'punct'): continue
        a, ic = adv(g)
        if a is None: continue
        L = xs[k] if k == 0 else None
        if k > 0:
            pa, _ = adv(gl[k - 1])
            if pa is not None: L = snap(xs[k] - (xs[k - 1] + pa))
        R = snap((xs[k + 1] if k + 1 < len(gl) else W) - (xs[k] + a))
        cnt[c][(L, R)] += 1
for c, cn in cnt.items():
    print(c, sum(cn.values()))
    for k, n in cn.most_common(16): print('   ', k, n)
