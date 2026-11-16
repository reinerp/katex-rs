import sys,json,base64
sys.path.insert(0,'/home/agents/scratch/katex-font/tools')
from cu import *
b64=base64.b64encode(open('/home/agents/scratch/katex-font/outx/blank.woff2','rb').read()).decode()
html='<!doctype html><meta charset=utf-8><style>@font-face{font-family:B;src:url(data:font/woff2;base64,%s) format("woff2")}@font-face{font-family:M;src:url(file:///nonexistent.woff2);font-display:swap}body{font:30px/40px system-ui}.t{font-family:M,B,serif}</style><p><span class=t>a+b\U0001d465\U000f0001a</span> end <p><span style="font-family:serif">a+b\U0001d465a</span> (no blank font)'%b64
open('pages/t8.html','w').write(html)
with chrome(9596) as c:
    c.ws.call('Emulation.setDeviceMetricsOverride', width=500, height=130, deviceScaleFactor=1, mobile=False)
    c.goto('pages/t8.html',wait=False); c.ev('new Promise(r=>setTimeout(r,400))',await_=True)
    print(c.ev('JSON.stringify([...document.fonts].map(f=>[f.family,f.status]))'), c.console()[:3])
    c.shot('pages/t8.png')
