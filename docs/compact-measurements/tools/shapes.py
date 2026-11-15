import json, sys, collections, re
from html.parser import HTMLParser
class P(HTMLParser):
    def __init__(s): super().__init__(convert_charrefs=False); s.c = collections.Counter(); s.b = collections.Counter(); s.inhtml = 0; s.stk = []
    def handle_starttag(s, tag, attrs):
        d = dict(attrs); cls = d.get('class', '')
        s.stk.append(tag)
        if 'katex-mathml' in cls.split(): s.inhtml = -1
        if s.inhtml < 0: return
        props = sorted(x.split(':')[0] for x in (d.get('style') or '').split(';') if ':' in x)
        key = (tag, cls, ','.join(props), ','.join(k for k in d if k not in ('class', 'style')))
        s.c[key] += 1; s.b[key] += len(s.get_starttag_text()) + len(tag) + 3
    def handle_endtag(s, tag):
        if s.stk: s.stk.pop()
rows = [json.loads(l) for l in open(sys.argv[1])]
p = P()
for r in rows:
    if 'html' in r: p.inhtml = 0; p.stk = []; h = re.sub(r'<span class="katex-mathml">.*?</math></span>', '', r['html'], flags=re.S); p.feed(h)
tot = sum(p.b.values())
for k, v in p.b.most_common(int(sys.argv[2])): print('%8d %5.1f%% n=%6d  %s' % (v, 100 * v / tot, p.c[k], k))
