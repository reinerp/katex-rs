# mfhybrid.py FONTBASE OUT.jsonl : compact ALL fixture html with font spans where a formula is font-only (id, html jsonl, as mkpage.py reads)
import sys, json, collections
sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from mathfont_encode import Encoder, to_html
enc = Encoder(sys.argv[1] + '.map.json')
F = [json.loads(l) for l in open('/home/agents/scratch/katex-harden/data/formulas.jsonl')]
comp = {j['id']: j for j in map(json.loads, open('/home/agents/scratch/katex-compact/data/lv_compact_32767_nolabel.jsonl'))}
gl = {m: {o['id']: o for o in json.load(open('/home/agents/scratch/katex-font/data/glyphs_%s.json' % m)) if 'g' in o} for m in ('inline', 'display')}
out = open(sys.argv[2], 'w'); n_font = n_disp = 0; stats = {}
for f in F:
    i = f['id']; o = gl['display' if f['display'] else 'inline'].get(i)
    html = comp[i]['html']; use = False
    if o and not o['r'] and not o['svg']:
        r = enc.best(o['g'], o['box'][0])
        if not r[1]:
            span = '<span class=mf style="--h:%.3fem;--d:%.3fem">%s</span>' % (o['box'][1], o['box'][2], to_html(r[0]))
            html = '<span class=katex-display>%s</span>' % span if f['display'] else span
            use = True; n_font += 1; n_disp += f['display']
    stats[i] = use
    out.write(json.dumps({'id': i, 'html': html}) + '\n')
json.dump(stats, open(sys.argv[2] + '.stats.json', 'w'))
# fixture: corpus[k % 1128] for k in 0..4799
fx = [stats[F[k % len(F)]['id']] for k in range(4800)]
print('corpus font-only formulas %d of %d (display %d); fixture occurrences %d of 4800 = %.1f%%' % (n_font, len(F), n_disp, sum(fx), 100 * sum(fx) / 4800))
