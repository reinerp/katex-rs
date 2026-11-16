# mfctx.py OUTDIR : line box of a paragraph around an inline formula: KaTeX vs the font with several ascent/descent and strut choices
import sys, os, json, statistics, collections
sys.path.insert(0, '/home/agents/scratch/katex-font/tools'); sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import Encoder, to_html
out = sys.argv[1]; os.makedirs(out, exist_ok=True)
FA = '/home/agents/scratch/katex-font/outb/MathB'        # ascent = descent = 0.6 em
FB = '/home/agents/scratch/katex-font/outbA/MathBA'      # ascent 0.903, descent 0.272 (Main-Regular's)
enc = Encoder(FA + '.map.json')
G = [o for o in json.load(open('/home/agents/scratch/katex-font/data/glyphs_inline.json')) if 'g' in o and not o['r'] and not o['svg']]
rows = []
for o in G:
    r = enc.best(o['g'], o['box'][0])
    if r[1]: continue
    rows.append((o['id'], r[0], o['box']))
F = {f['id']: f['tex'] for f in map(json.loads, open('/home/agents/scratch/katex-harden/data/formulas.jsonl'))}
print('formulas', len(rows))
face = lambda nm, fb: '@font-face{font-family:%s;src:url(file://%s.woff2) format("woff2")}' % (nm, fb)
base = 'body{margin:0;font:16px/1.5 system-ui,sans-serif}.row{width:560px;padding:6px 20px;margin:0;border-bottom:1px solid #ccc}.bl{display:inline-block;width:0;height:0}.katex{text-rendering:geometricPrecision!important}.mf{text-rendering:geometricPrecision;white-space:nowrap;text-indent:0}'
variants = {
    'P': ('', ''),
    'K': ('', ''),
    'F0 asc=desc=.6, line-height:0': (face('FA', FA), '.mf{font:normal 1.21em/0 FA}'),
    'F1 asc=desc=.6, line-height:normal': (face('FA', FA), '.mf{font:normal 1.21em FA}'),
    'F2 asc=.903 desc=.272, line-height:normal': (face('FB', FB), '.mf{font:normal 1.21em FB}'),
    'F3 asc=desc=.6, line-height:0 + strut ::before': (face('FA', FA), '.mf{font:normal 1.21em/0 FA}.mf::before{content:"";display:inline-block;height:calc(var(--h) + var(--d));vertical-align:calc(-1 * var(--d))}'),
}
res = {}
with chrome(9601) as c:
    c.ws.call('Emulation.setDeviceMetricsOverride', width=700, height=400, deviceScaleFactor=1, mobile=False)
    for name, (ff, css) in variants.items():
        parts = []
        for i, cps, box in rows:
            tex = F[i]
            if name == 'P': f = 'xx'
            elif name == 'K': f = '__KATEX__'
            else: f = '<span class=mf style="--h:%.4fem;--d:%.4fem">%s</span>' % (box[1], box[2], to_html(cps))
            parts.append('<p class=row data-id=%d>Some words before, <i class=bl></i>%s and some words after, so that this line goes on for a while and wraps onto a second line.</p>' % (i, f))
        page = '<!doctype html><meta charset=utf-8><link rel=stylesheet href="file:///home/agents/scratch/katex-font/katex/katex.min.css"><style>%s%s%s</style>%s' % (base, ff, css, ''.join(parts))
        if name == 'K':
            page = page.replace('__KATEX__', '').replace('</style>', '</style>', 1) + '<script src="file:///home/agents/scratch/katex-font/katex/katex.min.js"></script><script>const T=%s;document.querySelectorAll(".row").forEach(r=>{const t=T[r.dataset.id];const bl=r.querySelector(".bl");const s=document.createElement("span");s.innerHTML=katex.renderToString(t,{output:"html",throwOnError:false});bl.after(s.firstChild)})</script>' % json.dumps({str(i): F[i] for i, _, _ in rows})
        open('%s/ctx_%s.html' % (out, name.split()[0]), 'w').write(page)
        c.goto('%s/ctx_%s.html' % (out, name.split()[0]))
        res[name] = {int(k): v for k, v in json.loads(c.ev('JSON.stringify(Object.fromEntries([...document.querySelectorAll(".row")].map(r=>{const b=r.getBoundingClientRect();const bl=r.querySelector(".bl").getBoundingClientRect();const f=r.querySelector(".katex,.mf");const fr=f?f.getBoundingClientRect():null;return [r.dataset.id,[b.height,bl.bottom-b.top,fr?fr.top-b.top:0,fr?fr.bottom-b.top:0,fr?fr.width:0]]})))')).items()}
json.dump(res, open(out + '/ctx.json', 'w'))
K = res['K']; P = res['P']
print('paragraph height (px) around a formula: reference text without formula = %s' % collections.Counter(v[0] for v in P.values()).most_common(2))
print('KaTeX: heights', sorted(collections.Counter(v[0] for v in K.values()).items()))
for name in variants:
    if name in ('P', 'K'): continue
    V = res[name]
    dh = [V[i][0] - K[i][0] for i in K]; db = [V[i][1] - K[i][1] for i in K]; dw = [V[i][4] - K[i][4] for i in K]
    print('%-48s height != KaTeX: %4d of %d (taller %d, shorter %d, max |d| %.1f px); baseline != KaTeX: %d (max %.2f px); paragraph height != no-formula text: %d' % (
        name, sum(1 for d in dh if abs(d) > 0.01), len(dh), sum(1 for d in dh if d > 0.01), sum(1 for d in dh if d < -0.01), max(abs(d) for d in dh), sum(1 for d in db if abs(d) > 0.02), max(abs(d) for d in db),
        sum(1 for i in K if abs(V[i][0] - P[i][0]) > 0.01)))
print('KaTeX itself: height != plain text paragraph: %d of %d' % (sum(1 for i in K if abs(K[i][0] - P[i][0]) > 0.01), len(K)))
