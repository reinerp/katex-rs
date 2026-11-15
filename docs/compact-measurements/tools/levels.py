# levels.py : size/node table over lever configurations. usage: levels.py [--only NAME...]
import json, subprocess, sys, gzip, re, os
from html.parser import HTMLParser
D = '/home/agents/scratch/katex-compact/'
BIN = '/home/agents/scratch/katex-harden/formulas.bin'
L = dict(UNWRAP_HTML=1, DEAD=2, SHORT=4, VALUE=8, NUM=16, GLUE=32, UNWRAP=64, MERGE=128, TEXT=256, VLIST=512, PSEUDO=1024, UNWRAP_TEXT=2048)
def lv(*names): return sum(L[n] for n in names)
STEPS = [
 ('0 current fork output (HTML + MathML)', 'legacy'),
 ('1 drop the MathML (KaTeX output=html)', 'html'),
 ('2 compact: katex-html wrapper dropped', 'compact:1:nolabel'),
 ('3 + unused classes dropped', 'compact:3:nolabel'),
 ('4 + short class names', 'compact:7:nolabel'),
 ('5 + shorter numbers', 'compact:23:nolabel'),
 ('6 + common declarations as classes', 'compact:31:nolabel'),
 ('7 + vertical lists as inline blocks', 'compact:543:nolabel'),
 ('8 + struts and pstruts as pseudo-elements', 'compact:1567:nolabel'),
 ('9 + one element for a one-item list', 'compact:5663:nolabel'),
 ('10 + group classes (= levels::EXACT)', 'compact:22047:nolabel'),
 ('11 + spacing as margins', 'compact:22079:nolabel'),
 ('12 + msupsub and size spans merged', 'compact:30399:nolabel'),
 ('13 + grouping spans removed', 'compact:30463:nolabel'),
 ('14 + text runs joined (= levels::ALL)', 'compact:32767:nolabel'),
 ('15 ALL + aria-label with the TeX', 'compact:32767'),
]
class P(HTMLParser):
    def __init__(s): super().__init__(convert_charrefs=False); s.el = 0; s.tx = 0
    def handle_starttag(s, t, a): s.el += 1
    def handle_data(s, d): s.tx += 1 if d else 0
    handle_entityref = handle_charref = lambda s, n: None
def stats(items):
    raw = sum(len(h.encode()) for h in items); p = P()
    for h in items: p.feed(h)
    blob = '\n'.join(items).encode()
    z = len(subprocess.run(['zstd', '-9', '-q', '-c'], input=blob, capture_output=True).stdout)
    return dict(n=len(items), raw=raw, zstd=z, gzip=len(gzip.compress(blob, 9)), el=p.el, tx=p.tx)
def main():
    only = sys.argv[2:] if len(sys.argv) > 1 and sys.argv[1] == '--only' else None
    out = {}
    for name, mode in STEPS:
        fn = D + 'data/lv_%s.jsonl' % re.sub(r'\W+', '_', mode)
        subprocess.run([D + 'sxb', D + 'rs/target/release/ch', 'render', BIN, fn, mode, '1'], check=True)
        rows = {j['id']: j for j in map(json.loads, open(fn))}
        ids = sorted(i for i, j in rows.items() if 'html' in j)
        corpus = [rows[i]['html'] for i in ids]
        held = [rows[i]['html'] for i in ids if i % 2]
        fixture = [rows[ids[k % len(ids)]]['html'] for k in range(4800)]
        out[name] = dict(mode=mode, corpus=stats(corpus), heldout=stats(held), fixture=stats(fixture))
        c = out[name]
        print('%-52s corpus raw %8d zstd %6d gz %6d el %6d | fixture raw %9d zstd %7d gz %7d el %7d tx %7d' % (name, c['corpus']['raw'], c['corpus']['zstd'], c['corpus']['gzip'], c['corpus']['el'], c['fixture']['raw'], c['fixture']['zstd'], c['fixture']['gzip'], c['fixture']['el'], c['fixture']['tx']), flush=True)
    json.dump(out, open(D + 'data/levels.json', 'w'), indent=1)
if __name__ == '__main__':
    main()
