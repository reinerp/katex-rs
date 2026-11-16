import json, sys
a = {j['id']: j for j in json.load(open(sys.argv[1]))}; b = {j['id']: j for j in json.load(open(sys.argv[2]))}
th = float(sys.argv[3]) if len(sys.argv) > 3 else 0.05
n = 0; nd = 0; tot = 0; mx = 0; bad = []
for i in a:
    ca, cb = a[i]['chars'], b[i]['chars']
    m = 0
    for p, q in zip(ca, cb):
        d = max(abs(p[0]-q[0]), abs(p[1]-q[1]), abs(p[2]-q[2]), abs(p[3]-q[3])); m = max(m, d)
        tot += 1; nd += d > 1e-6
    mx = max(mx, m)
    if m > th: bad.append((i, round(m, 4)))
print('formulas', len(a), 'glyphs', tot, 'differing glyphs', nd, 'max', mx, 'formulas over', th, ':', len(bad), bad[:12])
