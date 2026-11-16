import sys,json
sys.path.insert(0,'/home/agents/scratch/katex-font/tools'); sys.path.insert(0,'/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import Encoder
import os
D='/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/'
mf=[f for f in os.listdir(D) if f.endswith('.map.json')][0]
enc=Encoder(D+mf)
V=chr(enc.var[('Main-Regular',50,0.7,0.363)][0]) if ('Main-Regular',50,0.7,0.363) in enc.var else None
A=chr(enc.base[('Main-Regular',ord('a'))][0]); B=chr(enc.base[('Main-Regular',ord('b'))][0])
adv=lambda ch: enc.base[('Main-Regular',ord(ch))][1]
sp=lambda u: ''.join(chr(c) for c in enc.spacer(u,0)[0])
aw=adv('a'); bw=adv('b')   # units; 4000 upem -> at 40px, 1 unit = .01 px
cases={
 'text A, back(-a), element[b]':  (A+sp(-aw),'<span>%s</span>'%B,   '', bw),            # run total 0, then element: width = b
 'text A+B, back(-a-b), element[A]': (A+B+sp(-aw-bw),'<span>%s</span>'%A, '', aw),
 'text A, back(-2a), element[B] (run total negative)': (A+sp(-2*aw),'<span>%s</span>'%B,'', bw-aw),
 'element[A], text back(-a) B': ('<span>%s</span>'%A, sp(-aw)+B,'',bw),               # text starts with negative spacer
 'element[A] then text [back(-a)+B] same text node wrapper': ('<span>%s</span>'%A, sp(-aw)+B+A,'',bw+aw),
 'text A B (no neg) | element[A]': (A+B,'<span>%s</span>'%A,'',aw+bw+aw),
}
css='@font-face{font-family:G;src:url(file://%s%s)}.t{font:40px/60px G;white-space:nowrap;text-rendering:geometricPrecision}'%(D,mf.replace('.map.json','.woff2'))
html='<!doctype html><meta charset=utf-8><style>%s</style>'%css
exp={}
for i,(k,(a,b,c,w)) in enumerate(cases.items()):
    html+='<p><span class=t id=c%d>%s%s%s</span>'%(i,a,b,c); exp[i]=w
open('pages/t12.html','w').write(html)
with chrome(9691) as c:
    c.goto('pages/t12.html')
    for i,(k,v) in enumerate(cases.items()):
        w=c.ev('document.getElementById("c%d").getBoundingClientRect().width'%i)
        print('%-62s width %.2f px (ideal %.2f)'%(k,w,exp[i]/4000*40))
