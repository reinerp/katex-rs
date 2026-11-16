# mfstock.py RS_JSONL MODE : what is left outside font runs: stock characters by the classes of their element chain, and what the text of those formulas is
import sys, json, re, collections, os
from html.parser import HTMLParser
sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from mathfont_encode import Encoder
D = '/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/'
enc = Encoder([D + f for f in os.listdir(D) if f.endswith('.map.json')][0])
glyph = set()
for (f, c), (cp, a, ic, noic, alt) in enc.base.items(): glyph.add(cp); noic and glyph.add(noic)
for k, (cp, a) in enc.var.items(): glyph.add(cp)
spacers = set(enc.pos.values()) | set(enc.neg.values()) | {cp for n, cp in enc.named}
class P(HTMLParser):
    def __init__(s): super().__init__(); s.stack = []; s.stock = collections.Counter(); s.run = 0
    def handle_starttag(s, tag, attrs):
        if tag in ('wbr', 'br', 'img', 'path', 'line'): return
        s.stack.append((tag, dict(attrs).get('class', '')))
    def handle_endtag(s, tag):
        if s.stack and s.stack[-1][0] == tag: s.stack.pop()
    def handle_data(s, d):
        for ch in d:
            o = ord(ch)
            if o in spacers or ord(ch) in (0x200b,): continue
            if o in glyph: s.run += 1; continue
            if ch in ' \n': continue
            cls = [c for t, c in s.stack if c and c not in ('katex','katex-display') and not re.match(r'^(mf|t\w+|f)$', c)]
            key = ' '.join(cls[-2:]) if cls else '(root)'
            s.stock[key] += 1
tot = collections.Counter(); run = 0
for l in open(sys.argv[1]):
    j = json.loads(l)
    if 'html' not in j: continue
    p = P(); p.feed(j['html']); run += p.run; tot.update(p.stock)
n = sum(tot.values())
print('run glyphs %d, stock chars %d (%.1f%%)' % (run, n, 100 * n / (run + n)))
for k, v in tot.most_common(25): print('  %5d %s' % (v, k))
