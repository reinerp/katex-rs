# cpcmp.py OLD.map NEW.map OLD.jsonl NEW.jsonl : the hybrid output of the new font equals the old output with every glyph code point replaced by the same glyph's code point in the new font
import json, sys
def keys(mp):
    m = json.load(open(mp)); d = {}
    for f, g in m['glyphs'].items():
        for ch, e in g.items():
            d[e['cp']] = ('b', f, ch)
            if 'noic' in e: d[e['noic']] = ('n', f, ch)
    for v in m['variants']:
        for k, (cp, adv) in v['glyphs'].items(): d[cp] = ('v', k, v['size'], v['shift'])
    for b in m['baked']: d[b[6]] = ('k', b[0], b[1], b[2], b[3], b[4], b[5])
    for nm, (cp, adv) in m['spacers']['named'].items(): d[cp] = ('s', round(adv * 4000))
    for k, cp in m['spacers']['pos'].items(): d[cp] = ('p', k)
    for k, cp in m['spacers']['neg'].items(): d[cp] = ('q', k)
    return d
o = keys(sys.argv[1]); n = keys(sys.argv[2])
inv = {}
for cp, k in n.items():
    assert k not in inv, ('duplicate key in new font', k); inv[k] = cp
ol = [json.loads(l)['html'] for l in open(sys.argv[3])]; nl = [json.loads(l)['html'] for l in open(sys.argv[4])]
bad = 0; chars = 0
def pua(c): return 0xE000 <= ord(c) <= 0xF8FF or ord(c) >= 0xF0000
for i, (a, b) in enumerate(zip(ol, nl)):
    exp = []
    for c in a:
        if pua(c) and ord(c) in o:
            k = o[ord(c)]; chars += 1
            if k not in inv: exp.append('?'); continue    # a glyph the new font does not have
            exp.append(chr(inv[k]))
        else: exp.append(c)
    if ''.join(exp) != b:
        bad += 1
        if bad <= 3: print('formula', i, 'differs'); print(' exp', ''.join(exp)[:300]); print(' got', b[:300])
print('formulas', len(ol), 'differ', bad, '| glyph code points compared', chars)
