# mfpairs.py WHY.txt... --out EXTRA.json [--ids even|odd|all] [--min N] [--closure N] : (size, shift, glyphs) the Rust encoder needed and the font did not have
import sys, re, json, collections, argparse
ap = argparse.ArgumentParser(); ap.add_argument('why', nargs='+'); ap.add_argument('--out', required=True); ap.add_argument('--ids', default='all')
ap.add_argument('--min', type=int, default=1, help='a pair needed by at least this many formulas is added'); ap.add_argument('--spec', default='spec_mk.json', help='a spec of the build (its first pair has the closure glyph set)'); ap.add_argument('--closure', type=int, default=8, help='a pair needed by this many formulas gets the whole script glyph set')
a = ap.parse_args()
FONTS = ['Main-Regular', 'Math-Italic', 'AMS-Regular', 'Size1-Regular', 'Size2-Regular', 'Size3-Regular', 'Size4-Regular']
need = collections.defaultdict(lambda: collections.defaultdict(set))   # (size, shift) -> glyph -> formula ids
for path in a.why:
    cur = None
    for l in open(path):
        if l.startswith('ID '): cur = int(l.split()[1]); continue
        m = re.match(r'MISS (\d+) (\d+) (\d+) (-?\d+)', l)
        if not m or cur is None: continue
        if a.ids != 'all' and cur % 2 != (0 if a.ids == 'even' else 1): continue
        size = int(m.group(3)) / 1000; shift = int(m.group(4)) / 1000
        need[(size, shift)]['%s:%d' % (FONTS[int(m.group(1))], int(m.group(2)))].add(cur)
spec = json.load(open(a.spec))
closure = sorted(set(spec['pairs'][0][2]))
# shifts of one size within 0.003 em are one pair (a variant is used for shifts within 0.0015 em of it)
merged = {}
for (size, shift), gl in sorted(need.items()):
    for (s2, sh2) in list(merged):
        if s2 == size and abs(sh2 - shift) <= 0.003:
            tgt = merged[(s2, sh2)]
            for g, ids in gl.items(): tgt[g] |= ids
            break
    else:
        merged[(size, shift)] = collections.defaultdict(set, {g: set(ids) for g, ids in gl.items()})
need = merged
out = []
for (size, shift), gl in sorted(need.items()):
    formulas = set().union(*gl.values())
    if len(formulas) < a.min: continue
    glyphs = set(gl)
    if len(formulas) >= a.closure: glyphs |= set(closure)
    out.append([size, shift, sorted(glyphs)])
json.dump(out, open(a.out, 'w'))
print(len(out), 'pairs,', sum(len(o[2]) for o in out), 'variants')
