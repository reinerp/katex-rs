# mfnamed.py OUT.json SPACES.txt... [--max N] [--min-gain G] : the named spacers of a font, chosen from the distances (font units) that the Rust
# encoder wrote spacers for (MATHRUN_SPACES=1 of the harness): greedily the distance that saves most code points, as long as it saves G or more.
import sys, json, collections, argparse
ap = argparse.ArgumentParser(); ap.add_argument('out'); ap.add_argument('spaces', nargs='+'); ap.add_argument('--max', type=int, default=80); ap.add_argument('--min-gain', type=int, default=30)
ap.add_argument('--upem', type=int, default=4000); a = ap.parse_args()
req = collections.Counter()
for p in a.spaces:
    for l in open(p):
        if l.startswith('SPACE '): req[int(l.split()[1])] += 1
TOL = 1
def pc(n):
    n = abs(n); return (n >> 14) + bin(n & 16383).count('1')
def cost(units, named):
    if abs(units) <= TOL: return 0
    best = pc(units + (1 if units % 2 and units > 0 else 0))  # placeholder, replaced below
    best = None
    for n in [0] + [n for n in named if (n > 0) == (units > 0) and abs(n) <= abs(units) + TOL]:
        rem = units - n
        if abs(rem) <= TOL: rem = 0
        elif abs(rem) % 2 == 1: rem += 1 if rem > 0 else -1
        c = (1 if n else 0) + pc(rem)
        if best is None or c < best: best = c
    return best
named = []
cur = {v: cost(v, named) for v in req}
print('requests', sum(req.values()), 'distinct', len(req), 'code points now', sum(cur[v] * n for v, n in req.items()))
cands = [v for v, n in req.items() if n >= 3 and abs(v) > TOL]
while len(named) < a.max:
    best = None
    for c in cands:
        if c in named: continue
        gain = 0
        for v, n in req.items():
            if (c > 0) != (v > 0) or abs(c) > abs(v) + TOL: continue
            nc = cost(v, named + [c])
            if nc < cur[v]: gain += (cur[v] - nc) * n
        if best is None or gain > best[0]: best = (gain, c)
    if best is None or best[0] < a.min_gain: break
    named.append(best[1])
    for v in req: cur[v] = cost(v, named)
    print('named %d: %d saves %d' % (len(named), best[1], best[0]))
print('code points after', sum(cur[v] * n for v, n in req.items()))
named.sort()
json.dump({'named': named, 'em': {str(n): n / a.upem for n in named}}, open(a.out, 'w'))
