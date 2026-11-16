# Small helpers over cdp.py: with chrome(port) as c: c.goto(path); c.ev(js); c.shot(path)
import sys, os, json, base64, contextlib
sys.path.insert(0, '/home/agents/code/agentserver/scripts'); sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import *
from headless_chrome import headless_chrome
class C:
    def __init__(s, port, dpr=1, w=1200, h=900):
        s.port = port; s.t = new_tab(port); s.ws = WS(s.t['webSocketDebuggerUrl']); s.logs = []
        for m in ('Page.enable', 'Runtime.enable', 'Log.enable'): s.ws.call(m)
        s.ws.call('Emulation.setDeviceMetricsOverride', width=w, height=h, deviceScaleFactor=dpr, mobile=False)
    def goto(s, path, wait=True):
        s.ws.call('Page.navigate', url='file://' + os.path.abspath(path)); s.ws.pump('Page.loadEventFired', 120)
        if wait: s.ev('document.fonts.ready.then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))', await_=True)
    def ev(s, js, await_=False):
        r = s.ws.call('Runtime.evaluate', expression=js, returnByValue=True, awaitPromise=await_)
        if 'exceptionDetails' in r: raise RuntimeError(json.dumps(r['exceptionDetails'])[:800])
        return r['result'].get('value')
    def console(s):
        out = []
        for e in s.ws.events:
            if e['method'] == 'Log.entryAdded': out.append(e['params']['entry'].get('text', '')[:300])
            if e['method'] == 'Runtime.exceptionThrown': out.append('EXC ' + json.dumps(e['params'])[:300])
        return out
    def shot(s, path, clip=None):
        kw = {'format': 'png', 'captureBeyondViewport': False}
        if clip: kw['clip'] = dict(x=clip[0], y=clip[1], width=clip[2], height=clip[3], scale=1)
        open(path, 'wb').write(base64.b64decode(s.ws.call('Page.captureScreenshot', **kw)['data']))
    def close(s): close_tab(s.port, s.t['id'])
@contextlib.contextmanager
def chrome(port, *args, dpr=1, **kw):
    with headless_chrome(port, '--allow-file-access-from-files', '--force-color-profile=srgb', *args, window_size='1200,900') as h:
        c = C(port, dpr, **kw)
        try: yield c
        finally: c.close()
