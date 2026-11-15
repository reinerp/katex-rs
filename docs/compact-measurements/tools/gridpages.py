# gridpages.py NAME DATA.jsonl CSS   -> pages/g_NAME_NNN.html : 25 formulas per page, 200px rows (as katex-harden/pages)
import json, os, sys
R = '/home/agents/scratch/katex-harden/'
name, data, css = sys.argv[1:4]
F = [json.loads(l) for l in open(R + 'data/formulas.jsonl')]
res = {j['id']: j for j in map(json.loads, open(data))}
ROW, PER = 200, 25
head = '<!doctype html><meta charset=utf-8><link rel=stylesheet href="file://%s"><style>body{margin:0;background:#fff;font-size:20px}.r{height:%dpx;overflow:hidden;padding:20px;box-sizing:border-box;white-space:nowrap}</style>' % (os.path.abspath(css), ROW)
os.makedirs('/home/agents/scratch/katex-compact/pages', exist_ok=True)
for p in range(0, len(F), PER):
    rows = ''.join('<div class=r>%s</div>' % res[f['id']].get('html', '') for f in F[p:p + PER])
    open('/home/agents/scratch/katex-compact/pages/g_%s_%03d.html' % (name, p // PER), 'w').write(head + rows)
print('pages', (len(F) + PER - 1) // PER)
