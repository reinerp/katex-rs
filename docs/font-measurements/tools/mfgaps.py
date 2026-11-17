# mfgaps.py RUNS.txt... : the gaps between the glyphs of the runs the Rust encoder wrote (MATHRUN_RUNS=1 of the harness): what spacing is
# in the data. Prints the histogram of gaps and, per glyph, the (left gap, right gap) pairs.
import sys, json, os, collections
D = '/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/'
mapf = [D + f for f in os.listdir(D) if f.endswith('.map.json')][0]
m = json.load(open(mapf)); UPEM = m['unitsPerEm']
FONTS = ['Main-Regular', 'Math-Italic', 'AMS-Regular', 'Size1-Regular', 'Size2-Regular', 'Size3-Regular', 'Size4-Regular']
base = {}
for f, d in m['glyphs'].items():
    for ch, e in d.items(): base[(f, int(ch))] = (round(e['adv'] * UPEM), round(e.get('ic', 0) * UPEM))
var = {}
for v in m['variants']:
    for k, (cp, adv) in v['glyphs'].items():
        f, ch = k.rsplit(':', 1); var[(f, int(ch), round(v['size'] * 1000), round(v['shift'] * 1000))] = round(adv * UPEM)
def adv_of(g):
    f = FONTS[g[0]]; ch = g[1]; size = round(g[2] * 1000); y = round(g[3] * 1000)
    if size == 1000 and abs(y) < 1: return base.get((f, ch), (None, 0))
    for (ff, c, s, sh), a in var.items():
        pass
    return None, 0
varidx = collections.defaultdict(list)
for (f, c, s, sh), a in var.items(): varidx[(f, c, s)].append((sh, a))
def adv(g):
    f = FONTS[g[0]]; ch = g[1]; size = round(g[2] * 1000); y = round(g[3] * 1000)
    if size == 1000 and abs(y) <= 1: return base.get((f, ch), (None, 0))
    best = None
    for sh, a in varidx.get((f, ch, size), []):
        if abs(sh - y) <= 2 and (best is None or abs(sh - y) < best[0]): best = (abs(sh - y), a)
    if best: return best[1], 0
    return None, 0
def load(paths):
    runs = []
    for p in paths:
        for l in open(p):
            if l.startswith('RUN '):
                g, w = l[4:].rsplit(' ', 1)
                runs.append((json.loads(g), float(w)))
    return runs
if __name__ == '__main__':
    runs = load(sys.argv[1:])
    gaps = collections.Counter(); per = collections.defaultdict(collections.Counter)
    for gl, w in runs:
        xs = [round(g[4] * UPEM) for g in gl]
        for k in range(len(gl)):
            a, ic = adv(gl[k])
            if a is None: continue
            end = xs[k] + a
            if k + 1 < len(gl):
                nxt = xs[k + 1]
                gap = nxt - end
                gaps[gap] += 1
    tot = sum(gaps.values()); print('adjacent pairs', tot)
    for gap, n in gaps.most_common(40): print('%7d  %6d  %.4f em' % (gap, n, gap / UPEM))
