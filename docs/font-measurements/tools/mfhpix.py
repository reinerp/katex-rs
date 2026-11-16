# mfhpix.py DIR ROWH : per-formula differing pixels of mfhyb.py's screenshots; joins metrics.json -> metrics_px.json, prints a summary
import sys, json, subprocess, glob, statistics, collections
d, rowh = sys.argv[1], sys.argv[2]
met = json.load(open(d + '/metrics.json')); labels = [m['l'] for m in met]; by = {m['l']: m for m in met}
for dpr in (1, 2):
    pages = sorted(glob.glob('%s/k_*_dpr%d.png' % (d, dpr)))
    if not pages: continue
    i = 0
    for p in pages:
        r = json.loads(subprocess.run(['node', '/home/agents/scratch/katex-font/tools/pngdiff.mjs', p, p.replace('/k_', '/f_'), rowh, str(dpr)], stdout=subprocess.PIPE, check=True).stdout)
        for band in r[:40]:
            if i < len(labels):
                m = by[labels[i]]; m['px%d' % dpr] = band[0]; m['mx%d' % dpr] = band[1]; m['big%d' % dpr] = band[2]
            i += 1
json.dump(met, open(d + '/metrics_px.json', 'w'))
for dpr in (1, 2):
    rows = [m for m in met if 'px%d' % dpr in m and m['wk'] > 0]
    if not rows: continue
    n = len(rows); big = [m['big%d' % dpr] for m in rows]
    print('dpr%d: %d formulas: identical %d (%.1f%%), no pixel >64/255: %d (%.1f%%), median differing px %d, >200 differing px: %d, >1000: %d' % (dpr, n, sum(1 for m in rows if m['px%d' % dpr] == 0), 100 * sum(1 for m in rows if m['px%d' % dpr] == 0) / n,
          sum(1 for b in big if b == 0), 100 * sum(1 for b in big if b == 0) / n, sorted(m['px%d' % dpr] for m in rows)[n // 2], sum(1 for m in rows if m['px%d' % dpr] > 200), sum(1 for m in rows if m['px%d' % dpr] > 1000)))
