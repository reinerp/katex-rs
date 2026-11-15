# measure.py PORT RUNS page.html... : settle time + chrome metrics per page (median of RUNS), then one traced run per page.
import sys, json, time, statistics, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import *
from headless_chrome import headless_chrome
port = int(sys.argv[1]); runs = int(sys.argv[2]); pages = sys.argv[3:]
def load():
    return open('/proc/loadavg').read().split()[:3]
def run_one(page, trace=False):
    t = new_tab(port); ws = WS(t['webSocketDebuggerUrl'])
    ws.call('Page.enable'); ws.call('Runtime.enable'); ws.call('Performance.enable')
    if trace:
        ws.call('Tracing.start', categories='-*,devtools.timeline,disabled-by-default-devtools.timeline', transferMode='ReportEvents')
    t0 = time.time()
    ws.call('Page.navigate', url='file://' + os.path.abspath(page))
    ws.pump('Page.loadEventFired', 120)
    while True:
        v = ws.call('Runtime.evaluate', expression='window.__settled===true', returnByValue=True)['result'].get('value')
        if v: break
        time.sleep(0.05)
    wall = time.time() - t0
    ev = ws.call('Runtime.evaluate', expression='JSON.stringify(window.__ev)', returnByValue=True)['result']['value']
    ev = json.loads(ev)
    m = {x['name']: x['value'] for x in ws.call('Performance.getMetrics')['metrics']}
    out = {'wall_s': wall, 'ev': ev, 'metrics': {k: m[k] for k in ('LayoutDuration', 'RecalcStyleDuration', 'ScriptDuration', 'TaskDuration', 'LayoutCount', 'RecalcStyleCount', 'Nodes', 'JSHeapUsedSize') if k in m}}
    if trace:
        ws.call('Tracing.end'); evs = []
        while True:
            e = ws.pump_any = None
            r = ws.recv()
            if r.get('method') == 'Tracing.dataCollected': evs += r['params']['value']
            elif r.get('method') == 'Tracing.tracingComplete': break
        out['trace'] = split(evs)
    ws.s.close(); close_tab(port, t['id'])
    return out
NAMES = ['ParseHTML', 'UpdateLayoutTree', 'Layout', 'PrePaint', 'Paint', 'Layerize', 'Commit', 'FunctionCall', 'EvaluateScript', 'FireAnimationFrame', 'RunTask', 'CompositeLayers', 'IntersectionObserverController::computeIntersections', 'UpdateLayerTree']
def split(evs):
    # outermost-only sum per name on the renderer main thread (threads with CrRendererMain)
    main = set(); 
    for e in evs:
        if e.get('name') == 'thread_name' and e.get('args', {}).get('name') == 'CrRendererMain': main.add((e['pid'], e['tid']))
    byname = {}
    for e in evs:
        if e.get('ph') == 'X' and (e['pid'], e['tid']) in main and e['name'] in NAMES: byname.setdefault(e['name'], []).append((e['ts'], e['ts'] + e['dur']))
    out = {}
    for n, iv in byname.items():
        iv.sort(); tot = 0; end = -1
        for s, f in iv:
            if s >= end: tot += f - s; end = f
            elif f > end: tot += f - end; end = f
        out[n] = [round(tot / 1000, 1), len(iv)]
    return out
with headless_chrome(port, '--disable-extensions', '--disable-background-timer-throttling', '--allow-file-access-from-files', window_size='1200,900'):
    print('load before', load(), flush=True)
    for page in pages: run_one(page)   # warm: fonts, disk cache
    results = {p: [] for p in pages}
    for r in range(runs):               # interleaved, so slow phases of the shared machine hit every page alike
        for page in pages:
            results[page].append(dict(run_one(page), load=load()[0]))
        print('round', r, load(), flush=True)
    for page in pages:
        rs = results[page]
        st = sorted(r['ev']['settled'] for r in rs)
        med = lambda f: statistics.median(f(r) for r in rs)
        summary = {'page': os.path.basename(page), 'size': os.path.getsize(page), 'settled_ms_median': round(statistics.median(st), 0), 'settled_ms_min': round(st[0]), 'settled_ms_all': [round(x) for x in st],
                   'load_ms': round(med(lambda r: r['ev']['load'])), 'layout_ms': round(1000 * med(lambda r: r['metrics']['LayoutDuration'])),
                   'style_ms': round(1000 * med(lambda r: r['metrics']['RecalcStyleDuration'])), 'script_ms': round(1000 * med(lambda r: r['metrics']['ScriptDuration'])),
                   'task_ms': round(1000 * med(lambda r: r['metrics']['TaskDuration'])), 'nodes': rs[0]['metrics'].get('Nodes'), 'layouts': rs[0]['metrics'].get('LayoutCount'),
                   'jsrender_ms': rs[0]['ev'].get('jsrender'), 'loadavg_runs': [r['load'] for r in rs]}
        tr = run_one(page, trace=True)
        summary['trace_ms_count'] = tr['trace']
        print(json.dumps(summary), flush=True)
