# fmtcheck.py FONTBASE : render every glyph of the font loaded as TTF / WOFF / WOFF2 / untransformed WOFF2 and compare pixels
import sys, json, os, subprocess
sys.path.insert(0, '/home/agents/scratch/katex-font/tools')
from cu import *
b = sys.argv[1]; m = json.load(open(b + '.map.json')); name = os.path.basename(b)
cps = []
for f, d in m['glyphs'].items():
    for ch, e in d.items(): cps.append(e['cp']); 
    for ch, e in d.items():
        if 'noic' in e: cps.append(e['noic'])
for v in m['variants']:
    for k, (cp, adv) in v['glyphs'].items(): cps.append(cp)
for r in m['rules']: cps.append(r['cp'])
for o in m['overlays']: cps.append(o['cp'])
cps = sorted(set(cps))
print(len(cps), 'glyphs')
variants = {'ttf': '.ttf', 'woff': '.woff', 'woff2': '.woff2', 'woff2u': '.untransformed.woff2'}
fmts = {'ttf': 'truetype', 'woff': 'woff', 'woff2': 'woff2', 'woff2u': 'woff2'}
cols = 40; rows = (len(cps) + cols - 1) // cols
for k, ext in variants.items():
    cells = ''.join('<span>%s</span>' % chr(c) for c in cps)
    open('/home/agents/scratch/katex-font/pages/fmt_%s.html' % k, 'w').write('<!doctype html><meta charset=utf-8><style>@font-face{font-family:F;src:url(file://%s%s) format("%s")}body{margin:0;font:28px/50px F;width:%dpx}span{display:inline-block;width:50px;height:50px;text-align:left;overflow:visible}</style>%s' % (os.path.abspath(b), ext, fmts[k], cols * 50, cells))
with chrome(9551) as c:
    c.ws.call('Emulation.setDeviceMetricsOverride', width=cols * 50, height=rows * 50, deviceScaleFactor=1, mobile=False)
    for k in variants:
        c.goto('/home/agents/scratch/katex-font/pages/fmt_%s.html' % k); c.shot('/home/agents/scratch/katex-font/pages/fmt_%s.png' % k)
        print(k, c.ev('JSON.stringify([...document.fonts].map(f=>f.status))'), c.console()[-1:] if False else '')
