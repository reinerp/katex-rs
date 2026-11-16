# wrappage.py OUT.html DATA.jsonl CSS WIDTH : every corpus formula inline in a paragraph of WIDTH px (16px text), for line breaking comparisons
import json, sys, os
R = '/home/agents/scratch/katex-harden/'
out, data, css, width = sys.argv[1:5]
F = [json.loads(l) for l in open(R + 'data/formulas.jsonl')]
res = {j['id']: j for j in map(json.loads, open(data))}
style = 'body{margin:0;font:16px/1.5 system-ui,sans-serif}.row{width:%spx;padding:6px 20px;margin:0;border-bottom:1px solid #ccc;word-break:normal}' % width
rows = ''.join('<p class=row data-id=%d>Some words before, %s and some words after, so that this line goes on for a while.</p>' % (f['id'], res[f['id']].get('html', '')) for f in F)
open(out, 'w').write('<!doctype html><meta charset=utf-8><link rel=stylesheet href="file://%s"><style>%s</style>%s' % (os.path.abspath(css), style, rows))
