import sys,json,os
sys.path.insert(0,'/home/agents/scratch/katex-font/tools'); sys.path.insert(0,'/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import *
fb='/home/agents/scratch/katex-font/outb/MathB'
e=Encoder(fb+'.map.json')
def cps(*xs): return ''.join(chr(c) for c in xs)
sp=lambda u: cps(*e.spacer(u,0)[0])
A=2000   # a
P=chr(e.var[('Main-Regular',50,0.7,0.363)][0]) if ('Main-Regular',50,0.7,0.363) in e.var else None
X=chr(e.base[('Math-Italic',120)][0]); a=chr(e.base[('Main-Regular',97)][0]); b=chr(e.base[('Main-Regular',98)][0]); Ph=chr(e.base[('Main-Regular',0x2212)][0])
V=chr(P)if False else P
ch=lambda k:chr(k)
neg1=chr(e.neg[1024]); neg2=chr(e.neg[2048]); neg4=chr(e.neg[512])
Xb=chr(e.base[('Math-Italic',121)][0]); AMS=chr(e.base[('Main-Regular',0x2212)][0])
n20=sp(-2000); n10=sp(-1000)
G=chr(e.base[('Main-Regular',0x393)][0])
cases={
 'V neg20 a  (run V+neg = -6 total)':V+n20+a,
 'V neg20 V  (run sum +8)':V+n20+V,
 'V neg20 1  (digit joins PUA run)':V+n20+'1',
 'a neg10 V neg10 V':a+n10+V+n10+V,
 'a neg20 V (run sum -6)':a+n20+V,
 'G(Greek cap) neg10 V':G+n10+V,
 'a neg10 G':a+n10+G,
 'V neg10 b':V+n10+b,
 'V neg10 + (U+002B)':V+n10+'+',
 '1 neg10 b':'1'+n10+b,
 '1 neg10 1':'1'+n10+'1',
 '= neg10 =':'='+n10+'=',
 'a neg10 a (both Latin)':a+n10+a,
}
css='@font-face{font-family:F;src:url(file://%s.woff2) format("woff2")}.t{font:40px/60px F;white-space:nowrap}'%fb
html='<!doctype html><meta charset=utf-8><style>%s</style>'%css+''.join('<p><span class=t data-k="%s">%s</span>'%(k,v) for k,v in cases.items())
open('pages/t4.html','w').write(html)
with chrome(9593) as c:
    c.goto('pages/t4.html')
    for k,w in json.loads(c.ev('JSON.stringify([...document.querySelectorAll(".t")].map(e=>[e.dataset.k,e.getBoundingClientRect().width]))')): print('%-20s %s'%(k,w))
