# mfcent.py DIR ROWH DPR : per formula, the shift (device px) of the ink centroid of the hybrid rendering against KaTeX, and the ratio of the ink; joins nothing, writes DIR/cent<DPR>.json
import sys, json, subprocess, glob
d, rowh, dpr = sys.argv[1], sys.argv[2], sys.argv[3]
labels = [m['l'] for m in json.load(open(d + '/metrics.json'))]
out = {}; i = 0
for p in sorted(glob.glob('%s/k_*_dpr%s.png' % (d, dpr))):
    r = json.loads(subprocess.run(['node', '/home/agents/scratch/katex-font/tools/pngcent.mjs', p, p.replace('/k_', '/f_'), rowh, dpr], stdout=subprocess.PIPE, check=True).stdout)
    for b in r[:40]:
        if i < len(labels): out[labels[i]] = dict(dx=b[3] - b[0], dy=b[4] - b[1], ink=(b[5] / b[2]) if b[2] else 1)
        i += 1
json.dump(out, open('%s/cent%s.json' % (d, dpr), 'w'))
