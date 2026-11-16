# mfanalyze.py DATA_DIR [inline|display]: pair coverage, script glyph use, gap clusters, font use over the corpus glyph lists
import sys, json, collections
mode = sys.argv[2] if len(sys.argv) > 2 else 'inline'
a = [o for o in json.load(open('%s/glyphs_%s.json' % (sys.argv[1], mode))) if 'g' in o]
glyph_only = [o for o in a if not o['r'] and not o['svg']]
print(mode, 'formulas', len(a), 'glyph-only (no rule/svg)', len(glyph_only), 'glyphs', sum(len(o['g']) for o in a))
print('sizes', collections.Counter(g[2] for o in a for g in o['g']).most_common(10))
print('fonts', collections.Counter(g[0] for o in a for g in o['g']).most_common())
pc = collections.Counter(); 
for o in a:
    for g in o['g']: pc[(g[2], round(g[4], 3))] += 1
pairs = [k for k, v in pc.most_common() if k != (1, 0)]
tot = sum(pc.values()); nb = pc[(1, 0)]
print('glyphs at (1,0): %d of %d = %.1f%%; distinct other pairs %d' % (nb, tot, 100 * nb / tot, len(pairs)))
print('N pairs | glyphs covered %% (incl. base) | formulas fully covered (of glyph-only) %% | of all')
fs = [(o, {(g[2], round(g[4], 3)) for g in o['g']} - {(1, 0)}) for o in glyph_only]
for N in (0, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 128, len(pairs)):
    S = set(pairs[:N]); gc = nb + sum(pc[k] for k in pairs[:N])
    fc = sum(1 for o, p in fs if p <= S)
    print('%5d | %5.1f | %5.1f | %5.1f' % (N, 100 * gc / tot, 100 * fc / len(fs), 100 * fc / len(a)))
# script glyph set: distinct (font,char) at size<1
sg = collections.Counter((g[0], g[1]) for o in a for g in o['g'] if g[2] < 1)
print('script glyphs: distinct', len(sg), 'occurrences', sum(sg.values()))
print('fonts in scripts', collections.Counter(f for (f, c), v in sg.items() for _ in [0]).most_common())
# gap clusters (positive and negative), values snapped to 8 units
import statistics
def canon(v):
    best = None
    for k in range(1, 73):
        for s in (1, 0.7, 0.5):
            c = k * 8000 / 18 * s
            if abs(abs(v) - c) <= 6 and (best is None or abs(abs(v) - c) < abs(abs(v) - best)): best = c
    for c in (8000 / 20, 8000 / 40, 8000 * 0.5, 8000 * 0.25, 8000):
        if abs(abs(v) - c) <= 6: best = c
    return best
# gaps: need the font advances, so this part uses the encoder with the map of a first build
if len(sys.argv) > 3:
    sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
    from mathfont_encode import Encoder
    e = Encoder(sys.argv[3])
    h = collections.Counter(); raw = collections.defaultdict(list)
    for o in glyph_only:
        for gp in e.gaps(o['g'], o['box'][0]):
            if abs(gp) >= 100:
                key = round(gp / 8)
                h[key] += 1; raw[key].append(gp)
    # merge adjacent bins
    cl = []
    for key, n in sorted(h.items()):
        if cl and key - cl[-1][1] <= 1: cl[-1][1] = key; cl[-1][2] += n; cl[-1][3] += raw[key]
        else: cl.append([key, key, n, list(raw[key])])
    cl = sorted(cl, key=lambda c: -c[2])
    print('gap clusters >=100 units (count, median units, em, canonical):')
    named = {}
    for c in cl[:40]:
        med = statistics.median(c[3]); cn = canon(med)
        print('  %5d  %8.1f  %.4f  %s' % (c[2], med, med / 8000, cn))
    tot = sum(c[2] for c in cl)
    print('large gaps total', tot, ' top8 %.1f%%  top16 %.1f%%  top24 %.1f%%' % tuple(100 * sum(c[2] for c in cl[:n]) / tot for n in (8, 16, 24)))
    json.dump([[statistics.median(c[3]), c[2], canon(statistics.median(c[3]))] for c in cl], open('%s/gap_clusters_%s.json' % (sys.argv[1], mode), 'w'))
# accents
trip = collections.Counter(); nacc = 0; multi = 0
for o in a:
    g = o['g']
    for i, x in enumerate(g):
        if x[6] == 'accbody':
            nacc += 1
            if i and g[i - 1][6] == 'accbase' and (i < 2 or g[i - 2][6] != 'accbase'):
                b = g[i - 1]
                trip[('%s:%d' % (b[0], b[1]), '%s:%d' % (x[0], x[1]), round(x[3] - b[3], 3), round(x[4] - b[4], 3), x[2])] += 1
            else: multi += 1
print('accents: occurrences', nacc, 'single-glyph base', sum(trip.values()), 'distinct combos', len(trip), 'multi/other', multi)
for k, v in trip.most_common(8): print('  ', v, k)
