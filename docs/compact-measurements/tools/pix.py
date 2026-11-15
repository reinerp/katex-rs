# pix.py PORT OUTDIR DPR NAME...   screenshot pages/g_NAME_NNN.html (all pages) at device scale DPR -> OUTDIR/NAME_dprD_NNN.png
import sys, os, glob, base64, time, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import *
from headless_chrome import headless_chrome
MAXP = int(os.environ.get('MAXP', '999'))
port = int(sys.argv[1]); out = sys.argv[2]; dprs = [int(x) for x in sys.argv[3].split(',')]; names = sys.argv[4:]
os.makedirs(out, exist_ok=True)
P = '/home/agents/scratch/katex-compact/pages/'
with headless_chrome(port, '--allow-file-access-from-files', '--force-color-profile=srgb', '--font-render-hinting=none', window_size='1200,5000'):
    t = new_tab(port); ws = WS(t['webSocketDebuggerUrl']); ws.call('Page.enable'); ws.call('Runtime.enable')
    for dpr in dprs:
        ws.call('Emulation.setDeviceMetricsOverride', width=1200, height=5000, deviceScaleFactor=dpr, mobile=False)
        for name in names:
            for page in sorted(glob.glob(P + 'g_%s_[0-9][0-9][0-9].html' % name))[:MAXP]:
                ws.call('Page.navigate', url='file://' + page)
                ws.pump('Page.loadEventFired', 60)
                ws.call('Runtime.evaluate', expression='document.fonts.ready.then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))', awaitPromise=True)
                png = ws.call('Page.captureScreenshot', format='png', captureBeyondViewport=False)['data']
                num = page[-8:-5]
                open('%s/%s_dpr%d_%s.png' % (out, name, dpr, num), 'wb').write(base64.b64decode(png))
    close_tab(port, t['id'])
print('done')
