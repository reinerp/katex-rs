# fontcmp.py OLD.map.json NEW.map.json : every glyph of the new font has the advance (and italic correction, size, shift, gaps) it has in the old font; what is missing
import json, sys
o = json.load(open(sys.argv[1])); n = json.load(open(sys.argv[2]))
bad = 0; miss = {'base': 0, 'var': 0, 'baked': 0}
for f, d in n['glyphs'].items():
    for ch, e in d.items():
        a = o['glyphs'][f].get(ch)
        if a is None: bad += 1; print('new base glyph not in old', f, ch); continue
        if a['adv'] != e['adv'] or a.get('ic') != e.get('ic') or ('noic' in a) != ('noic' in e): bad += 1; print('base differs', f, ch, a, e)
for f, d in o['glyphs'].items():
    miss['base'] += sum(1 for ch in d if ch not in n['glyphs'].get(f, {}))
ov = {(v['size'], v['shift']): v['glyphs'] for v in o['variants']}; nv = {(v['size'], v['shift']): v['glyphs'] for v in n['variants']}
if set(ov) != set(nv): bad += 1; print('pairs differ', set(ov) ^ set(nv))
for k, g in nv.items():
    for gk, (cp, adv) in g.items():
        a = ov[k].get(gk)
        if a is None or a[1] != adv: bad += 1; print('variant differs', k, gk, a, adv)
    miss['var'] += sum(1 for gk in ov[k] if gk not in g)
ob = {tuple(b[:6]): b[7] for b in o['baked']}; nb = {tuple(b[:6]): b[7] for b in n['baked']}
for k, adv in nb.items():
    if ob.get(k) != adv: bad += 1; print('baked differs', k, ob.get(k), adv)
miss['baked'] = sum(1 for k in ob if k not in nb)
for nm, (cp, adv) in n['spacers']['named'].items():
    if o['spacers']['named'][nm][1] != adv: bad += 1; print('named differs', nm)
print('differences:', bad, '| old glyphs not in the new font:', miss, '| new glyphs total: base', sum(len(d) for d in n['glyphs'].values()), 'variants', sum(len(g) for g in nv.values()), 'baked', len(nb))
