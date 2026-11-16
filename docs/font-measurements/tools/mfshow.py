# mfshow.py OUTDIR : side-by-side page (KaTeX JS | hybrid output of the Rust encoder) of formulas that cover every construct kind, at device scale 1 and 2
import sys, os, json, re, subprocess
sys.path.insert(0, '/home/agents/scratch/katex-font/tools')
from cu import *
out = sys.argv[1]; os.makedirs(out, exist_ok=True)
F = {f['id']: f for f in map(json.loads, open('/home/agents/scratch/katex-harden/data/formulas.jsonl'))}
H = {}
for l in open('/home/agents/scratch/katex-font/rsout/font_inline.jsonl'):
    j = json.loads(l)
    if 'html' in j: H[j['id']] = j['html']
def pick(pred, n, short=True):
    r = []
    for i, h in H.items():
        t = F[i]['tex'].strip()
        if (not short or len(t) < 60) and pred(h, t) and i not in used: r.append(i); used.add(i)
        if len(r) >= n: break
    return r
used = set()
groups = [
    ('plain glyphs, spacing, scripts', pick(lambda h, t: h.count('<span') == 1 and '_' in t and '^' in t, 4) + pick(lambda h, t: h.count('<span') == 1 and '\\quad' in t or h.count('<span') == 1 and '\\,' in t, 2)),
    ('fractions', pick(lambda h, t: 'mfrac' in h and '\\frac' in t and '\\frac{' not in t.replace('\\frac{', '', 1), 7)),
    ('nested fractions / fractions with scripts', pick(lambda h, t: t.count('\\frac') >= 2, 6)),
    ('limits and big operators', pick(lambda h, t: '\\sum' in t or '\\prod' in t or '\\bigoplus' in t, 7)),
    ('matrices, arrays, aligned', pick(lambda h, t: 'mtable' in h, 7, short=False)),
    ('roots and SVG', pick(lambda h, t: '\\sqrt' in t and '<svg' in h, 5)),
    ('bold, calligraphic, typewriter, sans', pick(lambda h, t: any(k in t for k in ('\\mathbf', '\\mathcal', '\\mathtt', '\\mathsf', '\\mathbb')), 8)),
    ('text mode', pick(lambda h, t: '\\text' in t, 7)),
    ('delimiters, \\left \\right, \\big', pick(lambda h, t: '\\left' in t or '\\big' in t, 7)),
    ('accents, overlaps, boxed', pick(lambda h, t: any(k in t for k in ('\\hat', '\\bar', '\\vec', '\\widehat', '\\boxed', '\\ne ', '\\not', '\\cancel')), 6)),
    ('scripts with shifts the font lacks, scripts on stacks', pick(lambda h, t: 'class="F' in h and '_' in t and ('\\frac' in t or '\\binom' in t or '\\sum' in t), 5)),
]
custom = ['\\color{red}{x+1} + \\color{blue}{\\frac{a}{b}}', '{\\large a+b} + {\\small c} + {\\Huge x}', '\\mathbf{x}_i^2 + \\mathcal{L}(\\theta) + \\mathtt{code}', '\\frac{\\frac{a}{b}}{c} + \\left(\\frac{x^2}{y_i}\\right)^2', '\\sqrt[3]{x^2+y^2} + \\overrightarrow{AB} + \\hat{x}',
          '\\sum_{i=0}^{n} \\binom{n}{i} x_i^{2^k}', '\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}^{-1} = \\frac{1}{ad-bc}', 'a + \\frac{x^2}{y_i} = \\mathbf{b}', '\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}', '\\left\\{ x \\in \\mathbb{R} : \\lim_{n\\to\\infty} \\frac{1}{n}\\sum_{k=1}^n a_k = 0 \\right\\}']
cb = ''.join('%d 0 %d\n%s\n' % (100000 + i, len(t.encode()), t) for i, t in enumerate(custom))
open(out + '/custom.bin', 'w').write(cb)
if not os.path.exists(out + '/custom.jsonl'): subprocess.run(['/home/agents/scratch/katex-font/sxh', 'bash', '-c', '/home/agents/scratch/katex-font/target-ch/release/katex-font-harness render %s/custom.bin %s/custom.jsonl font 1 inline' % (os.path.abspath(out), os.path.abspath(out))], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
rows = []   # (label, tex, html)
for g, ids in groups:
    for i in ids: rows.append((g, F[i]['tex'].strip(), H[i], i))
for j, l in enumerate(open(out + '/custom.jsonl')): rows.append(('hand-written', custom[j], json.loads(l)['html'], 100000 + j))
long_ids = [i for i, h in H.items() if '<wbr>' in h and len(F[i]['tex']) > 80][:3]
for i in long_ids: rows.append(('long, wraps in 300 px', F[i]['tex'].strip(), H[i], i))
print(len(rows), 'rows')
css = open('/home/agents/scratch/katex-font/rsout/font.css').read()
D = '/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/'
mapf = [f for f in os.listdir(D) if f.endswith('.woff2') and 'Blank' not in f and 'untransformed' not in f][0]
css = css.replace('url(MathRun.woff2)', 'url(file://%s%s)' % (D, mapf)).replace('url(fonts/', 'url(file:///home/agents/scratch/katex-compact/css/fonts/')
PER = 14
for pi in range(0, len(rows), PER):
    part = rows[pi:pi + PER]
    trs = ''.join('<tr><td class=lb>%s<br><code>%s</code></td><td class=k data-i="%d"></td><td class=h>%s</td></tr>' % (g, t.replace('<', '&lt;').replace('\n', ' ')[:70], i, h) for g, t, h, i in part)
    kt = json.dumps({str(i): t for g, t, h, i in part})
    page = '<!doctype html><meta charset=utf-8><link rel=stylesheet href="file:///home/agents/scratch/katex-font/katex/katex.min.css"><style>body{margin:8px;font:15px/1.4 system-ui}table{border-collapse:collapse}td{border-bottom:1px solid #ddd;padding:6px 12px;vertical-align:middle}.lb{width:150px;font-size:11px;color:#666}.lb code{font-size:10px}.k,.h{width:520px;white-space:nowrap}.k .katex{text-rendering:geometricPrecision}%s</style><table>%s</table><script src="file:///home/agents/scratch/katex-font/katex/katex.min.js"></script><script>const T=%s;document.querySelectorAll(".k").forEach(e=>{e.innerHTML=katex.renderToString(T[e.dataset.i],{output:"html",throwOnError:false,maxExpand:1000})})</script>' % (css, trs, kt)
    open('%s/p%d.html' % (out, pi // PER), 'w').write(page)
with chrome(9701) as c:
    for dpr in (1, 2):
        c.ws.call('Emulation.setDeviceMetricsOverride', width=1280, height=1500, deviceScaleFactor=dpr, mobile=False)
        for pi in range(0, len(rows), PER):
            c.goto('%s/p%d.html' % (out, pi // PER)); h = int(c.ev('document.body.scrollHeight'))
            c.ws.call('Emulation.setDeviceMetricsOverride', width=1280, height=h + 16, deviceScaleFactor=dpr, mobile=False)
            c.shot('%s/p%d_dpr%d.png' % (out, pi // PER, dpr))
print('done')
