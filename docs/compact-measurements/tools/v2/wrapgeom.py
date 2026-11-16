# wrapgeom.py PAGE OUT.json : per formula the chars' left/top and the line-start chars (x decreases, or a new line) from headless Chrome
import sys, os, json
sys.path.insert(0, '/home/agents/scratch/katex-compact/tools')
from cdp import *
from headless_chrome import headless_chrome
page, out = sys.argv[1], sys.argv[2]
JS = r'''(()=>{const out=[];for(const row of document.querySelectorAll('.row')){const r0=row.getBoundingClientRect();const k=row.querySelector('.katex, .katex-error');
const starts=[];let n_chars=0;if(k){const w=document.createTreeWalker(k.querySelector('.katex-html')||k,NodeFilter.SHOW_TEXT);let n;const rg=document.createRange();let prev=null;
while(n=w.nextNode()){const t=n.nodeValue;for(let i=0;i<t.length;i++){if(/[\s​ ]/.test(t[i]))continue;rg.setStart(n,i);rg.setEnd(n,i+1);const b=rg.getBoundingClientRect();
if(prev&&b.left<prev.left-1&&b.bottom>prev.bottom+3)starts.push(n_chars);prev=b;n_chars++}}}
out.push({id:+row.dataset.id,h:r0.height,n:n_chars,starts})}return JSON.stringify(out)})()'''
with headless_chrome(9414, '--allow-file-access-from-files', window_size='1200,900'):
    t = new_tab(9414); ws = WS(t['webSocketDebuggerUrl']); ws.call('Page.enable'); ws.call('Runtime.enable')
    ws.call('Page.navigate', url='file://' + os.path.abspath(page)); ws.pump('Page.loadEventFired', 120)
    ws.call('Runtime.evaluate', expression='document.fonts.ready', awaitPromise=True)
    ws.call('Runtime.evaluate', expression='document.body.offsetHeight', returnByValue=True)
    open(out, 'w').write(ws.call('Runtime.evaluate', expression=JS, returnByValue=True)['result']['value'])
