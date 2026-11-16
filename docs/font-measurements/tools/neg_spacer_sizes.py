import sys,json,os
sys.path.insert(0,'/home/agents/scratch/katex-font/tools'); sys.path.insert(0,'/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import *
fb='/home/agents/scratch/katex-font/outb/MathB'
e=Encoder(fb+'.map.json')
V=e.var[('Main-Regular',50,0.7,0.363)]; W=e.var[('Math-Italic',105,0.7,-0.15)]
sp,moved=e.spacer(-V[1],0)
s=chr(V[0])+''.join(chr(c) for c in sp)+chr(W[0])
expect=lambda px:(W[1])/e.upem*px     # V, back by V's advance, W: width = W advance
css='@font-face{font-family:F;src:url(file://%s.woff2) format("woff2")}.t{white-space:nowrap;font-family:F}'%fb
html='<!doctype html><meta charset=utf-8><style>%s</style>'%css
sizes=[6,8,10,12,14,16,20,24,32,48,100]
for tr in ('geometricPrecision','auto'):
    for px in sizes: html+='<p style="margin:0"><span class=t data-tr="%s" data-px=%d style="font-size:%dpx;text-rendering:%s">%s</span>'%(tr,px,px,tr,s)
open('pages/t6.html','w').write(html)
for dpr in (1,2):
    with chrome(9594,dpr=dpr) as c:
        c.ws.call('Emulation.setDeviceMetricsOverride', width=600, height=800, deviceScaleFactor=dpr, mobile=False)
        c.goto('pages/t6.html')
        r=json.loads(c.ev('JSON.stringify([...document.querySelectorAll(".t")].map(e=>[e.dataset.tr,+e.dataset.px,e.getBoundingClientRect().width]))'))
        print('dpr',dpr)
        for tr,px,w in r: print('  %-18s %3dpx width %8.3f expected %8.3f diff %+.3f'%(tr,px,w,expect(px),w-expect(px)))
