import sys,json
sys.path.insert(0,'/home/agents/scratch/katex-font/tools'); sys.path.insert(0,'/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import *
e=Encoder('/home/agents/scratch/katex-font/outx4/MathX4.map.json')
cp=[v for k,v in e.var.items() if k[1]==50][0][0]; cp8=[v for k,v in e.var.items() if k[1]==56][0][0]
tr=sys.argv[1]
html='<!doctype html><meta charset=utf-8><link rel=stylesheet href="file:///home/agents/scratch/katex-font/katex/katex.min.css"><style>@font-face{font-family:MathX4;src:url(file:///home/agents/scratch/katex-font/outx4/MathX4.woff2)}body{margin:0;font:16px/60px system-ui;text-rendering:%s}.row{height:60px;padding:0 20px;white-space:nowrap}.mf{font:normal 19.36px/0 MathX4}.nat{font:13.552px KaTeX_Main}</style>'%tr
html+='<div class=row></div><div class=row><span class=nat>28</span></div><div class=row><span class=mf>%s%s</span></div>'%(chr(cp),chr(cp8))
open('/home/agents/scratch/katex-font/pages/exp4.html','w').write(html)
for dpr in (1,2):
    with chrome(9542,dpr=dpr) as c:
        c.ws.call('Emulation.setDeviceMetricsOverride', width=400, height=180, deviceScaleFactor=dpr, mobile=False)
        c.goto('/home/agents/scratch/katex-font/pages/exp4.html'); c.shot('/home/agents/scratch/katex-font/pages/exp4_%s_%d.png'%(tr,dpr))
