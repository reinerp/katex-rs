# mfsizes.py OUTDIR : build the repertoire/plane configurations, report sizes and corpus coverage
import sys, os, json, subprocess, collections, zlib
sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
os.environ['BROTLI'] = '/home/agents/scratch/katex-font/brotli'
import gen_math_font as G, sfnt
from mathfont_encode import Encoder
out = sys.argv[1]; os.makedirs(out, exist_ok=True)
M4 = 'Main-Regular,Math-Italic,AMS-Regular,Size1-Regular'
M7 = M4 + ',Size2-Regular,Size3-Regular,Size4-Regular'
FULL = ','.join(G.ALL_FONTS)
data = {m: [o for o in json.load(open('/home/agents/scratch/katex-font/data/glyphs_%s.json' % m)) if 'g' in o] for m in ('inline', 'display')}
M3 = 'Main-Regular,Math-Italic,Size1-Regular'
R = M3 + ',AMS-Regular,Size2-Regular,Size3-Regular,Size4-Regular'
configs = [('M3 base only', M3, 0, 0, ''), ('M3 p2', M3, 2, 2, ''), ('M3 p8', M3, 8, 8, ''), ('M3 p16', M3, 16, 16, ''),
           ('M3+AMSsub p16', M3 + ',AMS-Regular', 16, 16, 'amssub'), ('R(+Size2-4) p16', R, 16, 16, 'amssub'),
           ('R p16 +accents +rules', R, 16, 16, 'amssub,overlays,rules'), ('R p64 (closure 16)', R, 64, 16, 'amssub,overlays,rules'),
           ('M4 base only', M4, 0, 0, ''), ('M4 p8 (closure 2)', M4, 8, 2, ''), ('M4 p8 all closure', M4, 8, 8, ''), ('M4 p32 (closure 2)', M4, 32, 2, ''),
           ('M7 p8 (closure 2)', M7, 8, 2, ''), ('FULL base only', FULL, 0, 0, ''), ('FULL p8 (closure 2)', FULL, 8, 2, ''), ('FULL p64 (closure 4)', FULL, 64, 4, 'overlays,rules')]
rows = []
for name, fonts, n, cl, flags in configs:
    key = name.replace(' ', '_').replace('(', '').replace(')', '')
    spec_path = '%s/spec_%s.json' % (out, key)
    subprocess.run(['python3', '-I', '/home/agents/scratch/katex-font/tools/mfspec.py', '/home/agents/scratch/katex-font/data', spec_path, '--fonts', fonts, '--npairs', str(n), '--closure', str(cl), '--nspacers', '16'] + (['--ams-subset'] if 'amssub' in flags else []), check=True, stdout=subprocess.DEVNULL)
    sp = json.load(open(spec_path))
    if 'overlays' in flags: sp['overlays'] = json.load(open('/home/agents/scratch/katex-font/data/overlays.json'))
    if 'rules' in flags: sp['rules'] = [[0.04, 0.23]]
    json.dump(sp, open(spec_path, 'w'), separators=(',', ':'))
    b, ttf = G.build('/home/agents/scratch/katex-font/fonts', sp, '%s/%s' % (out, key), 'F')
    p = '%s/%s/F' % (out, key)
    sz = lambda ext: os.path.getsize(p + ext)
    z = subprocess.run(['zstd', '-19', '-c', p + '.ttf'], stdout=subprocess.PIPE).stdout
    br = G.brotli_cli(ttf)
    e = Encoder(p + '.map.json')
    tot = ok = 0; gl_ok = gl_tot = 0; okg = totg = 0
    for m in ('inline', 'display'):
        for o in data[m]:
            tot += 1
            donly = not o['r'] and not o['svg']
            r = e.best(o['g'], o['box'][0]) if True else None
            miss = len(r[1])
            gl_tot += len(o['g']); gl_ok += len(o['g']) - miss
            if donly:
                totg += 1; okg += (miss == 0)
            ok += (miss == 0 and donly)
    rows.append(dict(name=name, glyphs=len(b.ordered) + 1, variants=len(b.variants), ttf=len(ttf), woff=sz('.woff'), woff2u=sz('.untransformed.woff2'), woff2=sz('.woff2'), zstd=len(z), brotli=len(br), formulas_ok=ok, formulas=tot, glyph_cov=gl_ok / gl_tot, tables=b.sizes))
    r = rows[-1]
    print('%-24s glyphs %5d variants %5d ttf %7d woff %7d woff2u %7d woff2 %7d zstd19 %7d br11 %7d | font-only formulas %d/%d (%.1f%%) glyphs %.1f%%' % (name, r['glyphs'], r['variants'], r['ttf'], r['woff'], r['woff2u'], r['woff2'], r['zstd'], r['brotli'], ok, tot, 100 * ok / tot, 100 * r['glyph_cov']), flush=True)
json.dump(rows, open(out + '/sizes.json', 'w'), indent=1)
