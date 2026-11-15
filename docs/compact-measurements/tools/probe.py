# probe.py PAGE JS : load a page in headless chrome, print JSON of JS expression result
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import *
from headless_chrome import headless_chrome
with headless_chrome(9411, '--allow-file-access-from-files', window_size='1200,900'):
    t = new_tab(9411); ws = WS(t['webSocketDebuggerUrl']); ws.call('Page.enable'); ws.call('Runtime.enable')
    ws.call('Page.navigate', url='file://' + os.path.abspath(sys.argv[1])); ws.pump('Page.loadEventFired', 60)
    ws.call('Runtime.evaluate', expression='document.fonts.ready', awaitPromise=True)
    print(ws.call('Runtime.evaluate', expression=sys.argv[2], returnByValue=True)['result'].get('value'))
