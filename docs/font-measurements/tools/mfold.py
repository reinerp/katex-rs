# mfold.py OLD.jsonl OLD.css OLD.woff2 NEW.jsonl NEW.css NEW.woff2 MODE OUTDIR [--dpr 1,2] [--ids a,b,c|all]: the same formulas rendered with two versions of the
# font and the encoder, side by side (rows of 80 px: old / new), pixels compared at device scale 1 and 2
import sys, os, json, re, argparse, subprocess
sys.path.insert(0, '/home/agents/scratch/katex-font/tools')
from cu import *
ap = argparse.ArgumentParser()
for n in ('oldj', 'oldcss', 'oldfont', 'newj', 'newcss', 'newfont', 'mode', 'out'): ap.add_argument(n)
ap.add_argument('--dpr', default='1,2'); ap.add_argument('--ids', default='all'); ap.add_argument('--port', type=int, default=9646); ap.add_argument('--width', type=int, default=0); ap.add_argument('--ch', type=int, default=40)
a = ap.parse_args(); os.makedirs(a.out, exist_ok=True)
def load(p):
    return {j['id']: j['html'] for j in map(json.loads, open(p)) if 'html' in j}
O = load(a.oldj); N = load(a.newj)
ids = [i for i in sorted(O) if i in N and (a.ids == 'all' or str(i) in a.ids.split(','))]
def css(path, woff): return open(path).read().replace('url(MathRun.woff2)', 'url(file://%s)' % os.path.abspath(woff)).replace('url(fonts/', 'url(file:///home/agents/scratch/katex-compact/css/fonts/')
base = 'body{margin:0;font:16px/80px system-ui,sans-serif;background:#fff;color:#000}.row{height:80px;line-height:80px;padding:0 20px;box-sizing:border-box;overflow:hidden;position:relative;width:max-content;min-width:100%%}' + ('.katex-display{margin:0!important}' if a.mode == 'display' else '')
CH = a.ch
for tag, d, c in (('o', O, css(a.oldcss, a.oldfont)), ('n', N, css(a.newcss, a.newfont))):
    for ci in range(0, len(ids), CH):
        open('%s/%s_%03d.html' % (a.out, tag, ci // CH), 'w').write('<!doctype html><meta charset=utf-8><style>%s%s</style><div>%s</div>' % (base, c, ''.join('<div class=row>%s</div>' % d[i] for i in ids[ci:ci + CH])))
with chrome(a.port) as c:
    for dpr in [int(x) for x in a.dpr.split(',')]:
        c.ws.call('Emulation.setDeviceMetricsOverride', width=a.width or 1400, height=CH * 80, deviceScaleFactor=dpr, mobile=False)
        for ci in range(0, len(ids), CH):
            for tag in 'on':
                c.goto('%s/%s_%03d.html' % (a.out, tag, ci // CH)); c.shot('%s/%s_%03d_dpr%d.png' % (a.out, tag, ci // CH, dpr))
res = {}
for dpr in [int(x) for x in a.dpr.split(',')]:
    rows = []
    for ci in range(0, len(ids), CH):
        r = json.loads(subprocess.run(['node', '/home/agents/scratch/katex-font/tools/pngdiff.mjs', '%s/o_%03d_dpr%d.png' % (a.out, ci // CH, dpr), '%s/n_%03d_dpr%d.png' % (a.out, ci // CH, dpr), '80', str(dpr)], stdout=subprocess.PIPE, check=True).stdout)
        rows += r[:len(ids[ci:ci + CH])]
    res[dpr] = dict(zip(ids, rows))
    n = len(rows)
    print('dpr%d: %d formulas: identical %d (%.1f%%), no pixel >64/255 different %d (%.1f%%), median differing px %d, >200 differing px: %d' % (dpr, n, sum(1 for r in rows if r[0] == 0), 100 * sum(1 for r in rows if r[0] == 0) / n,
          sum(1 for r in rows if r[2] == 0), 100 * sum(1 for r in rows if r[2] == 0) / n, sorted(r[0] for r in rows)[n // 2], sum(1 for r in rows if r[0] > 200)))
json.dump({str(k): {str(i): v for i, v in d.items()} for k, d in res.items()}, open(a.out + '/pix.json', 'w'))
