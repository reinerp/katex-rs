# mkmicro.py OUTDIR NAME=HTMLFILE... : micro page, 3000 paragraphs "Step i: we have <formula> here."  Extra css from env EXTRA_CSS file.
import sys, os
out = sys.argv[1]
extra = open(os.environ['EXTRA_CSS']).read() if os.environ.get('EXTRA_CSS') else ''
css = os.environ.get('CSS', '/home/agents/scratch/katex-compact/css/katex-compact.css')
N = int(os.environ.get('N', '3000'))
for a in sys.argv[2:]:
    name, f = a.split('=', 1)
    body = open(f).read().strip() if os.path.exists(f) else f
    head = ('<!doctype html><meta charset=utf-8><link rel=stylesheet href="file://%s"><style>body{margin:0;font:15px/1.5 system-ui,sans-serif}.msgs{width:760px;margin:0 auto}p{margin:.5em 0}%s</style>'
            '<script>window.__ev={};addEventListener("load",()=>{window.__ev.load=performance.now();document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>{window.__ev.settled=performance.now();window.__settled=true})))})</script>' % (css, extra))
    ps = ''.join('<p>Step %d: we have %s here.</p>' % (i, body) for i in range(N))
    open(os.path.join(out, name + '.html'), 'w').write(head + '<body><div class=msgs>' + ps + '</div>')
