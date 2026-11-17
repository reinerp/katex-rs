# mfscriptspace.py OUT.json RUNS.txt... [--cover 0.95] : the (size, shift) pairs of the script glyphs that the script space (0.05 em) follows, most frequent first until
# they are COVER of all of them, from the runs the Rust encoder wrote (MATHRUN_RUNS=1 of the harness)
import sys, json, collections, argparse
sys.path.insert(0, '/home/agents/scratch/katex-font/tools')
ap = argparse.ArgumentParser(); ap.add_argument('out'); ap.add_argument('runs', nargs='+'); ap.add_argument('--cover', type=float, default=0.95); a = ap.parse_args()
from mfgaps import load, adv, UPEM
SPACE = round(0.05 * UPEM)
base = (0, 667, 889, 1111, 1000, 4000, 8000, 8667)      # a gap after a script that is the script space and what TeX puts between the atoms
cnt = collections.Counter()
for gl, w in load(a.runs):
    xs = [round(g[4] * UPEM) for g in gl]; end = round(w * UPEM)
    for k, g in enumerate(gl):
        if g[2] >= 1 - 1e-6: continue
        av, _ = adv(g)
        if av is None: continue
        gap = (xs[k + 1] if k + 1 < len(gl) else end) - (xs[k] + av)
        if any(abs(gap - (b + SPACE)) <= 1 for b in base): cnt[(round(g[2], 3), round(g[3], 3))] += 1
tot = sum(cnt.values()); acc = 0; out = []
for (size, shift), n in cnt.most_common():
    if acc >= a.cover * tot: break
    out.append([size, shift, 0.05]); acc += n
print('script glyphs followed by the script space:', tot, '; pairs', len(out), 'cover', acc / max(tot, 1))
json.dump(out, open(a.out, 'w'))
