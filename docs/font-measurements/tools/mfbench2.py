# mfbench2.py : controlled variants of the 3,000-paragraph page to find where a font formula's time goes
import sys, os, json
sys.path.insert(0, '/home/agents/scratch/katex-font/tools'); sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from mathfont_encode import Encoder, to_html
D = '/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/KaTeXMathRun'
enc = Encoder(D + '.map.json')
G = {o['id']: o for o in json.load(open('/home/agents/scratch/katex-font/data/glyphs_inline.json')) if 'g' in o}
F = [json.loads(l) for l in open('/home/agents/scratch/katex-harden/data/formulas.jsonl')]
def enc_tex(tex):
    for f in F:
        if f['tex'] == tex and f['id'] in G: o = G[f['id']]; return enc.best(o['g'], o['box'][0])[0]
# the micro formulas are not all in the corpus: build glyph lists by hand
def mk(tex_glyphs, width): return enc.best(tex_glyphs, width)[0]
adv = lambda f, c: enc.base[(f, c)][1] / enc.upem
a, b, c, d = [adv('Math-Italic', ord(x)) for x in 'abcd']
# a+b+c=d, hand-placed with 4mu around + and 5mu around =
th, med, thick = 3 / 18, 4 / 18, 5 / 18
pl = adv('Main-Regular', ord('+')); eq = adv('Main-Regular', ord('='))
x = 0; gl = []
def put(font, ch, size=1, y=0):
    global x
    gl.append([font, ch, size, x, y, 0, '']); x += adv(font, ch)
put('Math-Italic', ord('a')); x += med; put('Main-Regular', ord('+')); x += med
put('Math-Italic', ord('b')); x += med; put('Main-Regular', ord('+')); x += med
put('Math-Italic', ord('c')); x += thick; put('Main-Regular', ord('=')); x += thick
put('Math-Italic', ord('d'))
E_cps = enc.best(gl, x)[0]
print('E', len(E_cps), [hex(c) for c in E_cps])
# x_i^2 with negatives: x, then sup 2 at (0.7,0.363) and sub i at (0.7,-0.259) stacked
vx = lambda f, ch, s, y: enc.var[(f, ch, s, round(y, 3))]
xw = adv('Math-Italic', ord('x'))
gl2 = [['Math-Italic', ord('x'), 1, 0, 0, 0, ''], ['Main-Regular', ord('2'), 0.7, xw, 0.363, 0, ''], ['Math-Italic', ord('i'), 0.7, xw, -0.259, 0, '']]
X_cps = enc.best(gl2, xw + 0.4)[0]
print('X', len(X_cps), [hex(c) for c in X_cps])
S = lambda cps: ''.join(chr(c) for c in cps)
pua = [c for c in range(0xE010, 0xE010 + 40)]
puaglyphs = [enc.var[k][0] for k in list(enc.var)[:40]]
mathit = [enc.base[('Math-Italic', ch)][0] for ch in range(ord('a'), ord('a') + 26)]
fontface = '@font-face{font-family:F;src:url(file://%s.woff2) format("woff2")}' % D
blank = open(D + '.css').read().split('\n')[1]       # the blank-face @font-face rule
sub = lambda n: fontface.replace('F;', n + ';').replace('%s.woff2' % D, '%s.woff2' % ('/home/agents/scratch/katex-font/outg_%s/K%s' % (n, n)))
variants = {}
KM = '@font-face{font-family:KM;src:url(file:///home/agents/scratch/katex-compact/css/fonts/KaTeX_Main-Regular.woff2) format("woff2")}'
variants['a_plain_text_in_span'] = ('', '<span>a+b+c=d</span>')
variants['a0_plain_no_span'] = ('', 'a+b+c=d')
variants['b_span_webfont_KM'] = (KM + '.t{font-family:KM}', '<span class=t>a+b+c=d</span>')
variants['c_+geometricPrecision'] = (KM + '.t{font-family:KM;text-rendering:geometricPrecision}', '<span class=t>a+b+c=d</span>')
variants['d_+lh0_nowrap_ls'] = (KM + '.t{font:normal 1.21em/0 KM;text-rendering:geometricPrecision;white-space:nowrap;letter-spacing:normal;text-indent:0}', '<span class=t>a+b+c=d</span>')
mfcss = 'font:normal 1.21em/0 %s;white-space:nowrap;text-indent:0;text-rendering:geometricPrecision;letter-spacing:normal'
variants['e0_font_full_css(blank+strut)'] = (fontface.replace('font-family:F', 'font-family:KaTeXMathRun') + blank + '.mf{font:normal 1.21em/0 KaTeXMathRun,KaTeXMathRunBlank;white-space:nowrap;text-indent:0;text-rendering:geometricPrecision;letter-spacing:normal}.mf::before{content:"";display:inline-block;height:calc(var(--h,0em) + var(--d,0em));vertical-align:calc(-1*var(--d,0em))}', '<span class=mf style="--h:.88em;--d:.31em">%s</span>' % S(E_cps))
base_css = fontface + '.mf{%s}' % (mfcss % 'F')
variants['e1_font_no_blank_no_strut'] = (base_css, '<span class=mf>%s</span>' % S(E_cps))
variants['e2_font_blank_no_UR'] = (base_css.replace('font:normal 1.21em/0 F', 'font:normal 1.21em/0 F,B') + '@font-face{font-family:B;src:url(data:font/woff2;base64,%s) format("woff2")}' % blank.split('base64,')[1].split(')')[0], '<span class=mf>%s</span>' % S(E_cps))
variants['e3_font_blank_UR_no_strut'] = (fontface.replace('font-family:F', 'font-family:KaTeXMathRun') + blank + '.mf{font:normal 1.21em/0 KaTeXMathRun,KaTeXMathRunBlank;white-space:nowrap;text-indent:0;text-rendering:geometricPrecision;letter-spacing:normal}', '<span class=mf>%s</span>' % S(E_cps))
variants['e4_font_strut_only'] = (base_css + '.mf::before{content:"";display:inline-block;height:calc(var(--h,0em) + var(--d,0em));vertical-align:calc(-1*var(--d,0em))}', '<span class=mf style="--h:.88em;--d:.31em">%s</span>' % S(E_cps))
for n in ('g2', 'g1', 'g0'):
    css = sub(n) + '.mf{%s}' % (mfcss % n)
    variants['f_gpos_%s_negformula' % n] = (css, '<span class=mf>%s</span>' % S(X_cps))
variants['f_gpos_all5_negformula'] = (base_css, '<span class=mf>%s</span>' % S(X_cps))
variants['f_nonneg_formula'] = (base_css, '<span class=mf>%s</span>' % S(E_cps))
n13 = 13
variants['g_ascii13'] = (base_css, '<span class=mf>%s</span>' % 'a+b+c=d1234'[:n13])
variants['g_pua13'] = (base_css, '<span class=mf>%s</span>' % S(puaglyphs[:n13]))
variants['g_mathitalic13'] = (base_css, '<span class=mf>%s</span>' % S(mathit[:n13]))
variants['g_mixed_ascii_pua13'] = (base_css, '<span class=mf>%s</span>' % S([ord(ch) if i % 2 == 0 else puaglyphs[i] for i, ch in enumerate('a+b+c=d1234')][:n13]))
for n in (3, 13, 40):
    variants['h_pua%d' % n] = (base_css, '<span class=mf>%s</span>' % S(puaglyphs[:n]))
settle = '<script>window.__ev={};addEventListener("load",()=>{window.__ev.load=performance.now();document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>{window.__ev.settled=performance.now();window.__settled=true})))})</script>'
base = 'body{margin:0;font:15px/1.5 system-ui,sans-serif}.msgs{width:760px;margin:0 auto}p{margin:.5em 0}'
for name, (css, body) in variants.items():
    ps = ''.join('<p>Step %d: we have %s here.</p>' % (i, body) for i in range(3000))
    open('/home/agents/scratch/katex-font/bench2/%s.html' % name, 'w').write('<!doctype html><meta charset=utf-8><style>%s%s</style>%s<body><div class=msgs>%s</div>' % (base, css, settle, ps))
print(len(variants), 'pages')
