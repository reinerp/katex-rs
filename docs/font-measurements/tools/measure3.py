# measure3.py PORT RUNS page.html... : every run traced; settle wall time and main-thread CPU time (tdur) of the phases; interleaved; prints JSON lines of medians/minimums
import sys, json, time, statistics, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import *
from headless_chrome import headless_chrome
port = int(sys.argv[1]); runs = int(sys.argv[2]); pages = sys.argv[3:]
NAMES = ['ParseHTML', 'UpdateLayoutTree', 'Layout', 'PrePaint', 'Paint', 'RunTask']
def load(): return float(open('/proc/loadavg').read().split()[0])
def run_one(page):
    t = new_tab(port); ws = WS(t['webSocketDebuggerUrl'])
    ws.call('Page.enable'); ws.call('Runtime.enable'); ws.call('Performance.enable')
    ws.call('Tracing.start', categories='-*,devtools.timeline,disabled-by-default-devtools.timeline', transferMode='ReportEvents')
    ws.call('Page.navigate', url='file://' + os.path.abspath(page))
    ws.pump('Page.loadEventFired', 120)
    while True:
        v = ws.call('Runtime.evaluate', expression='window.__settled===true', returnByValue=True)['result'].get('value')
        if v: break
        time.sleep(0.05)
    ev = json.loads(ws.call('Runtime.evaluate', expression='JSON.stringify(window.__ev)', returnByValue=True)['result']['value'])
    ws.call('Tracing.end'); evs = []
    while True:
        r = ws.recv()
        if r.get('method') == 'Tracing.dataCollected': evs += r['params']['value']
        elif r.get('method') == 'Tracing.tracingComplete': break
    main = set()
    for e in evs:
        if e.get('name') == 'thread_name' and e.get('args', {}).get('name') == 'CrRendererMain': main.add((e['pid'], e['tid']))
    byname = {}
    for e in evs:
        if e.get('ph') == 'X' and (e['pid'], e['tid']) in main and e['name'] in NAMES: byname.setdefault(e['name'], []).append((e['ts'], e['ts'] + e['dur'], e.get('tdur', 0)))
    out = {}
    for n, iv in byname.items():
        iv.sort(); wall = cpu = 0; end = -1
        for s, f, c in iv:   # outermost only
            if s >= end: wall += f - s; cpu += c; end = f
            elif f > end: end = f
        out[n] = (wall / 1000, cpu / 1000)
    ws.s.close(); close_tab(port, t['id'])
    return ev.get('settled'), out
with headless_chrome(port, '--disable-extensions', '--disable-background-timer-throttling', '--allow-file-access-from-files', window_size='1200,900'):
    for page in pages: run_one(page)
    res = {p: [] for p in pages}
    for r in range(runs):
        for page in pages:
            s, o = run_one(page); res[page].append((s, o, load()))
    for page in pages:
        rs = res[page]
        d = {'page': os.path.basename(page)[:-5], 'settle_med': round(statistics.median(r[0] for r in rs)), 'settle_min': round(min(r[0] for r in rs)), 'load': round(statistics.median(r[2] for r in rs), 1)}
        for n in NAMES:
            d[n + '_wall'] = round(statistics.median(r[1].get(n, (0, 0))[0] for r in rs))
            d[n + '_cpu'] = round(statistics.median(r[1].get(n, (0, 0))[1] for r in rs))
            d[n + '_cpumin'] = round(min(r[1].get(n, (0, 0))[1] for r in rs))
        print(json.dumps(d), flush=True)
