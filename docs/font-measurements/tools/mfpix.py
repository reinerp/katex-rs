# mfpix.py TESTDIR ROWH : per-formula differing pixels (KaTeX JS vs font) from mftest's screenshots; joins metrics.json
import sys, os, json, subprocess, glob
d, rowh = sys.argv[1], sys.argv[2]
met = json.load(open(d + '/metrics.json')); by = {m['l']: m for m in met}
labels = [m['l'] for m in met]
ex = {}
for dpr in (1, 2):
    pages = sorted(glob.glob('%s/k_*_dpr%d.png' % (d, dpr)))
    if not pages: continue
    i = 0
    for p in pages:
        f = p.replace('/k_', '/f_')
        r = json.loads(subprocess.run(['node', '/home/agents/scratch/katex-font/tools/pngdiff.mjs', p, f, rowh, str(dpr)], stdout=subprocess.PIPE, check=True).stdout)
        for band in r[:40]:
            if i < len(labels): by[labels[i]]['px%d' % dpr] = band[0]; by[labels[i]]['mx%d' % dpr] = band[1]; by[labels[i]]['big%d' % dpr] = band[2]; by[labels[i]]['sum%d' % dpr] = band[3]
            i += 1
json.dump(met, open(d + '/metrics_px.json', 'w'))
G = {str(o['id']): o for o in json.load(open('/home/agents/scratch/katex-font/data/glyphs_inline.json')) if 'g' in o}
def cls(m):
    o = G.get(m['l'])
    if o is None: return 'tex'
    return 'norule' if not o['r'] and not o['svg'] else 'rule'
import collections
for dpr in (1, 2):
    rows = [m for m in met if 'px%d' % dpr in m and not m['missing'] and cls(m) in ('norule', 'tex')]
    if not rows: continue
    n = len(rows); z = sum(1 for m in rows if m['px%d' % dpr] == 0)
    print('dpr%d: font-only formulas %d: identical %d (%.1f%%), <=10px %d, <=50px %d, >200px %d; max px %d; max channel diff median %s' % (dpr, n, z, 100 * z / n, sum(1 for m in rows if m['px%d' % dpr] <= 10), sum(1 for m in rows if m['px%d' % dpr] <= 50), sum(1 for m in rows if m['px%d' % dpr] > 200), max(m['px%d' % dpr] for m in rows), sorted(m['mx%d' % dpr] for m in rows)[n // 2]))
