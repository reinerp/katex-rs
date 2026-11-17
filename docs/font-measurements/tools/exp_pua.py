import sys, json
sys.path.insert(0, '/home/agents/scratch/katex-font/tools')
from cu import *
html = '<!doctype html><meta charset=utf-8><style>@font-face{font-family:P;src:url(file:///home/agents/scratch/katex-font/exp/p.woff2)}body{margin:0;font:10px/14px P,monospace}.c{width:4px;white-space:normal}</style>'
cps = list(range(0xE000, 0xF900))
for cp in cps: html += '<div class=c>%s</div>' % chr(cp)
html += '''<script>window.r=()=>JSON.stringify([...document.querySelectorAll(".c")].map(c=>{const n=c.firstChild;const rg=document.createRange();const t=[];for(let i=0;i<n.nodeValue.length;i++){rg.setStart(n,i);rg.setEnd(n,i+1);t.push(Math.round(rg.getBoundingClientRect().top));} return t.join(",")}))</script>'''
open('exp/pua.html', 'w').write(html)
with chrome(9664) as c:
    c.goto('exp/pua.html')
    res = json.loads(c.ev('r()'))
    bad = [(hex(cp), r) for cp, r in zip(cps, res) if len(set(r.split(","))) > 1]
    print(len(bad), bad[:20])
