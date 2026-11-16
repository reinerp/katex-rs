import sys, re, collections
from html.parser import HTMLParser
C = collections.Counter(); T = [0]; inside = [0]; stack = []
class P(HTMLParser):
    def handle_starttag(s, tag, attrs):
        d = dict(attrs); cl = d.get('class', '').split()
        stack.append(tag)
        if 'katex' in cl: inside[0] += 1
        if not inside[0]: return
        cl2 = [re.sub(r'^(g|i|u|h|a|y|l|r|q|m|w|x|z|t)[0-9a-z]+$', r'\1*', c) for c in cl if c != 'katex']
        cl2 = [re.sub(r'^k[0-9a-z]+$', 'k*', c) if c != 'katex' else c for c in cl2]
        C[(tag, ' '.join(sorted(cl2)), 'S' if 'style' in d else '')] += 1
    def handle_endtag(s, tag):
        if stack: stack.pop()
    def handle_data(s, data):
        if inside[0]: T[0] += 1
s = open(sys.argv[1]).read(); s = s[s.index('<body>'):]
# crude: count only within katex spans by regex splitting
p = P(convert_charrefs=False)
# treat whole body; mark inside by class katex nesting approximated: all elements inside <p> that have katex ancestors -> we simply count everything except p/div
inside[0] = 1
p.feed(s)
tot = sum(C.values())
print('elements', tot, 'texts', T[0])
for k, v in C.most_common(int(sys.argv[2])): print('%7d %5.1f%%  %s' % (v, 100*v/tot, k))
