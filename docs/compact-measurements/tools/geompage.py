# geompage.py OUT.html DATA.jsonl CSS [--context] : all corpus formulas, one per row, 16px base (nowrap rows), for geometry probes.
# --context: each formula inline in a paragraph of text (560px wide, line-height 1.5) with a zero-size baseline marker before it.
import json, sys, os
R = '/home/agents/scratch/katex-harden/'
out, data, css = sys.argv[1:4]; ctx = '--context' in sys.argv
F = [json.loads(l) for l in open(R + 'data/formulas.jsonl')]
res = {j['id']: j for j in map(json.loads, open(data))}
if ctx:
    style = 'body{margin:0;font:16px/1.5 system-ui,sans-serif}.row{width:560px;padding:6px 20px;margin:0;border-bottom:1px solid #ccc}.bl{display:inline-block;width:0;height:0}'
    rows = ''.join('<p class=row data-id=%d>Some words before, <i class=bl></i>%s and some words after, so that this line goes on for a while.</p>' % (f['id'], res[f['id']].get('html', '')) for f in F)
else:
    style = 'body{margin:0;font:16px/1.5 system-ui,sans-serif}.row{white-space:nowrap;padding:10px 20px;position:relative;border-bottom:1px solid #ccc}'
    rows = ''.join('<div class=row data-id=%d>%s</div>' % (f['id'], res[f['id']].get('html', '')) for f in F)
open(out, 'w').write('<!doctype html><meta charset=utf-8><link rel=stylesheet href="file://%s"><style>%s</style>%s' % (os.path.abspath(css), style, rows))
