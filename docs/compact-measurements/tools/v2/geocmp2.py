# geocmp2.py REF.json CAND.json [ctx]: like geocmp.py, but a glyph's rectangle is compared by left and width only (the text box's top and height are those of the font of the run, which differ when a run is in another font family although the glyph is at the same place); also reports the max over glyphs and counts
import json, sys, statistics as st
ctx = len(sys.argv) > 3
a = {j['id']: j for j in json.load(open(sys.argv[1]))}; b = {j['id']: j for j in json.load(open(sys.argv[2]))}
def pct(v, q): v = sorted(v); return v[min(len(v) - 1, int(q * len(v)))] if v else 0
if ctx:
    dh = []; db = []; dk = []
    for i in a:
        x, y = a[i], b[i]
        dh.append(abs(x['h'] - y['h'])); db.append(abs(x['base'] - y['base']))
        if x['katex'] and y['katex']: dk.append(max(abs(p - q) for p, q in zip((x['katex'][0], x['katex'][2]), (y['katex'][0], y['katex'][2]))))
    for name, v in (('paragraph height', dh), ('baseline y', db), ('katex box left/right', dk)):
        print('%-18s differing %4d / %d  over .05: %d  max %.4f px  p99 %.4f' % (name, sum(1 for t in v if t > 1e-3), len(v), sum(1 for t in v if t > .05), max(v), pct(v, .99)))
else:
    sh = []; per = []; mism = 0; wd = []; over = []
    for i in a:
        ca, cb = a[i]['chars'], b[i]['chars']
        if len(ca) != len(cb): mism += 1; continue
        m = 0
        for p, q in zip(ca, cb):
            d = max(abs(p[0] - q[0]), abs(p[2] - q[2])); sh.append(d); m = max(m, d)
        per.append(m)
        if m > .05: over.append((i, round(m, 3)))
        if a[i]['katex'] and b[i]['katex']: wd.append(abs((a[i]['katex'][2] - a[i]['katex'][0]) - (b[i]['katex'][2] - b[i]['katex'][0])))
    print('formulas %d (char count mismatch %d), glyphs %d, differing glyphs %d, max shift (x or width) %.4f px, p99 %.4f; formulas over 0.05 px: %d %s; formula width change max %.4f' % (len(per), mism, len(sh), sum(1 for t in sh if t > 1e-4), max(sh), pct(sh, .99), len(over), over[:8], max(wd)))
