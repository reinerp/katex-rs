# mfspec.py DATA_DIR OUT_SPEC.json --fonts A,B --npairs N [--closure] [--tier N] [--gaps FILE]
# Builds a font spec from the corpus glyph lists: script variant pairs by frequency, script glyph sets, named spacers.
import sys, json, collections, argparse
ap = argparse.ArgumentParser(); ap.add_argument('data'); ap.add_argument('out')
ap.add_argument('--fonts', default='Main-Regular,Math-Italic,AMS-Regular,Size1-Regular'); ap.add_argument('--npairs', type=int, default=8)
ap.add_argument('--closure', type=int, default=4, help='the top N pairs get the full script glyph closure; the rest only the glyphs seen there')
ap.add_argument('--nspacers', type=int, default=14); ap.add_argument('--rules', action='store_true')
ap.add_argument('--ams-subset', action='store_true', help='AMS-Regular restricted to blackboard capitals, those seen in the corpus and a few common symbols')
ap.add_argument('--ascent', type=float, default=0.6); ap.add_argument('--descent', type=float, default=0.6)
a = ap.parse_args()
fonts = a.fonts.split(',')
data = [o for m in ('inline', 'display') for o in json.load(open('%s/glyphs_%s.json' % (a.data, m))) if 'g' in o]
pc = collections.Counter(); seen = collections.defaultdict(set)
for o in data:
    for g in o['g']:
        if g[0] not in fonts: continue
        k = (g[2], round(g[4], 3))
        pc[k] += 1; seen[k].add('%s:%d' % (g[0], g[1]))
pairs = [k for k, v in pc.most_common() if k != (1, 0)][:a.npairs]
# closure of glyphs that may be in scripts: all Math-Italic, ASCII and common symbols of Main-Regular
cl = set()
sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
import sfnt
for f in fonts:
    if f == 'Math-Italic':
        for c in sfnt.SourceFont('/home/agents/scratch/katex-font/fonts/KaTeX_Math-Italic.ttf').cmap:
            if c > 0x20: cl.add('Math-Italic:%d' % c)
    if f == 'Main-Regular':
        for c in list(range(0x21, 0x7F)) + [0xB1, 0xD7, 0xF7, 0x2032, 0x2212, 0x2217, 0x2264, 0x2265, 0x2260, 0x2208, 0x2192, 0x221E, 0x22C5, 0x2026, 0x2211, 0x2202, 0x2207, 0x3A3, 0x3A9, 0x394, 0x398, 0x3A0, 0x3A6, 0x3A8, 0x39B, 0x393, 0x39E, 0xB7, 0x2218, 0x2223, 0x2225, 0x2229, 0x222A, 0x2282, 0x2286, 0x2248, 0x223C, 0x2261]:
            cl.add('Main-Regular:%d' % c)
spec = {'fonts': fonts, 'ic_fonts': ['Math-Italic'], 'ascent': a.ascent, 'descent': a.descent, 'pairs': [], 'spacers': {}}
for i, (s, y) in enumerate(pairs):
    glyphs = sorted(seen[(s, y)] | (cl if i < a.closure and s < 1 else set()))
    spec['pairs'].append([s, y, glyphs])
# named spacers from the gap clusters
try:
    gc = json.load(open('%s/gap_clusters_inline.json' % a.data))
except OSError:
    gc = []
for med, n, cn in gc[:a.nspacers]:
    v = abs(cn if cn else med) / 8000
    spec['spacers']['%s%d' % ('n' if med < 0 else 'g', round(abs(med)))] = round(v if med > 0 else -v, 6)
if a.ams_subset:
    seen_ams = {int(k.split(':')[1]) for p in data for g in p['g'] for k in ['%s:%d' % (g[0], g[1])] if g[0] == 'AMS-Regular'}
    spec['subset'] = {'AMS-Regular': sorted(set(range(0x41, 0x5B)) | seen_ams | {0x210F, 0x2A7D, 0x2A7E, 0x2272, 0x2273, 0x2270, 0x2271, 0x2205, 0x25A1, 0x2713, 0x2235, 0x2234, 0x22A8, 0x21CC, 0x2216, 0x2201, 0x2204, 0x2224, 0x2226, 0x2220, 0x2127})}
json.dump(spec, open(a.out, 'w'), separators=(',', ':'))
print(len(pairs), 'pairs', sum(len(p[2]) for p in spec['pairs']), 'variants', len(spec['spacers']), 'named spacers')
