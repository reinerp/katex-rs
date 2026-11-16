# mftest.py FONTBASE OUTDIR [--ids a,b,c|all|tex:file] [--dpr 1,2] [--kt geo|auto] [--rowh 80] [--tex 'a+b' ...]
# Encode formulas for the generated font from KaTeX JS geometry, then compare with KaTeX JS: widths, glyph x, pixels.
import sys, os, json, subprocess, argparse, statistics
sys.path.insert(0, '/home/agents/scratch/katex-font/tools'); sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import Encoder, to_html
ap = argparse.ArgumentParser(); ap.add_argument('font'); ap.add_argument('out'); ap.add_argument('--ids', default='')
ap.add_argument('--dpr', default='1'); ap.add_argument('--kt', default='geo'); ap.add_argument('--rowh', type=int, default=80)
ap.add_argument('--tex', action='append', default=[]); ap.add_argument('--port', type=int, default=9521); ap.add_argument('--nopix', action='store_true')
ap.add_argument('--mfcss', default=''); ap.add_argument('--nohint', action='store_true'); ap.add_argument('--texfile', default='')
a = ap.parse_args()
os.makedirs(a.out, exist_ok=True)
fam = os.path.basename(a.font)
enc = Encoder(a.font + '.map.json')
F = [json.loads(l) for l in open('/home/agents/scratch/katex-harden/data/formulas.jsonl')]
items = []   # (label, tex)
if a.ids == 'all': items = [(str(f['id']), f['tex']) for f in F]
elif a.ids: items = [(i, F[int(i)]['tex']) for i in a.ids.split(',')]
items += [('t%d' % k, t) for k, t in enumerate(a.tex)]
if a.texfile: items += [('x%d' % k, t.rstrip('\n')) for k, t in enumerate(open(a.texfile))]
fontcss = '@font-face{font-family:%s;src:url(file://%s.woff2) format("woff2")}.mf{font:normal 1.21em/0 %s;text-rendering:geometricPrecision;white-space:nowrap;text-indent:0}%s' % (fam, os.path.abspath(a.font), fam, a.mfcss)
ktcss = '.katex{text-rendering:geometricPrecision!important}' if a.kt == 'geo' else ''
base = 'body{margin:0;font:16px/%dpx system-ui,sans-serif;background:#fff;color:#000}.row{height:%dpx;line-height:%dpx;white-space:nowrap;padding:0 20px;box-sizing:border-box;overflow:hidden;position:relative}' % (a.rowh, a.rowh, a.rowh)
res = {}
with chrome(a.port, '--font-render-hinting=none' if a.nohint else '--no-sandbox-dummy') as c:
    c.goto('/home/agents/scratch/katex-font/data/ex.html')
    c.ev(open('/home/agents/scratch/katex-font/tools/mfextract.py').read().split("c.goto(page)")[1].split("''', await_=True)")[0].split("c.ev('''")[1] + "", await_=True)
    gl = {}
    for i in range(0, len(items), 100):
        part = [[l, t] for l, t in items[i:i + 100]]
        r = json.loads(c.ev('JSON.stringify(%s.map(([l,t])=>Object.assign({l},extract(t,false))))' % json.dumps(part)))
        for o in r: gl[o['l']] = o
    rows = []
    for l, t in items:
        o = gl[l]
        if 'g' not in o: continue
        glx = list(o['g']) + [['rule', 0.04, 1, r[1], r[3], r[2], ''] for r in o['r'] if r[0] == 'frac-line']
        cps, missing, nsp, trace = enc.best(glx, o['box'][0])
        o['gl'] = glx
        rows.append(dict(l=l, tex=t, o=o, cps=cps, missing=missing, nsp=nsp, trace=trace))
    # pages (chunks of 40 rows)
    CH = 40
    for ci in range(0, len(rows), CH):
        part = rows[ci:ci + CH]
        kt = ''.join('<div class=row data-l="%s">%s</div>' % (r['l'], '__K__') for r in part)
        res_chunk = []
        # KaTeX rendering is done in-page by JS so the markup is exactly KaTeX's
        kp = '<!doctype html><meta charset=utf-8><link rel=stylesheet href="file:///home/agents/scratch/katex-font/katex/katex.min.css"><style>%s%s</style><div id=c></div><script src="file:///home/agents/scratch/katex-font/katex/katex.min.js"></script><script>const T=%s;const c=document.getElementById("c");for(const [l,t] of T){const d=document.createElement("div");d.className="row";d.dataset.l=l;d.innerHTML=katex.renderToString(t,{displayMode:false,output:"html",throwOnError:false,maxExpand:1000});c.appendChild(d)}</script>' % (base, ktcss, json.dumps([[r['l'], r['tex']] for r in part]))
        fp = '<!doctype html><meta charset=utf-8><style>%s%s</style><div id=c>%s</div>' % (base, fontcss, ''.join('<div class=row data-l="%s"><span class=mf>%s</span></div>' % (r['l'], to_html(r['cps'])) for r in part))
        open('%s/k_%03d.html' % (a.out, ci // CH), 'w').write(kp); open('%s/f_%03d.html' % (a.out, ci // CH), 'w').write(fp)
    metr = {}
    for dpr in [int(x) for x in a.dpr.split(',')]:
        c.ws.call('Emulation.setDeviceMetricsOverride', width=1400, height=CH * a.rowh, deviceScaleFactor=dpr, mobile=False)
        for ci in range(0, len(rows), CH):
            n = ci // CH
            for kind in 'kf':
                c.goto('%s/%s_%03d.html' % (a.out, kind, n))
                if not a.nopix: c.shot('%s/%s_%03d_dpr%d.png' % (a.out, kind, n, dpr))
                if dpr == 1:
                    if kind == 'k': js = 'JSON.stringify([...document.querySelectorAll(".row")].map(r=>[r.dataset.l,r.querySelector(".katex").getBoundingClientRect().width,r.querySelector(".katex").getBoundingClientRect().left]))'
                    else: js = '''JSON.stringify([...document.querySelectorAll(".row")].map(r=>{const s=r.querySelector(".mf");const sr=s.getBoundingClientRect();const tn=s.firstChild;const out=[];const rg=document.createRange();const t=tn?tn.nodeValue:"";for(let i=0;i<t.length;i++){const cp=t.codePointAt(i);const n=cp>0xffff?2:1;rg.setStart(tn,i);rg.setEnd(tn,i+n);const b=rg.getBoundingClientRect();out.push([b.left-sr.left,b.width]);i+=n-1}return [r.dataset.l,sr.width,out]}))'''
                    metr[(kind, n)] = json.loads(c.ev(js))
    print('console', c.console()[:6])
# glyph x deviations and widths
out = []
for ci in range(0, len(rows), CH):
    n = ci // CH
    for j, r in enumerate(rows[ci:ci + CH]):
        wk = metr[('k', n)][j][1]; wf, lefts = metr[('f', n)][j][1], metr[('f', n)][j][2]
        fs = r['o']['fs']; dev = 0; k = 0
        for pos, gi in enumerate(r['trace']):
            if gi is None: continue
            dev = max(dev, abs(lefts[pos][0] - r['o']['gl'][gi][3] * fs))
        out.append(dict(l=r['l'], wk=wk, wf=wf, dw=wf - wk, dev=dev, ncp=len(r['cps']), nglyph=len(r['o']['g']), nsp=r['nsp'], missing=r['missing'], els=r['o']['els']))
json.dump(out, open(a.out + '/metrics.json', 'w'))
ok = [o for o in out if not o['missing']]
print('formulas', len(out), 'fully encoded', len(ok))
if ok:
    print('max |dx| px: median %.4f p95 %.4f max %.4f' % (statistics.median(o['dev'] for o in ok), sorted(o['dev'] for o in ok)[int(.95 * len(ok))], max(o['dev'] for o in ok)))
    print('width diff px: median %.4f max %.4f' % (statistics.median(abs(o['dw']) for o in ok), max(abs(o['dw']) for o in ok)))
