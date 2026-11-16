import sys,json
sys.path.insert(0,'/home/agents/scratch/katex-font/tools'); sys.path.insert(0,'/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import *
fb='/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/KaTeXMathRun'
e=Encoder(fb+'.map.json')
a=e.base[('Main-Regular',97)][1]/e.upem; b=e.base[('Main-Regular',98)][1]/e.upem
# a and b both at x=0 (stacked), then c after the wider of them
g=[['Main-Regular',97,1,0,0,a,''],['Main-Regular',98,1,0,0,b,''],['Main-Regular',99,1,max(a,b),0,0.444,'']]
res=e.best(g,max(a,b)+0.444)
print('cps',[hex(c) for c in res[0]],'missing',res[1])
cps_alt=''.join(chr(c) for c in res[0])
res2=e.encode(g,'dom',None,max(a,b)+0.444,alt=False)
cps_noalt=''.join(chr(c) for c in res2[0])
html='<!doctype html><meta charset=utf-8><style>@font-face{font-family:F;src:url(file://%s.woff2) format("woff2")}.t{font:40px F;white-space:nowrap;text-rendering:geometricPrecision}</style><p><span class=t id=a>%s</span><p><span class=t id=b>%s</span>'%(fb,cps_alt,cps_noalt)
open('pages/t7.html','w').write(html)
with chrome(9595) as c:
    c.goto('pages/t7.html')
    print('expected width px',(max(a,b)+0.444)*40)
    print('with alt twins',c.ev('document.getElementById("a").getBoundingClientRect().width'),' without (real letters)',c.ev('document.getElementById("b").getBoundingClientRect().width'))
