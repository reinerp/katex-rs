# mkpage.py OUT.html DATA.jsonl [--css PATH] [--js]   Build the fixture-shaped static page (400 messages x 12 corpus formulas).
# With --js the formulas are rendered in the page by KaTeX JS 0.18.5 (all at once, on DOMContentLoaded) instead.
import json, sys, html as H
R = '/home/agents/scratch/katex-harden/'
KCSS = R + 'npm/katex-0.18.5/package/dist/katex.min.css'
KJS = R + 'npm/katex-0.18.5/package/dist/katex.min.js'
a = sys.argv[1:]; out = a[0]; data = a[1]
css = KCSS; js = False; cv = False; nmsg = 400; per = 12
i = 2
while i < len(a):
    if a[i] == '--css': css = a[i+1]; i += 2
    elif a[i] == '--js': js = True; i += 1
    elif a[i] == '--cv': cv = True; i += 1
    elif a[i] == '--msgs': nmsg = int(a[i+1]); i += 2
    else: raise SystemExit('bad arg ' + a[i])
F = [json.loads(l) for l in open(R + 'data/formulas.jsonl')]
res = {}
if not js:
    for l in open(data):
        j = json.loads(l); res[j['id']] = j
def fm(f):
    if js: return '<span class="jsm" data-d="%d">%s</span>' % (f['display'], H.escape(f['tex']))
    r = res[f['id']]
    return r.get('html') or ''
parts = []; k = 0
for i in range(nmsg):
    ps = []
    for j in range(per):
        f = F[k % len(F)]; k += 1
        if f['display']: ps.append('<p>Step %d.%d:</p><p>%s</p>' % (i, j, fm(f)))
        else: ps.append('<p>Step %d.%d: we have %s here.</p>' % (i, j, fm(f)))
    parts.append('<div class="md">' + ''.join(ps) + '</div>')
style = ('body{margin:0;font:15px/1.5 system-ui,sans-serif;background:#fff;color:#111}.msgs{width:760px;margin:0 auto}'
         '.md{word-break:break-word;overflow-wrap:anywhere%s}' % (';content-visibility:auto;contain-intrinsic-size:auto 420px' if cv else '') + '.md p{margin:.5em 0}.md .katex-display{overflow-x:auto;overflow-y:hidden;padding:4px 0}')
settle = ('<script>window.__t0=performance.now();window.__ev={};'
          'function __done(){document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>{window.__ev.settled=performance.now();window.__settled=true;})))}'
          'addEventListener("load",()=>{window.__ev.load=performance.now();__done()});</script>')
jsrun = ''
if js:
    jsrun = ('<script src="file://%s"></script><script>addEventListener("DOMContentLoaded",()=>{const t=performance.now();'
             'for(const e of document.querySelectorAll(".jsm")){e.outerHTML=katex.renderToString(e.textContent,{displayMode:e.dataset.d==="1",throwOnError:false,output:"htmlAndMathml",trust:false,maxExpand:1000,maxSize:100})}'
             'window.__ev.jsrender=performance.now()-t})</script>') % KJS
open(out, 'w').write('<!doctype html><meta charset=utf-8><link rel=stylesheet href="file://%s"><style>%s</style>%s<body><div class=msgs>%s</div>%s' % (css, style, settle, ''.join(parts), jsrun))
print(out, 'bytes', __import__('os').path.getsize(out))
