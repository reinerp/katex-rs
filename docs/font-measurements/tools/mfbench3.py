# mfbench3.py : micro pages (3,000 paragraphs, one of ten formulas each) and fixture pages (400 messages x 12 corpus formulas) for
# floor (the formula's text as plain text) / flat design (compact ALL) / hybrid (compact ALL + FONT_RUN)
import sys, os, json, re, html as H
out = '/home/agents/scratch/katex-font/bench3'
R = '/home/agents/scratch/katex-font/rsout/'
F = [json.loads(l) for l in open('/home/agents/scratch/katex-harden/data/formulas.jsonl')]
D = '/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/'
woff = [f for f in os.listdir(D) if f.endswith('.woff2') and 'Blank' not in f and 'untransformed' not in f][0]
fonts = 'file:///home/agents/scratch/katex-compact/css/fonts/'
cflat = open(R + "flat.css").read().replace("url(fonts/", "url(" + fonts)
chyb = open(R + 'font.css').read().replace('url(MathRun.woff2)', 'url(file://%s%s)' % (D, woff)).replace('url(fonts/', 'url(' + fonts)
open(out + '/flat.css', 'w').write(cflat); open(out + '/hyb.css', 'w').write(chyb)
def text_of(h):
    h = re.sub(r'<[^>]*>', '', h)      # attributes may hold '>' only in aria-label which is quoted; good enough for the floor
    h = re.sub(r'[-\U000f0000-\U000ffffd​]', '', h)
    return H.unescape(h)
settle = '<script>window.__ev={};addEventListener("load",()=>{window.__ev.load=performance.now();document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>{window.__ev.settled=performance.now();window.__settled=true})))})</script>'
base = 'body{margin:0;font:15px/1.5 system-ui,sans-serif}.msgs{width:760px;margin:0 auto}p{margin:.5em 0}'
T = json.load(open(R + 'micro.json'))
fl = [json.loads(l)['html'] for l in open(R + 'micro_compact.jsonl')]
hy = [json.loads(l)['html'] for l in open(R + 'micro_font.jsonl')]
def page(name, css, body, n=3000):
    ps = ''.join('<p>Step %d: we have %s here.</p>' % (i, body) for i in range(n))
    open('%s/%s.html' % (out, name), 'w').write('<!doctype html><meta charset=utf-8><style>%s%s</style>%s<body><div class=msgs>%s</div>' % (base, css, settle, ps))
for i, t in enumerate(T):
    # the floor is the characters that the flat output shows, as one text node
    page('m%d_floor' % i, '', H.escape(text_of(fl[i]) or 'x'))
    page('m%d_flat' % i, cflat, fl[i])
    page('m%d_hyb' % i, chyb, hy[i])
# fixture
def natural(kind):
    A = {json.loads(l)['id']: json.loads(l) for l in open(R + '%s_inline.jsonl' % kind)}
    B = {json.loads(l)['id']: json.loads(l) for l in open(R + '%s_display.jsonl' % kind)}
    return {f['id']: (B if f['display'] else A)[f['id']] for f in F}
for kind, name, css in (('compact', 'flat', cflat), ('font', 'hyb', chyb)):
    nat = natural(kind)
    rows = []
    k = 0
    msgs = []
    for i in range(400):
        ps = []
        for j in range(12):
            f = F[k % len(F)]; k += 1
            h = nat[f['id']].get('html', '')
            ps.append('<p>Step %d.%d:</p><p>%s</p>' % (i, j, h) if f['display'] else '<p>Step %d.%d: we have %s here.</p>' % (i, j, h))
        msgs.append('<div class="md">' + ''.join(ps) + '</div>')
    st = 'body{margin:0;font:15px/1.5 system-ui,sans-serif;background:#fff;color:#111}.msgs{width:760px;margin:0 auto}.md{word-break:break-word;overflow-wrap:anywhere}.md p{margin:.5em 0}.md .katex-display{overflow-x:auto;overflow-y:hidden;padding:4px 0}'
    open('%s/fx_%s.html' % (out, name), 'w').write('<!doctype html><meta charset=utf-8><style>%s%s</style>%s<body><div class=msgs>%s</div>' % (css, st, settle, ''.join(msgs)))
    if kind == 'compact':   # the floor: the same text, no formulas
        msgs = []; k = 0
        for i in range(400):
            ps = []
            for j in range(12):
                f = F[k % len(F)]; k += 1
                t = H.escape(text_of(nat[f['id']].get('html', '')))
                ps.append('<p>Step %d.%d:</p><p>%s</p>' % (i, j, t) if f['display'] else '<p>Step %d.%d: we have %s here.</p>' % (i, j, t))
            msgs.append('<div class="md">' + ''.join(ps) + '</div>')
        open('%s/fx_floor.html' % out, 'w').write('<!doctype html><meta charset=utf-8><style>%s</style>%s<body><div class=msgs>%s</div>' % (st, settle, ''.join(msgs)))
print(os.listdir(out)[:5], len(os.listdir(out)))
