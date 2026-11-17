# mfbench4.py OUTDIR : micro pages (3,000 paragraphs, one of the formulas each) and fixture pages (400 messages x 12 corpus formulas) for
# floor (the formula's text as plain text) / flat design (compact ALL) / old hybrid (the font before this stage: OLD) / new hybrid
# needs: OLD/{font_inline,font_display,compact_inline,compact_display}.jsonl and old.woff2/old.css, NEW/... likewise (see regen.sh)
import sys, os, json, re, html as H
out = sys.argv[1]; os.makedirs(out, exist_ok=True)
S = '/home/agents/scratch/katex-font/'
F = [json.loads(l) for l in open('/home/agents/scratch/katex-harden/data/formulas.jsonl')]
fonts = 'file:///home/agents/scratch/katex-compact/css/fonts/'
def css(path, woff): return open(path).read().replace('url(MathRun.woff2)', 'url(file://%s)' % woff).replace('url(fonts/', 'url(' + fonts)
cflat = css(S + 'rsout/flat.css', '')
cold = css(S + 'bench4/old.css', S + 'bench4/old.woff2'); cnew = css(S + 'bench4/new.css', S + 'bench4/new.woff2')
def text_of(h):
    h = re.sub(r'<[^>]*>', '', h)
    h = re.sub(r'[-​⁠]', '', h)
    return H.unescape(h)
settle = '<script>window.__ev={};addEventListener("load",()=>{window.__ev.load=performance.now();document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>{window.__ev.settled=performance.now();window.__settled=true})))})</script>'
base = 'body{margin:0;font:15px/1.5 system-ui,sans-serif}.msgs{width:760px;margin:0 auto}p{margin:.5em 0}'
M = json.load(open(S + 'bench4/micro.json'))
def rd(p): return [json.loads(l).get('html', '') for l in open(p)]
fl = rd(S + 'bench4/micro_flat.jsonl'); ol = rd(S + 'bench4/micro_old.jsonl'); nw = rd(S + 'bench4/micro_new.jsonl')
def page(name, c, body, n=3000):
    ps = ''.join('<p>Step %d: we have %s here.</p>' % (i, body) for i in range(n))
    open('%s/%s.html' % (out, name), 'w').write('<!doctype html><meta charset=utf-8><style>%s%s</style>%s<body><div class=msgs>%s</div>' % (base, c, settle, ps))
stats = []
for i, t in enumerate(M):
    page('m%d_floor' % i, '', H.escape(text_of(fl[i]) or 'x'))
    page('m%d_flat' % i, cflat, fl[i]); page('m%d_old' % i, cold, ol[i]); page('m%d_new' % i, cnew, nw[i])
    cnt = lambda h: len(re.findall(r'<(?:span|svg|wbr|path|line)', h))
    stats.append(dict(i=i, tex=t, flat=(len(fl[i].encode()), cnt(fl[i])), old=(len(ol[i].encode()), cnt(ol[i])), new=(len(nw[i].encode()), cnt(nw[i])),
                      chars_old=len(re.findall('[-]', ol[i])), chars_new=len(re.findall('[-]', nw[i]))))
json.dump(stats, open(out + '/micro_stats.json', 'w'))
def natural(path):
    A = {json.loads(l)['id']: json.loads(l) for l in open(path % 'inline')}
    B = {json.loads(l)['id']: json.loads(l) for l in open(path % 'display')}
    return {f['id']: (B if f['display'] else A)[f['id']] for f in F}
st = 'body{margin:0;font:15px/1.5 system-ui,sans-serif;background:#fff;color:#111}.msgs{width:760px;margin:0 auto}.md{word-break:break-word;overflow-wrap:anywhere}.md p{margin:.5em 0}.md .katex-display{overflow-x:auto;overflow-y:hidden;padding:4px 0}'
for name, c, path in (('flat', cflat, S + 'rsout/compact_%s.jsonl'), ('old', cold, S + 'old/font_%s.jsonl'), ('new', cnew, S + 'rsout/font_%s.jsonl')):
    nat = natural(path); k = 0; msgs = []
    for i in range(400):
        ps = []
        for j in range(12):
            f = F[k % len(F)]; k += 1
            h = nat[f['id']].get('html', '')
            ps.append('<p>Step %d.%d:</p><p>%s</p>' % (i, j, h) if f['display'] else '<p>Step %d.%d: we have %s here.</p>' % (i, j, h))
        msgs.append('<div class="md">' + ''.join(ps) + '</div>')
    open('%s/fx_%s.html' % (out, name), 'w').write('<!doctype html><meta charset=utf-8><style>%s%s</style>%s<body><div class=msgs>%s</div>' % (c, st, settle, ''.join(msgs)))
    if name == 'flat':
        msgs = []; k = 0
        for i in range(400):
            ps = []
            for j in range(12):
                f = F[k % len(F)]; k += 1
                t = H.escape(text_of(nat[f['id']].get('html', '')))
                ps.append('<p>Step %d.%d:</p><p>%s</p>' % (i, j, t) if f['display'] else '<p>Step %d.%d: we have %s here.</p>' % (i, j, t))
            msgs.append('<div class="md">' + ''.join(ps) + '</div>')
        open('%s/fx_floor.html' % out, 'w').write('<!doctype html><meta charset=utf-8><style>%s</style>%s<body><div class=msgs>%s</div>' % (st, settle, ''.join(msgs)))
print(len(os.listdir(out)), 'pages')
