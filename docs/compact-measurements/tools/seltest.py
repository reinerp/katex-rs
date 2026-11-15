import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import *
from headless_chrome import headless_chrome
JS = '''JSON.stringify([...document.querySelectorAll('p')].map(p=>{const r=document.createRange();r.selectNodeContents(p);const s=getSelection();s.removeAllRanges();s.addRange(r);return [s.toString(), p.innerText]}))'''
with headless_chrome(9411, '--allow-file-access-from-files', window_size='1200,900'):
    for name in sys.argv[1:]:
        t = new_tab(9411); ws = WS(t['webSocketDebuggerUrl']); ws.call('Page.enable'); ws.call('Runtime.enable')
        ws.call('Page.navigate', url='file://' + os.path.abspath('pages/sel_%s.html' % name)); ws.pump('Page.loadEventFired', 60)
        ws.call('Runtime.evaluate', expression='document.fonts.ready', awaitPromise=True)
        v = json.loads(ws.call('Runtime.evaluate', expression=JS, returnByValue=True)['result']['value'])
        print('==', name)
        for sel_, inner in v: print('  selection: %r\n  innerText: %r' % (sel_, inner))
        close_tab(9411, t['id'])
