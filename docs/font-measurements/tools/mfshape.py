# mfshape.py FONTBASE OUTDIR : shaping, selection, copy, find and fallback behaviour of the generated font in headless Chrome
import sys, os, json
sys.path.insert(0, '/home/agents/scratch/katex-font/tools'); sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import Encoder, to_html
fb, out = sys.argv[1], sys.argv[2]; os.makedirs(out, exist_ok=True)
fam = 'F'
enc = Encoder(fb + '.map.json')
m = enc.m
cp = lambda font, ch: enc.base[(font, ch)][0]
adv = lambda font, ch: enc.base[(font, ch)][1] / enc.upem
a = chr(cp('Main-Regular', ord('a'))); b = chr(cp('Main-Regular', ord('b')))
alt = lambda ch: chr(enc.base[('Main-Regular', ord(ch))][4])      # private-use twin of a real letter
A, B = alt('a'), alt('b')
xi = chr(cp('Math-Italic', ord('x')))
neg = lambda units: ''.join(chr(c) for c in enc.spacer(-units, 0)[0])
pos = lambda units: ''.join(chr(c) for c in enc.spacer(units, 0)[0])
aw = round(adv('Main-Regular', ord('a')) * enc.upem)
css = '@font-face{font-family:F;src:url(file://%s.woff2) format("woff2")}.t{font:40px/60px F;white-space:nowrap}' % os.path.abspath(fb)
cases = {
    'real a,b (ab)': a + b,
    'real a,-adv(a),b (clamped)': a + neg(aw) + b,                 # the run clamp: expected 22 px width at 40 px, got 42
    'twins A,B (AB)': A + B,
    'twins A,-adv(A),B': A + neg(aw) + B,                          # B over A: width max(adv) = 22
    'twins A,-adv/2,-adv/2,B': A + neg(aw // 2) + neg(aw - aw // 2) + B,
    'twins A,+1em,B': A + pos(enc.upem) + B,
    'twins mix with Math-Italic and digit': A + xi + neg(100) + pos(300) + '1' + B,
}
html = '<!doctype html><meta charset=utf-8><style>%s</style>' % css
for k, v in cases.items(): html += '<p><span class=t data-k="%s">%s</span>' % (k, to_html([ord(c) for c in v]))
open(out + '/shape.html', 'w').write(html)
res = {}
with chrome(9591) as c:
    c.goto(out + '/shape.html')
    base = lambda: json.loads(c.ev('JSON.stringify([...document.querySelectorAll(".t")].map(e=>[e.dataset.k,e.getBoundingClientRect().width]))'))
    res['default'] = base()
    for name, rule in [('font-kerning:none', 'font-kerning:none'), ('text-rendering:optimizeSpeed', 'text-rendering:optimizeSpeed'), ('text-rendering:geometricPrecision', 'text-rendering:geometricPrecision'),
                       ('letter-spacing:1px', 'letter-spacing:1px'), ('font-variant-ligatures:none', 'font-variant-ligatures:none'), ('font-feature-settings:"kern" 0', 'font-feature-settings:"kern" 0'),
                       ('font-feature-settings:"kern" 0, "dist" 0', 'font-feature-settings:"kern" 0, "dist" 0'), ('word-spacing:10px', 'word-spacing:10px'), ('font-synthesis:none', 'font-synthesis:none'),
                       ('direction:rtl', 'direction:rtl'), ('writing-mode:vertical-rl', 'writing-mode:vertical-rl'), ('zoom:1.5 / transform scale', 'zoom:1.5')]:
        c.ev('document.querySelectorAll(".t").forEach(e=>e.style.cssText=%s)' % json.dumps(rule))
        res[name] = base()
    c.ev('document.querySelectorAll(".t").forEach(e=>e.style.cssText="")')
    # line breaking: narrow container, white-space normal, with and without an explicit ZWSP
    zw = a + b + a + b + a + b + a + b
    lb = ''.join(chr(cp('Main-Regular', ord(ch))) for ch in 'a+b-c/d,e;f!g?h') if True else ''
    c.ev('''(()=>{const d=document.createElement("div");d.id="lb";d.style.cssText="font:40px/60px F;width:60px;white-space:normal";document.body.appendChild(d);})()''')
    def lines(txt, extra=''):
        return c.ev('(()=>{const d=document.getElementById("lb");d.style.cssText="font:40px/60px F;width:60px;white-space:normal;%s";d.textContent=%s;return Math.round(d.getBoundingClientRect().height/60)})()' % (extra, json.dumps(txt)))
    res['lines in 60px box, no nowrap'] = {
        'PUA letters x8 (white-space:normal)': lines(zw), 'same with overflow-wrap:anywhere': lines(zw, 'overflow-wrap:anywhere'), 'same with word-break:break-all': lines(zw, 'word-break:break-all'),
        'same with white-space:nowrap': lines(zw, 'white-space:nowrap'),
        'ASCII punctuation a+b-c/d,e;f!g?h (normal)': lines('a+b-c/d,e;f!g?h'),
        'with U+200B between a/b pairs': lines((a + b + '​') * 4),
        'with U+200B and nowrap': lines((a + b + '​') * 4, 'white-space:nowrap'),
        'ZWSP between, overflow-wrap:anywhere not set, 2 chars width': lines(('​').join(list(zw))),
    }
    # selection, copy, find
    c.ev('document.body.innerHTML=%s' % json.dumps('<p style="font:20px/30px system-ui">Before <span class=t id=f style="font:24px/0 F;white-space:nowrap">%s</span> after, and <b id=z>zz</b> to find.</p><p style="font:20px/30px system-ui">ref: x_i^2+a</p>' % to_html([ord(ch) for ch in 'a' + neg(100) + pos(300) + xi + a + b])))
    res['copy text of "a ..spacers.. x a b"'] = c.ev('(()=>{const r=document.createRange();r.selectNodeContents(document.getElementById("f"));const s=getSelection();s.removeAllRanges();s.addRange(r);return [...s.toString()].map(ch=>"U+"+ch.codePointAt(0).toString(16).toUpperCase()).join(" ")})()')
    c.ws.call('Emulation.setDeviceMetricsOverride', width=500, height=120, deviceScaleFactor=2, mobile=False)
    c.shot(out + '/selection.png')
    res['window.find("b") finds a letter of the formula'] = c.ev('(()=>{getSelection().removeAllRanges();return window.find("zz")})()')
    res['window.find ASCII letter that is in the formula (a)'] = c.ev('(()=>{getSelection().removeAllRanges();window.find("a",false,false,true);const n=getSelection().anchorNode;return n&&n.parentElement?n.parentElement.id||n.parentElement.tagName:null})()')
    res['caret/double click word'] = 'see selection.png'
    print(json.dumps(res, indent=1))
    json.dump(res, open(out + '/shape.json', 'w'), indent=1)
# font fallback / failure
with chrome(9592) as c:
    f = ''.join(chr(cp('Main-Regular', ord(ch))) for ch in 'a+b') + xi + neg(100) + pos(300) + a
    for name, face in [('missing file, font-display:block', '@font-face{font-family:F;src:url(file:///nonexistent.woff2);font-display:block}'),
                       ('missing file, font-display:swap', '@font-face{font-family:F;src:url(file:///nonexistent.woff2);font-display:swap}'),
                       ('corrupt file (valid header, truncated)', '@font-face{font-family:F;src:url(file://%s.woff2.trunc);font-display:swap}' % os.path.abspath(fb))]:
        if 'trunc' in name: open(fb + '.woff2.trunc', 'wb').write(open(fb + '.woff2', 'rb').read()[:3000])
        open(out + '/fail.html', 'w').write('<!doctype html><meta charset=utf-8><style>%s body{font:30px/40px system-ui}.t{font:30px/40px F, serif;white-space:nowrap}</style><p>%s:<p><span class=t>%s</span> end' % (face, name, to_html([ord(ch) for ch in f])))
        c.ws.call('Emulation.setDeviceMetricsOverride', width=700, height=130, deviceScaleFactor=1, mobile=False)
        c.goto(out + '/fail.html', wait=False)
        c.ev('new Promise(r=>setTimeout(r,500))', await_=True)
        c.shot(out + '/fail_%s.png' % name.split(',')[1].strip().replace(':', '_').replace(' ', '_') if ',' in name else out + '/fail_corrupt.png')
        print(name, c.ev('JSON.stringify([...document.fonts].map(f=>f.status))'), [x[:120] for x in c.console()[:2]])
