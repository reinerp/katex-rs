# shot.py PAGE OUT.png [dpr] [width] : screenshot a page
import sys, os, base64
sys.path.insert(0, '/home/agents/scratch/katex-compact/tools')
from cdp import *
from headless_chrome import headless_chrome
page, out = sys.argv[1], sys.argv[2]; dpr = int(sys.argv[3]) if len(sys.argv) > 3 else 1; w = int(sys.argv[4]) if len(sys.argv) > 4 else 1200
with headless_chrome(9413, '--allow-file-access-from-files', '--force-color-profile=srgb', window_size='%d,1400' % w):
    t = new_tab(9413); ws = WS(t['webSocketDebuggerUrl']); ws.call('Page.enable'); ws.call('Runtime.enable')
    ws.call('Emulation.setDeviceMetricsOverride', width=w, height=1400, deviceScaleFactor=dpr, mobile=False)
    ws.call('Page.navigate', url='file://' + os.path.abspath(page)); ws.pump('Page.loadEventFired', 60)
    ws.call('Runtime.evaluate', expression='document.fonts.ready.then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))', awaitPromise=True)
    open(out, 'wb').write(base64.b64decode(ws.call('Page.captureScreenshot', format='png')['data']))
