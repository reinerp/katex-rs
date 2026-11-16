import sys,json,subprocess
sys.path.insert(0,'/home/agents/scratch/katex-font/tools'); sys.path.insert(0,'/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import *
e=Encoder('/home/agents/scratch/katex-font/'+sys.argv[2]+'/'+sys.argv[3]+'.map.json')
mi=e.base[('Math-Italic',120)]
rows=[]
vs=sorted(e.var.items(), key=lambda kv: kv[0][3])
base=sys.argv[1]
html='<!doctype html><meta charset=utf-8><link rel=stylesheet href="file:///home/agents/scratch/katex-font/katex/katex.min.css"><style>@font-face{font-family:'+sys.argv[3]+';src:url(file:///home/agents/scratch/katex-font/'+sys.argv[2]+'/'+sys.argv[3]+'.woff2)}body{margin:0;font:16px/60px system-ui}.row{height:60px;padding:0 20px;white-space:nowrap}.mf{font:normal 1.21em/0 '+sys.argv[3]+';text-rendering:geometricPrecision}%s</style>' % ('.katex{text-rendering:geometricPrecision!important}' if base=='geo' else '')
html+='<div class=row><span class=katex-host></span></div>'
html+='<div class=row id=k></div>'
cp2=[v for k,v in e.var.items() if k[1]==50][0][0]
for u in range(-120,121,20):
    sp,_=e.spacer(u,0)
    html+='<div class=row data-y="%s"><span class=mf>%s%s%s</span></div>'%(u,chr(mi[0]),''.join(chr(c) for c in sp),chr(cp2))
html+='<script src="file:///home/agents/scratch/katex-font/katex/katex.min.js"></script><script>document.getElementById("k").innerHTML=katex.renderToString("x^2",{output:"html"})</script>'
open('/home/agents/scratch/katex-font/pages/exp1.html','w').write(html)
for dpr in (1,2):
    with chrome(9541,dpr=dpr) as c:
        c.ws.call('Emulation.setDeviceMetricsOverride', width=400, height=60*15, deviceScaleFactor=dpr, mobile=False)
        c.goto('/home/agents/scratch/katex-font/pages/exp1.html'); c.shot('/home/agents/scratch/katex-font/pages/'+sys.argv[4]+'_%s_%d.png'%(base,dpr))
    # compare rows: row 1 (katex) vs rows 2..
    out=[]
    import os
