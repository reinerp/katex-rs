# mfaccents.py OUT.json : extract KaTeX JS glyph lists of every accent x letter and derive overlay (composite) specs
import sys, json, collections, statistics
sys.path.insert(0, '/home/agents/scratch/katex-font/tools')
from cu import *
src = open('/home/agents/scratch/katex-font/tools/mfextract.py').read().split("c.goto(page)")[1].split("''', await_=True)")[0].split("c.ev('''")[1]
ACC = ['hat', 'bar', 'tilde', 'vec', 'dot', 'ddot', 'acute', 'grave', 'check', 'breve']
LET = [chr(c) for c in range(97, 123)] + [chr(c) for c in range(65, 91)]
tex = ['\\%s{%s}' % (a, l) for a in ACC for l in LET]
with chrome(9561) as c:
    c.goto('/home/agents/scratch/katex-font/data/ex.html'); c.ev(src, await_=True)
    res = []
    for i in range(0, len(tex), 100):
        res += json.loads(c.ev('JSON.stringify(%s.map(t=>Object.assign({tex:t},extract(t,false))))' % json.dumps(tex[i:i + 100])))
ov = collections.defaultdict(list); bad = collections.Counter()
for o in res:
    if 'g' not in o: bad['err'] += 1; continue
    g = o['g']
    if o['svg'] or o['r']: bad['svg/rule ' + o['tex'].split('{')[0]] += 1; continue
    ab = [x for x in g if x[6] == 'accbody']; bs = [x for x in g if x[6] != 'accbody']
    if len(ab) == 1 and len(bs) == 1 and len(g) == 2:
        ov[('%s:%d' % (bs[0][0], bs[0][1]), '%s:%d' % (ab[0][0], ab[0][1]), ab[0][2])].append((ab[0][3] - bs[0][3], ab[0][4] - bs[0][4], o['box'][0], bs[0][5]))
    else: bad['shape ' + o['tex'].split('{')[0]] += 1
spec = []
for (b, a, s), v in sorted(ov.items()):
    spec.append({'base': b, 'over': a, 'size': s, 'dx': round(statistics.median(x[0] for x in v), 5), 'dy': round(statistics.median(x[1] for x in v), 5)})
print('formulas', len(res), 'overlay combos', len(spec), 'not expressible', dict(bad))
json.dump(spec, open(sys.argv[1], 'w'))
print(collections.Counter(s['over'] for s in spec).most_common())
