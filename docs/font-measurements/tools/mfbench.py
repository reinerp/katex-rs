# mfbench.py : build the micro pages (3000 paragraphs, one formula each) for plain text / current output / compact ALL / font
import sys, os, json
sys.path.insert(0, '/home/agents/scratch/katex-font/tools'); sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import Encoder, to_html
FONT = '/home/agents/scratch/katex-font/outb/MathB'
enc = Encoder(FONT + '.map.json')
TEX = {0: 'a+b+c=d', 2: 'x_i', 3: 'x^2', 4: 'x_i^2'}
src = open('/home/agents/scratch/katex-font/tools/mfextract.py').read().split("c.goto(page)")[1].split("''', await_=True)")[0].split("c.ev('''")[1]
with chrome(9571) as c:
    c.goto('/home/agents/scratch/katex-font/data/ex.html'); c.ev(src, await_=True)
    gl = {i: json.loads(c.ev('JSON.stringify(extract(%s,false))' % json.dumps(t))) for i, t in TEX.items()}
cmp_ = {r['id']: r['html'] for r in map(json.loads, open('/home/agents/scratch/katex-compact/data/micro_cmp.jsonl'))}
leg = {r['id']: r['html'] for r in map(json.loads, open('/home/agents/scratch/katex-compact/data/micro_leg.jsonl'))}
N = 3000
fontcss = '@font-face{font-family:F;src:url(file://%s.woff2) format("woff2")}.mf{font:normal 1.21em/0 F;text-rendering:geometricPrecision;white-space:nowrap}' % FONT
settle = '<script>window.__ev={};addEventListener("load",()=>{window.__ev.load=performance.now();document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>{window.__ev.settled=performance.now();window.__settled=true})))})</script>'
base = 'body{margin:0;font:15px/1.5 system-ui,sans-serif}.msgs{width:760px;margin:0 auto}p{margin:.5em 0}'
def page(name, body, css_link, extra=''):
    ps = ''.join('<p>Step %d: we have %s here.</p>' % (i, body) for i in range(N))
    open('/home/agents/scratch/katex-font/bench/%s.html' % name, 'w').write('<!doctype html><meta charset=utf-8>%s<style>%s%s</style>%s<body><div class=msgs>%s</div>' % (css_link, base, extra, settle, ps))
CC = '<link rel=stylesheet href="file:///home/agents/scratch/katex-compact/css/katex-compact.css">'
for i, t in TEX.items():
    o = gl[i]; r = enc.best(o['g'], o['box'][0]); assert not r[1], (t, r[1])
    page('plain_%d' % i, t.replace('^', '^').replace('_', '_'), '')
    page('leg_%d' % i, leg[i], CC)
    page('cmp_%d' % i, cmp_[i], CC)
    page('font_%d' % i, '<span class=mf>%s</span>' % to_html(r[0]), '', fontcss)
    print(i, t, 'cps', len(r[0]), 'font html bytes', len('<span class=mf>%s</span>' % to_html(r[0]).encode().decode()), 'compact', len(cmp_[i]), 'legacy', len(leg[i]))
