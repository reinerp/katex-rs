# geocmp.py REF.json CAND.json [ctx] : glyph / box geometry differences (CSS px; also em of .katex = 19.36px, and device px at scale 2)
import json, sys, statistics as st
ctx = len(sys.argv) > 3
a = {j['id']: j for j in json.load(open(sys.argv[1]))}; b = {j['id']: j for j in json.load(open(sys.argv[2]))}
KFS = 16 * 1.21
def pct(v, q): v = sorted(v); return v[min(len(v) - 1, int(q * len(v)))] if v else 0
if ctx:
    dh = []; db = []; dk = []
    for i in a:
        x, y = a[i], b[i]
        dh.append(abs(x['h'] - y['h'])); db.append(abs((x['base']) - (y['base'])))
        if x['katex'] and y['katex']: dk.append(max(abs(p - q) for p, q in zip(x['katex'], y['katex'])))
    for name, v in (('paragraph height', dh), ('baseline y', db), ('katex box', dk)):
        print('%-18s differing %4d / %d  max %.4f px  p99 %.4f  median %.4f' % (name, sum(1 for t in v if t > 1e-3), len(v), max(v), pct(v, .99), st.median(v)))
else:
    sh = []; per = []; mism = 0; wd = []; xs = []
    for i in a:
        ca, cb = a[i]['chars'], b[i]['chars']
        if len(ca) != len(cb): mism += 1; continue
        m = 0
        for p, q in zip(ca, cb):
            d = max(abs(p[0] - q[0]), abs(p[1] - q[1])); sh.append(d); m = max(m, d); xs.append((p[0], d))
        per.append(m)
        if a[i]['katex'] and b[i]['katex']: wd.append(abs((a[i]['katex'][2] - a[i]['katex'][0]) - (b[i]['katex'][2] - b[i]['katex'][0])))
    print('formulas compared %d (char count mismatch %d), glyphs %d' % (len(per), mism, len(sh)))
    print('glyph shift (px, base 16px): nonzero %d  median %.5f  p99 %.5f  max %.5f   (em of .katex: p99 %.5f max %.5f; device px @2x: p99 %.4f max %.4f)' % (
        sum(1 for t in sh if t > 1e-4), st.median(sh), pct(sh, .99), max(sh), pct(sh, .99) / KFS, max(sh) / KFS, 2 * pct(sh, .99), 2 * max(sh)))
    print('per-formula max shift: median %.5f  p99 %.5f  max %.5f  formulas with shift > 0.25 css px: %d' % (st.median(per), pct(per, .99), max(per), sum(1 for t in per if t > .25)))
    print('formula width change (px): median %.5f p99 %.5f max %.5f' % (st.median(wd), pct(wd, .99), max(wd)))
    bins = {}
    for x, d in xs: bins.setdefault(int(x // 150), []).append(d)
    print('mean glyph shift by x position (150px bins):', ' '.join('%d:%.4f' % (k * 150, sum(v) / len(v)) for k, v in sorted(bins.items()) if len(v) > 50))
