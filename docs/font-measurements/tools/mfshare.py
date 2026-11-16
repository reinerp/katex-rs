# mfshare.py RS_JSONL MODE [even|odd|all] : share of glyphs that the Rust output has in font runs; elements and bytes per formula
import sys, json, re, collections, statistics
sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from mathfont_encode import Encoder
import os
D = '/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/'
enc = Encoder([D + f for f in os.listdir(D) if f.endswith('.map.json')][0])
glyph = set(); 
for (f, c), (cp, a, ic, noic, alt) in enc.base.items():
    if c not in (0x20, 0xA0): glyph.add(cp); 
    if noic: glyph.add(noic)
for k, (cp, a) in enc.var.items(): glyph.add(cp)
rs, mode = sys.argv[1], sys.argv[2]; par = sys.argv[3] if len(sys.argv) > 3 else 'all'
G = {o['id']: o for o in json.load(open('/home/agents/scratch/katex-font/data/glyphs_%s.json' % mode)) if 'g' in o}
F = {f['id']: f for f in map(json.loads, open('/home/agents/scratch/katex-harden/data/formulas.jsonl'))}
tot = run = 0; elems = []; byts = []; full = 0; n = 0
for l in open(rs):
    j = json.loads(l)
    if 'html' not in j or j['id'] not in G: continue
    if mode == 'display' and not F[j['id']]['display']: continue
    if par != 'all' and j['id'] % 2 != (0 if par == 'even' else 1): continue
    h = j['html']; o = G[j['id']]
    t = len([g for g in o['g'] if g[1] not in (0x20, 0xA0)])
    r = sum(1 for c in h if ord(c) in glyph)
    tot += t; run += min(r, t); n += 1; elems.append(h.count('<span') + h.count('<wbr') + h.count('<svg') + h.count('<img')); byts.append(len(h.encode()))
    if r >= t: full += 1
print('%s %s: %d formulas; glyphs in runs %d of %d = %.1f%%; formulas with all glyphs in runs %d; elements per formula mean %.2f median %.0f p95 %.0f max %d; bytes mean %.0f' % (mode, par, n, run, tot, 100 * run / tot, full, statistics.mean(elems), statistics.median(elems), sorted(elems)[int(.95 * len(elems))], max(elems), statistics.mean(byts)))
