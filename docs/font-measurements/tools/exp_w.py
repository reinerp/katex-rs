import sys, json, os, re
sys.path.insert(0,'/home/agents/scratch/katex-font/tools')
from cu import *
N='/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/'
newh={j['id']:j['html'] for j in map(json.loads,open('rsout/font_inline.jsonl')) if 'html' in j}
oldh={j['id']:j['html'] for j in map(json.loads,open('old/font_inline.jsonl')) if 'html' in j}
def css(p,w): return open(p).read().replace('url(MathRun.woff2)','url(file://%s)'%w).replace('url(fonts/','url(file:///home/agents/scratch/katex-compact/css/fonts/')
cn=css('rsout/font_new.css',N+'KaTeXMathRun-794995a4.woff2'); co=css('rsout/font_old.css','/home/agents/scratch/katex-font/old/data/KaTeXMathRun-e9c82a06.woff2')
ids=[i for i in sorted(newh) if '​' in newh[i]][:200]
def row(h): return '<div class=row>%s</div>'%h
page=lambda c,hs:'<!doctype html><meta charset=utf-8><style>body{margin:0;font:16px/40px sans-serif}.row{white-space:nowrap;width:max-content}%s</style>%s'%(c,''.join(row(h) for h in hs))
def variant(h,kind):
    if kind=='asis': return h
    if kind=='wbr': return re.sub(r'<span class="mu"></span>','',h.replace('​','<wbr>').replace('katex mw mf','katex mf'))
    if kind=='nozw': return re.sub(r'<span class="mu"></span>','',h.replace('​','').replace('katex mw mf','katex mf'))
vs={'old':(co,[oldh[i] for i in ids]),'asis':(cn,[newh[i] for i in ids]),'wbr':(cn,[variant(newh[i],'wbr') for i in ids]),'nozw':(cn,[variant(newh[i],'nozw') for i in ids])}
res={}
with chrome(9668) as c:
    for k,(cc,hs) in vs.items():
        open('exp/w_%s.html'%k,'w').write(page(cc,hs)); c.goto('exp/w_%s.html'%k)
        res[k]=json.loads(c.ev('JSON.stringify([...document.querySelectorAll(".row .katex")].map(k=>k.getBoundingClientRect().width))'))
for k in ('asis','wbr','nozw'):
    d=[(res[k][j]-res['old'][j])*1 for j in range(len(ids))]; print(k,'minus old: max %.4f min %.4f mean %.4f'%(max(d),min(d),sum(d)/len(d)))
d=[res['asis'][j]-res['wbr'][j] for j in range(len(ids))]; print('asis minus wbr',max(d),min(d))
d=[res['asis'][j]-res['nozw'][j] for j in range(len(ids))]; print('asis minus nozw',max(d),min(d))
