import sys, json
sys.path.insert(0, '/home/agents/scratch/katex-font/tools')
from cu import *
WJ='\u2060'; ZW='\u200b'
LONG = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ' * 2    # 52 chars, 6px each = 312 > 300
PRE = 'Some words before, '
css = 'body{margin:0;font:10px/14px monospace}p{width:300px;margin:0 0 4px;outline:1px solid #eee}.k{white-space:normal}.k::before{content:"";display:inline-block;height:40px;vertical-align:-10px;width:0}.k2{white-space:normal}.u{display:inline-block;height:40px;vertical-align:-10px;width:0}'
U = '<span class=u></span>'
cases = {
 'pseudo strut, no WJ': '<span class=k>' + LONG + '</span>',
 'element strut, no WJ': '<span class=k2>' + U + LONG + '</span>',
 'element strut, WJ': '<span class=k2>' + U + WJ + LONG + '</span>',
 'element strut, WJ, ZW segs': '<span class=k2>' + U + WJ + LONG[:10] + ZW + LONG[:30] + ZW + LONG[:30] + '</span>',
 'pseudo strut, WJ': '<span class=k>' + WJ + LONG + '</span>',
 'nowrap root, element strut': '<span class=k2 style="white-space:nowrap">' + U + LONG + '</span>',
}
html = '<!doctype html><meta charset=utf-8><style>%s.kw::before{content:"\\2060"}</style>' % css
for k, v in cases.items(): html += '<p data-k="%s">%s%s</p>' % (k, PRE, v)
html += '''<script>window.r=()=>JSON.stringify([...document.querySelectorAll("p")].map(p=>{const w=document.createTreeWalker(p,NodeFilter.SHOW_TEXT);let n,tops=[];const rg=document.createRange();
 while(n=w.nextNode()){ for(const i of [0,n.nodeValue.length-1]){ if(n.nodeValue.length==0)continue; rg.setStart(n,i);rg.setEnd(n,i+1);const b=rg.getBoundingClientRect(); tops.push(Math.round(b.top-p.getBoundingClientRect().top)); } }
 return [p.dataset.k, Math.round(p.getBoundingClientRect().height), tops.join(",")]}))</script>'''
open('exp/strut.html', 'w').write(html)
with chrome(9661) as c:
    c.goto('exp/strut.html')
    for r in json.loads(c.ev('r()')): print(r)
