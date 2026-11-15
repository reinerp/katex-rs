# geom.py PAGE OUT.json [context]: per-formula geometry from headless Chrome (run inside sxc)
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import *
from headless_chrome import headless_chrome
page, out = sys.argv[1], sys.argv[2]; ctx = len(sys.argv) > 3
JS_CHARS = r'''(()=>{const out=[];for(const row of document.querySelectorAll('.row')){const r0=row.getBoundingClientRect();const k=row.querySelector('.katex, .katex-error');if(!k){out.push({id:+row.dataset.id,chars:[],katex:null});continue}
const chars=[];const w=document.createTreeWalker(k.querySelector('.katex-html')||k,NodeFilter.SHOW_TEXT);let n;const rg=document.createRange();
while(n=w.nextNode()){const t=n.nodeValue;for(let i=0;i<t.length;i++){if(/[\s\u200b\u00a0]/.test(t[i]))continue;rg.setStart(n,i);rg.setEnd(n,i+1);const b=rg.getBoundingClientRect();chars.push([b.left-r0.left,b.top-r0.top,b.width,b.height])}}
const kr=k.getBoundingClientRect();out.push({id:+row.dataset.id,chars,katex:[kr.left-r0.left,kr.top-r0.top,kr.right-r0.left,kr.bottom-r0.top]})}return JSON.stringify(out)})()'''
JS_CTX = '''(()=>{const out=[];for(const row of document.querySelectorAll('.row')){const r0=row.getBoundingClientRect();const k=row.querySelector('.katex, .katex-error');const bl=row.querySelector('.bl').getBoundingClientRect();
const kr=k?k.getBoundingClientRect():null;out.push({id:+row.dataset.id,h:r0.height,base:bl.bottom-r0.top,katex:kr?[kr.left-r0.left,kr.top-r0.top,kr.right-r0.left,kr.bottom-r0.top]:null})}return JSON.stringify(out)})()'''
with headless_chrome(9411, '--allow-file-access-from-files', window_size='1200,900'):
    t = new_tab(9411); ws = WS(t['webSocketDebuggerUrl']); ws.call('Page.enable'); ws.call('Runtime.enable')
    ws.call('Page.navigate', url='file://' + os.path.abspath(page)); ws.pump('Page.loadEventFired', 120)
    ws.call('Runtime.evaluate', expression='document.fonts.ready', awaitPromise=True)
    ws.call('Runtime.evaluate', expression='document.body.offsetHeight', returnByValue=True)
    r = ws.call('Runtime.evaluate', expression=JS_CTX if ctx else JS_CHARS, returnByValue=True)
    open(out, 'w').write(r['result']['value'])
print('ok')
