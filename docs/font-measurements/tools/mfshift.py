# mfshift.py DIR ROWH : per formula, the vertical offset (-2..2 device px) of the hybrid rendering that minimises the differing pixels against KaTeX, from mfhyb.py's screenshots
import sys, json, subprocess, glob, collections
d, rowh = sys.argv[1], sys.argv[2]
labels = [m['l'] for m in json.load(open(d + '/metrics.json'))]
for dpr in (1, 2):
    c = collections.Counter(); i = 0; shifted = []
    for p in sorted(glob.glob('%s/k_*_dpr%d.png' % (d, dpr))):
        r = json.loads(subprocess.run(['node', '/home/agents/scratch/katex-font/tools/pngshift.mjs', p, p.replace('/k_', '/f_'), rowh, str(dpr)], stdout=subprocess.PIPE, check=True).stdout)
        for band in r[:40]:
            if i < len(labels):
                if band[2] == 0 and min(band) == 0: c['identical'] += 1
                else:
                    best = min(range(5), key=lambda k: (band[k], abs(k - 2)))
                    if band[best] < 0.5 * band[2]: c['dy=%+d' % (best - 2)] += 1; shifted.append(labels[i])
                    else: c['dy=0'] += 1
            i += 1
    print('dpr%d:' % dpr, dict(c), 'shifted ids (first 15):', shifted[:15])
