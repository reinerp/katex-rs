import sys, json, os
sys.path.insert(0, '/home/agents/scratch/katex-font/tools')
from cu import *
def BOX(extra=''): return '<span class=b style="display:inline-block;width:20px;background:#ccc;%s">B</span>' % extra
TX = 'ABCD'; WJ = '⁠'; ZW = '​'
NW = lambda x: '<span style="white-space:nowrap">%s</span>' % x
cases = {}
P = [c for c in '!"#$%\'()*+,-./0123:;=?@AB[\\]^_`{|}~' ]
for x in P:
    for y in P:
        cases['%s%s' % (x, y)] = 'AB' + x + y + 'AB'
html = '<!doctype html><meta charset=utf-8><style>body{margin:0;font:10px/20px monospace}.c{margin:6px;width:%dpx;outline:1px solid #eee;white-space:normal}</style>'
def page(w):
    h = html % w
    for i, (k, body) in enumerate(cases.items()): h += '<div class=c>%s</div>' % body
    h += '''<script>window.lines=()=>JSON.stringify([...document.querySelectorAll(".c")].map(c=>{
      const items=[];const w=document.createTreeWalker(c,NodeFilter.SHOW_TEXT);let n;const rg=document.createRange();
      while(n=w.nextNode()){ for(let i=0;i<n.nodeValue.length;i++){ rg.setStart(n,i);rg.setEnd(n,i+1);const b=rg.getBoundingClientRect(); const ch=n.nodeValue[i]; items.push([ch=="\\u2060"?"~":ch=="\\u200b"?"z":ch=="\\u200d"?"j":ch,Math.round(b.top)]);} }
      let s="";let last=null;for(const [ch,t] of items){ if(last!==null&&t!==last&&!(ch=="~"||ch=="z")) s+="¦"; s+=ch; last=t;} return s;}))</script>'''
    return h
with chrome(9656) as c:
    for w in (26,):
        open('exp/wrap2.html', 'w').write(page(w)); c.goto('exp/wrap2.html')
        res = json.loads(c.ev('lines()'))
        print('container width', w)
        for k, r in zip(cases, res):
            if '¦' in r and 'AB'+k+'AB' != r: print(k, end=' ')
