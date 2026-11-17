# mfchars.py MAP.json OUT.jsonl... : code points per formula in the font's text (private use), how many are glyphs and how many spacers;
# per formula and per glyph; bytes (UTF-8) raw
import sys, json, statistics, glob
m = json.load(open(sys.argv[1]))
sp = set(v[0] for v in m['spacers']['named'].values()) | set(m['spacers']['pos'].values()) | set(m['spacers']['neg'].values())
named = set(v[0] for v in m['spacers']['named'].values())
for path in sys.argv[2:]:
    chars = []; glyphs = []; spacers = []; nb = []; by = []
    for l in open(path):
        j = json.loads(l)
        if 'html' not in j: continue
        h = j['html']
        pu = [ord(c) for c in h if 0xE000 <= ord(c) <= 0xF8FF]
        g = sum(1 for c in pu if c not in sp); chars.append(len(pu)); glyphs.append(g); spacers.append(len(pu) - g)
        nb.append(sum(1 for c in pu if c in named)); by.append(len(h.encode()))
    n = len(chars)
    print('%s: %d formulas, private-use characters per formula mean %.2f median %d (glyphs %.2f, spacers %.2f of which named %.2f); per glyph %.3f; bytes mean %.1f median %d; total chars %d, spacers %d' % (
        path, n, statistics.mean(chars), statistics.median(chars), statistics.mean(glyphs), statistics.mean(spacers), statistics.mean(nb),
        sum(chars) / sum(glyphs), statistics.mean(by), statistics.median(by), sum(chars), sum(spacers)))
