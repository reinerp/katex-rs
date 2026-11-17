# mferr.py MAP.json OUT.jsonl RUNS.txt : error of the glyph positions written (font units, from the code points) against the ideal ones the encoder was given
import sys, re, json, collections
sys.argv = [sys.argv[0], sys.argv[1], sys.argv[2], sys.argv[1], sys.argv[2], sys.argv[3]]
src = open('/home/agents/scratch/katex-font/tools/mfunits.py').read().split("oi, ni = load")[0]
exec(src)
info = load(sys.argv[1]); H = {}
for l in open(sys.argv[2]):
    j = json.loads(l)
    if j.get('html'): H[j['id']] = j['html']
FONTS = ['Main-Regular', 'Math-Italic', 'AMS-Regular', 'Size1-Regular', 'Size2-Regular', 'Size3-Regular', 'Size4-Regular']
ideal = collections.defaultdict(list); cur = None
for l in open(sys.argv[5]):
    if l.startswith('ID '): cur = int(l.split()[1]); continue
    if l.startswith('RUN '):
        g, w = l[4:].rsplit(' ', 1)
        ideal[cur].append([((FONTS[x[0]], x[1], round(x[2], 3), round(x[3], 3)), round(x[4] * 4000)) for x in json.loads(g)])
errs = collections.Counter(); skipped = 0
for i, runs in ideal.items():
    if i not in H: continue
    rr = [t for t in re.split(r'<[^>]*>', H[i]) if any(ord(c) in info for c in t)]
    if len(rr) != len(runs): skipped += 1; continue
    for r, idl in zip(rr, runs):
        g, w = glyphs(info, r)
        a = sorted(g, key=lambda t: (t[0], t[1])); b = sorted(idl, key=lambda t: (t[0], t[1]))
        if [x[0] for x in a] != [x[0] for x in b]: skipped += 1; continue
        for x, y in zip(a, b): errs[x[1] - y[1]] += 1
print(sum(errs.values()), 'glyphs;', 'skipped runs', skipped, 'error (units, written - ideal):', sorted(errs.items()))
