# mfpos.py OLD.jsonl OLD.map OLD.woff2 OLD.css NEW.jsonl NEW.map NEW.woff2 NEW.css MODE OUTDIR [--ids ...]: the glyph positions of two renderings of the same formulas
# (two versions of the font and the encoder): every glyph (a code point that is not a spacer) as (font, character, size, shift) and the x of its outline in
# em of the formula, from Range rects in headless Chrome; compared glyph by glyph (sorted by identity and x), widths of the formula and of stock text too.
import sys, os, json, re, argparse, statistics, collections
sys.path.insert(0, '/home/agents/scratch/katex-font/tools')
from cu import *
ap = argparse.ArgumentParser()
for n in ('oldj', 'oldmap', 'oldfont', 'oldcss', 'newj', 'newmap', 'newfont', 'newcss', 'mode', 'out'): ap.add_argument(n)
ap.add_argument('--ids', default='all'); ap.add_argument('--port', type=int, default=9644); ap.add_argument('--extra', default='')
a = ap.parse_args(); os.makedirs(a.out, exist_ok=True)
FONTS = ['Main-Regular', 'Math-Italic', 'AMS-Regular', 'Size1-Regular', 'Size2-Regular', 'Size3-Regular', 'Size4-Regular']
def revmap(path):
    m = json.load(open(path)); U = m['unitsPerEm']; rev = {}
    for f, d in m['glyphs'].items():
        for ch, e in d.items():
            rev[e['cp']] = ('g', f, int(ch), 1.0, 0.0, 0, e['adv'])
            if 'noic' in e: rev[e['noic']] = ('g', f, int(ch), 1.0, 0.0, 0, e['adv'] - e.get('ic', 0))
    for v in m['variants']:
        for k, (cp, adv) in v['glyphs'].items():
            f, ch = k.rsplit(':', 1); rev[cp] = ('g', f, int(ch), v['size'], v['shift'], 0, adv)
    for b in m.get('baked', []):
        f, ch, size, shift, L, R, cp, adv = b; rev[cp] = ('g', f, ch, size, shift, L, adv)
    for v in m['spacers']['named'].values(): rev[v[0]] = ('s',)
    for v in list(m['spacers']['pos'].values()) + list(m['spacers']['neg'].values()): rev[v] = ('s',)
    for r in m['rules']: rev[r['cp']] = ('s',)
    for o in m['overlays']: rev[o['cp']] = ('o',)
    return rev, U
GEOM = r'''
window.rows = function() { return JSON.stringify([...document.querySelectorAll('.row')].map(r => {
  const k = r.querySelector('.katex'); if (!k) return [r.dataset.l, 0, [], 0]; const kr = k.getBoundingClientRect(); const fs = parseFloat(getComputedStyle(k).fontSize);
  const out = []; const rg = document.createRange(); const w = document.createTreeWalker(k, NodeFilter.SHOW_TEXT); let n;
  while (n = w.nextNode()) { const px = parseFloat(getComputedStyle(n.parentElement).fontSize); const t = n.nodeValue;
    for (let i = 0; i < t.length; i++) { const cp = t.codePointAt(i); const len = cp > 0xffff ? 2 : 1;
      rg.setStart(n, i); rg.setEnd(n, i + len); const b = rg.getBoundingClientRect(); if (len == 2) i++;
      out.push([cp, +((b.left - kr.left) / fs).toFixed(5), +((b.right - kr.left) / fs).toFixed(5), +(px / fs).toFixed(4), +((b.top - kr.top) / fs).toFixed(4)]); } }
  return [r.dataset.l, kr.width / fs, out, k.getElementsByTagName('*').length + 1, kr.height / fs]; })); };
'''
def load(p, strip=False):
    rows = []
    for l in open(p):
        j = json.loads(l)
        if 'html' in j and (a.ids == 'all' or str(j['id']) in a.ids.split(',')): rows.append((j['id'], j['html'].replace('\u200b', '') if strip else j['html']))
    return rows
def run(c, tag, jf, mapf, fontf, cssf, strip=False):
    rows = load(jf, strip); rev, U = revmap(mapf)
    css = open(cssf).read().replace('url(MathRun.woff2)', 'url(file://%s)' % os.path.abspath(fontf)).replace('url(fonts/', 'url(file:///home/agents/scratch/katex-compact/css/fonts/')
    base = 'body{margin:0;font:16px/80px system-ui,sans-serif;background:#fff;color:#000}.row{height:80px;line-height:80px;white-space:nowrap;padding:0 20px;box-sizing:border-box;overflow:hidden;position:relative;width:max-content;min-width:100%%}' + ('.katex-display{margin:0!important}' if a.mode == 'display' else '')
    out = {}
    CH = 40
    for ci in range(0, len(rows), CH):
        part = rows[ci:ci + CH]
        open('%s/%s_%03d.html' % (a.out, tag, ci // CH), 'w').write('<!doctype html><meta charset=utf-8><style>%s%s%s</style><script>%s</script><div id=c>%s</div>' % (base, css, a.extra, GEOM, ''.join('<div class=row data-l="%s">%s</div>' % (i, h) for i, h in part)))
        c.goto('%s/%s_%03d.html' % (a.out, tag, ci // CH)); c.ev('document.fonts.ready.then(()=>1)')
        for r in json.loads(c.ev('rows()')):
            gl = []; stock = []
            prevz = None
            for cp, x0, x1, sz, top in r[2]:
                if cp == 0x200b: prevz = True; continue
                e = rev.get(cp)
                if e and e[0] == 'g':
                    # the pen position: after a zero-width space the rect of a glyph can be half of its cluster (then it ends where the glyph does)
                    if prevz and (x1 - x0) < e[6] * sz * 0.75: x0 = x1 - e[6] * sz
                    prevz = None
                    gl.append((e[1], e[2], round(e[3], 3), round(e[4], 3), round(x0 + e[5] / U * sz, 5)))
                elif e is None: stock.append((cp, x0, x1, sz))
            out[int(r[0])] = dict(raw=r[2], w=r[1], glyphs=gl, stock=stock, els=r[3], h=(r[4] if len(r) > 4 else 0))
    return out
with chrome(a.port) as c:
    c.ws.call('Emulation.setDeviceMetricsOverride', width=1400, height=3200, deviceScaleFactor=1, mobile=False)
    old = run(c, 'o', a.oldj, a.oldmap, a.oldfont, a.oldcss); new = run(c, 'n', a.newj, a.newmap, a.newfont, a.newcss, True); newz = run(c, 'z', a.newj, a.newmap, a.newfont, a.newcss)
FS = 19.36
res = []; bad = []
for i in sorted(old):
    o, n = old[i], new.get(i)
    if n is None: continue
    go = sorted(o['glyphs'], key=lambda g: (g[0], g[1], g[2], g[3], g[4])); gn = sorted(n['glyphs'], key=lambda g: (g[0], g[1], g[2], g[3], g[4]))
    ok = len(go) == len(gn) and all(x[:4] == y[:4] for x, y in zip(go, gn))
    dx = max((abs(x[4] - y[4]) for x, y in zip(go, gn)), default=0) * FS if ok else None
    # stock text: same characters, x
    so = sorted(o['stock']); sn = sorted(n['stock'])
    sdx = max((abs(x[1] - y[1]) for x, y in zip(so, sn)), default=0) * FS if len(so) == len(sn) and all(x[0] == y[0] for x, y in zip(so, sn)) else None
    z = newz[i]
    res.append(dict(id=i, same=ok, dwz=(z['w'] - o['w']) * FS, dhz=(z['h'] - o['h']) * FS, dx=dx, sdx=sdx, dw=(n['w'] - o['w']) * FS, dh=(n['h'] - o['h']) * FS, els_o=o['els'], els_n=n['els'], ng=len(go)))
    if not ok or dx > 0.0156 or sdx is None or sdx > 0.0156 or abs(n['w'] - o['w']) * FS > 0.0156: bad.append(res[-1])
json.dump(res, open(a.out + '/pos.json', 'w')); json.dump({'old': {str(k): v for k, v in old.items()}, 'new': {str(k): v for k, v in new.items()}}, open(a.out + '/glyphs.json', 'w'))
dxs = sorted(r['dx'] for r in res if r['dx'] is not None)
print('formulas', len(res), 'glyph sets differ', sum(1 for r in res if not r['same']))
if dxs: print('glyph x px (old vs new, %0.2f px font): median %.4f p99 %.4f max %.4f; over 1/64 px: %d' % (FS, statistics.median(dxs), dxs[int(.99 * len(dxs))], dxs[-1], sum(1 for d in dxs if d > 1 / 64 + 1e-9)))
sd = sorted(r['sdx'] for r in res if r['sdx'] is not None); print('stock text x: max %.4f px; differing sets %d' % (sd[-1] if sd else 0, sum(1 for r in res if r['sdx'] is None)))
dw = sorted(abs(r['dw']) for r in res); print('formula width: max %.4f px; over 1/64 px: %d' % (dw[-1], sum(1 for d in dw if d > 1 / 64 + 1e-9)))
print('height differences >0.02 px:', sum(1 for r in res if abs(r['dh']) > 0.02))
dz = sorted(abs(r['dwz']) for r in res); print('with the zero-width spaces: formula width max %.4f px over 1/64: %d; height differences >0.02 px: %d' % (dz[-1], sum(1 for d in dz if d > 1 / 64 + 1e-9), sum(1 for r in res if abs(r['dhz']) > 0.02)))
print('bad:', [(r['id'], r['same'], r['dx'], r['sdx'], round(r['dw'], 3)) for r in bad[:15]])
