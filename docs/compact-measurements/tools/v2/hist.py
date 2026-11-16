import json, sys, re, collections
from html.parser import HTMLParser
C = collections.Counter(); T = collections.Counter(); ntext = 0
class P(HTMLParser):
    def handle_starttag(s, tag, attrs):
        d = dict(attrs); cl = d.get('class', '').split()
        # normalise value classes
        cl2 = [re.sub(r'^(k|g|i|u|h|a|y|l|r|q|m|w|x)[0-9a-z]+$', r'\1*', c) for c in cl]
        C[(tag, ' '.join(sorted(cl2)), 'S' if 'style' in d else '')] += 1
    def handle_data(s, data):
        global ntext; ntext += 1
rows = [json.loads(l) for l in open(sys.argv[1])]
p = P(convert_charrefs=False)
for r in rows:
    if 'html' in r: p.feed(r['html'])
tot = sum(C.values())
print('elements', tot, 'texts', ntext)
for k, v in C.most_common(int(sys.argv[2])): print('%7d %5.1f%%  %s' % (v, 100*v/tot, k))
