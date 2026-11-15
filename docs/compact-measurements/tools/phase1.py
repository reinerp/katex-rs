# Byte/node accounting of KaTeX output. usage: phase1.py data.jsonl [more...]
import json, sys, re, collections, zlib, subprocess
from html.parser import HTMLParser
VOID = set()
class P(HTMLParser):
    def __init__(s):
        super().__init__(convert_charrefs=False)
        s.stack = []; s.c = collections.Counter(); s.tags = collections.Counter(); s.props = collections.Counter(); s.propn = collections.Counter()
        s.classes = collections.Counter(); s.nodes = 0; s.maxd = 0; s.branch = None; s.tagbytes = collections.Counter()
        s.branchnodes = collections.Counter(); s.depth_html = 0; s.maxdh = 0
    def handle_starttag(s, tag, attrs):
        raw = s.get_starttag_text(); b = s.branch or 'root'
        cls = dict(attrs).get('class') or ''
        if 'katex-mathml' in cls.split(): s.branch = 'mathml'; b = 'mathml'
        elif 'katex-html' in cls.split(): s.branch = 'html'; b = 'html'
        s.stack.append((tag, s.branch))
        s.nodes += 1; s.branchnodes[b] += 1; s.tags[(b, tag)] += 1
        s.maxd = max(s.maxd, len(s.stack))
        if b == 'html': s.maxdh = max(s.maxdh, len(s.stack))
        total = len(raw); acct = 1 + len(tag)  # '<' + name
        s.c[(b, 'tag-name')] += acct
        rest = total - acct - 1  # minus '>'
        s.c[(b, 'punct')] += 1
        for k, v in attrs:
            ab = len(k) + (len(v) + 3 if v is not None else 0) + 1  # space k="v"
            if k == 'class':
                s.c[(b, 'class-attr')] += ab
                for cl in v.split(): s.classes[cl] += 1
            elif k == 'style':
                s.c[(b, 'style-attr')] += ab
                for decl in v.split(';'):
                    if ':' in decl:
                        p, val = decl.split(':', 1); s.props[p] += len(decl) + 1; s.propn[p] += 1
            else: s.c[(b, 'other-attr:' + k)] += ab
    def handle_endtag(s, tag):
        b = s.branch or 'root'
        s.c[(b, 'end-tag')] += len(tag) + 3
        while s.stack:
            t, br = s.stack.pop()
            if t == tag: break
        if s.stack == [] : s.branch = None
        else:
            # branch resets when leaving the branch span
            s.branch = s.stack[-1][1] if s.stack else None
    def handle_data(s, d):
        b = s.branch or 'root'; s.c[(b, 'text')] += len(d.encode())
    def handle_entityref(s, n): s.handle_data('&%s;' % n)
    def handle_charref(s, n): s.handle_data('&#%s;' % n)

def comp(b):
    import gzip
    z = subprocess.run(['zstd', '-9', '-q', '-c'], input=b, capture_output=True).stdout
    return len(z), len(gzip.compress(b, 9))

def main():
    for path in sys.argv[1:]:
        rows = [json.loads(l) for l in open(path)]
        htmls = [r['html'] for r in rows if 'html' in r]
        P_ = P(); raw = 0
        for h in htmls:
            raw += len(h.encode()); P_.feed(h)
        n = len(htmls)
        print('==', path, 'formulas', n, 'raw bytes', raw, 'per formula %.0f' % (raw / n), 'nodes', P_.nodes, 'nodes/formula %.1f' % (P_.nodes / n), 'maxdepth', P_.maxd)
        tot = sum(P_.c.values()); print(' accounted', tot)
        for k, v in sorted(P_.c.items(), key=lambda kv: -kv[1])[:30]: print('  %-8s %-22s %9d  %5.1f%%' % (k[0], k[1], v, 100 * v / tot))
        print(' by branch:', {b: sum(v for k, v in P_.c.items() if k[0] == b) for b in ('root', 'mathml', 'html')}, 'nodes', dict(P_.branchnodes))
        print(' style props (bytes, count):'); [print('   %-18s %8d %7d' % (p, v, P_.propn[p])) for p, v in P_.props.most_common(15)]
        print(' top html tags:', [(k[1], v) for k, v in P_.tags.most_common(30) if k[0] == 'html'][:12])
        print(' top classes:', P_.classes.most_common(25))
        allb = '\n'.join(htmls).encode(); print(' compressed concat (zstd9, gzip9):', comp(allb))
        per = [comp(h.encode()) for h in htmls[:300]]
        print(' compressed per-formula avg (first 300) zstd %.0f gzip %.0f' % (sum(p[0] for p in per) / len(per), sum(p[1] for p in per) / len(per)))
main()
