# mfhyb.py RS_JSONL MODE OUTDIR [--dpr 1,2] [--ids a,b|all] : the hybrid output against KaTeX JS for every formula:
# widths, glyph x (runs and stock text), glyph y (runs), tops of stock text, pixels
import sys, os, json, re, argparse, statistics, collections
sys.path.insert(0, '/home/agents/scratch/katex-font/tools'); sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import Encoder
ap = argparse.ArgumentParser(); ap.add_argument('rs'); ap.add_argument('mode'); ap.add_argument('out'); ap.add_argument('--dpr', default='1'); ap.add_argument('--ids', default='all')
ap.add_argument('--map', default=''); ap.add_argument('--font', default=''); ap.add_argument('--port', type=int, default=9641); ap.add_argument('--rowh', type=int, default=80)
ap.add_argument('--nopix', action='store_true'); ap.add_argument('--ktauto', action='store_true'); ap.add_argument('--extra', default=''); ap.add_argument('--css', default='/home/agents/scratch/katex-font/rsout/font.css')
ap.add_argument('--custom', default='', help='file of TeX, one formula per line, rendered by the Rust harness into RS (ids 100000+)')
a = ap.parse_args(); os.makedirs(a.out, exist_ok=True)
D = '/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/'
mapf = a.map or [D + f for f in os.listdir(D) if f.endswith('.map.json')][0]
fontf = a.font or mapf.replace('.map.json', '.woff2')
enc = Encoder(mapf)
rev = {}
for (f, c), (cp, ad, ic, noic, alt) in enc.base.items():
    rev[cp] = (c, 1.0, 0.0)
    if noic: rev[noic] = (c, 1.0, 0.0)
for (f, c, s, y), (cp, ad) in enc.var.items(): rev[cp] = (c, s, y)
BAKEDL = {}
for b in json.load(open(mapf)).get('baked', []):
    rev[b[6]] = (b[1], b[2], b[3]); BAKEDL[b[6]] = b[4]
UPEM = json.load(open(mapf))['unitsPerEm']
F = {f['id']: f for f in map(json.loads, open('/home/agents/scratch/katex-harden/data/formulas.jsonl'))}
if a.custom:
    for k, t in enumerate(open(a.custom)):
        F[100000 + k] = {'id': 100000 + k, 'tex': t.rstrip('\n'), 'display': a.mode == 'display'}
rows = []
for l in open(a.rs):
    j = json.loads(l)
    if 'html' not in j: continue
    if a.mode == 'display' and not F[j['id']]['display']: continue
    if a.ids != 'all' and str(j['id']) not in a.ids.split(','): continue
    rows.append((j['id'], j['html']))
print('rows', len(rows))
css = open(a.css).read().replace('url(MathRun.woff2)', 'url(file://%s)' % os.path.abspath(fontf)).replace('url(fonts/', 'url(file:///home/agents/scratch/katex-compact/css/fonts/')
base = 'body{margin:0;font:16px/%dpx system-ui,sans-serif;background:#fff;color:#000}.row{height:%dpx;line-height:%dpx;white-space:nowrap;padding:0 20px;box-sizing:border-box;overflow:hidden;position:relative;width:max-content;min-width:100%%}' % (a.rowh, a.rowh, a.rowh)
disp = a.mode == 'display'
fixd = '.katex-display{margin:0!important}' if disp else ''
CH = 40
GEOMJS = r'''
window.charsOf = function(k, kr) {
  const out = []; const rg = document.createRange(); const w = document.createTreeWalker(k, NodeFilter.SHOW_TEXT); let n;
  const fs = parseFloat(getComputedStyle(k).fontSize);
  while (n = w.nextNode()) {
    const px = parseFloat(getComputedStyle(n.parentElement).fontSize); const t = n.nodeValue;
    for (let i = 0; i < t.length; i++) {
      const cp = t.codePointAt(i); const len = cp > 0xffff ? 2 : 1;
      if (cp === 0x200b) { if (len == 2) i++; continue; }
      rg.setStart(n, i); rg.setEnd(n, i + len); const b = rg.getBoundingClientRect(); if (len == 2) i++;
      out.push([cp, +((b.left - kr.left) / fs).toFixed(5), +((b.top - kr.top) / fs).toFixed(5), +(px / fs).toFixed(4)]);
    }
  }
  return out;
};
window.rowsInfo = function() { return JSON.stringify([...document.querySelectorAll('.row')].map(r => {
  const k = r.querySelector('.katex'); if (!k) return [r.dataset.l, 0, [], 0]; const b = k.getBoundingClientRect();
  return [r.dataset.l, b.width / parseFloat(getComputedStyle(k).fontSize), charsOf(k, b), k.getElementsByTagName('*').length + 1]; })); };
'''
for ci in range(0, len(rows), CH):
    part = rows[ci:ci + CH]
    kp = '<!doctype html><meta charset=utf-8><link rel=stylesheet href="file:///home/agents/scratch/katex-font/katex/katex.min.css"><style>%s%s%s%s</style><div id=c></div><script src="file:///home/agents/scratch/katex-font/katex/katex.min.js"></script><script>%s const T=%s;const c=document.getElementById("c");for(const [l,t] of T){const d=document.createElement("div");d.className="row";d.dataset.l=l;d.innerHTML=katex.renderToString(t,{displayMode:%s,output:"html",throwOnError:false,maxExpand:1000});c.appendChild(d)}</script>' % (base, '' if a.ktauto else '.katex{text-rendering:geometricPrecision!important}', fixd, a.extra, GEOMJS, json.dumps([[i, F[i]['tex']] for i, _ in part]), 'true' if disp else 'false')
    fp = '<!doctype html><meta charset=utf-8><style>%s%s%s%s</style><script>%s</script><div id=c>%s</div>' % (base, css, fixd, a.extra, GEOMJS, ''.join('<div class=row data-l="%s">%s</div>' % (i, h.replace('\u200b', '')) for i, h in part))
    open('%s/k_%03d.html' % (a.out, ci // CH), 'w').write(kp); open('%s/f_%03d.html' % (a.out, ci // CH), 'w').write(fp)
metr = {}
with chrome(a.port) as c:
    for dpr in [int(x) for x in a.dpr.split(',')]:
        c.ws.call('Emulation.setDeviceMetricsOverride', width=1400, height=CH * a.rowh, deviceScaleFactor=dpr, mobile=False)
        for ci in range(0, len(rows), CH):
            n = ci // CH
            for kind in 'kf':
                c.goto('%s/%s_%03d.html' % (a.out, kind, n))
                if not a.nopix: c.shot('%s/%s_%03d_dpr%d.png' % (a.out, kind, n, dpr))
                if dpr == 1: metr[(kind, n)] = json.loads(c.ev('rowsInfo()'))
    print('console', [x[:100] for x in c.console()[:3]])
out = []
for ci in range(0, len(rows), CH):
    n = ci // CH
    for j, (i, h) in enumerate(rows[ci:ci + CH]):
        kl, kc, kn = metr[('k', n)][j][1], metr[('k', n)][j][2], metr[('k', n)][j][3]
        fl, fc, fn = metr[('f', n)][j][1], metr[('f', n)][j][2], metr[('f', n)][j][3]
        fs = 19.36
        # KaTeX glyphs: (char, size) -> [(x, top)] ; hybrid: runs (PUA) map to (char, size, shift) ; stock text as is
        kt = collections.defaultdict(list); ft = collections.defaultdict(list); ftop = collections.defaultdict(list)
        for cp, x, top, s in kc:
            if cp in (0x20, 0xA0): continue
            kt[(cp, round(s, 2))].append((x, top))
        nrun = nstock = 0; dy_run = 0.0
        for cp, x, top, s in fc:
            if cp in (0x20, 0xA0): continue
            if 0xE000 <= cp <= 0xF8FF or cp >= 0xF0000:
                if cp not in rev: continue   # a spacer glyph
                ch, sz, y = rev[cp]
                ft[(ch, round(sz * s, 2))].append((x + BAKEDL.get(cp, 0) / UPEM * s, None)); nrun += 1
            else:
                ft[(cp, round(s, 2))].append((x, top)); nstock += 1
        dx = dtop = 0.0; ok = True; unmatched = 0; bad = []
        for k, xs in ft.items():
            ks = kt.get(k)
            if ks is None or len(ks) != len(xs): ok = False; unmatched += len(xs); continue
            runs = [p for p in xs if p[1] is None]; stock = [p for p in xs if p[1] is not None]
            # runs carry no usable top: pair by x order; stock text pairs with the nearest glyph in (x, top)
            kk = sorted(ks, key=lambda t: t[0])
            if runs:
                # the glyphs of the key that the runs did not take: all if there is no stock text of the key
                pool = list(kk)
                for p in stock:
                    j = min(range(len(pool)), key=lambda j: (pool[j][0] - p[0]) ** 2 + (pool[j][1] - p[1]) ** 2)
                    q = pool.pop(j); dx = max(dx, abs(p[0] - q[0]) * fs); dtop = max(dtop, abs(p[1] - q[1]) * fs)
                for p, q in zip(sorted(runs, key=lambda t: t[0]), pool):
                    dx = max(dx, abs(p[0] - q[0]) * fs)
                    if abs(p[0] - q[0]) * fs > 0.06: bad.append((k[0], k[1], round(q[0] * fs, 3), round((p[0] - q[0]) * fs, 3)))
            else:
                pool = list(kk)
                for p in stock:
                    j = min(range(len(pool)), key=lambda j: (pool[j][0] - p[0]) ** 2 + (pool[j][1] - p[1]) ** 2)
                    q = pool.pop(j); dx = max(dx, abs(p[0] - q[0]) * fs); dtop = max(dtop, abs(p[1] - q[1]) * fs)
                    if abs(p[0] - q[0]) * fs > 0.06: bad.append((k[0], k[1], round(q[0] * fs, 3), round((p[0] - q[0]) * fs, 3)))
        out.append(dict(l=str(i), wk=kl * fs, wf=fl * fs, dw=(fl - kl) * fs, dx=dx, dtop=dtop, matched=ok, nrun=nrun, nstock=nstock, els_k=kn, els_f=fn, bytes=len(h.encode()), bad=bad[:40]))
json.dump(out, open(a.out + '/metrics.json', 'w'))
ok = [o for o in out if o['matched']]
print('formulas', len(out), 'identity matched', len(ok))
for name, vals in (('glyph x', [o['dx'] for o in ok]), ('stock text top', [o['dtop'] for o in ok]), ('width', [abs(o['dw']) for o in ok])):
    vals = sorted(vals); print('%s px: median %.4f p95 %.4f p99 %.4f max %.4f' % (name, statistics.median(vals), vals[int(.95 * len(vals))], vals[int(.99 * len(vals))], vals[-1]))
