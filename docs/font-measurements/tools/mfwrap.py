# mfwrap.py RS_JSONL OUTDIR WIDTH [VARIANT] : every corpus formula inline in a paragraph of 16 px text WIDTH px wide: KaTeX JS vs the Rust output (css rsout/font.css)
# prints: formulas that wrap in KaTeX JS, same line starts (the source glyph at the start of each line), paragraph height and text baseline differences
import sys, os, json, re, collections
sys.path.insert(0, '/home/agents/scratch/katex-font/tools'); sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
from cu import *
from mathfont_encode import Encoder
rs, out, W = sys.argv[1], sys.argv[2], int(sys.argv[3]); os.makedirs(out, exist_ok=True); KAUTO = 'ktauto' in sys.argv; FLAT = 'flat' in sys.argv
D = '/home/agents/scratch/katex-font/old/data/' if 'oldfont' in sys.argv else '/home/agents/workspaces/katex-rs-font/crates/katex/data/math-font/'
mapf = [D + f for f in os.listdir(D) if f.endswith('.map.json')][0]; fontf = mapf.replace('.map.json', '.woff2')
enc = Encoder(mapf); rev = {}
for (f, c), (cp, ad, ic, noic, alt) in enc.base.items():
    rev[cp] = c
    if noic: rev[noic] = c
for (f, c, s, y), (cp, ad) in enc.var.items(): rev[cp] = c
for b in json.load(open(mapf)).get('baked', []): rev[b[6]] = b[1]
F = {f['id']: f for f in map(json.loads, open('/home/agents/scratch/katex-harden/data/formulas.jsonl'))}
CUS = [a[7:] for a in sys.argv if a.startswith('custom=')]
if CUS:
    for k, t in enumerate(open(CUS[0])):
        if t.strip(): F[100000 + k] = {'id': 100000 + k, 'tex': t.rstrip('\n'), 'display': False}
R = {}
for l in open(rs):
    j = json.loads(l)
    if 'html' in j and not F[j['id']]['display'] or ('html' in j and True): R[j['id']] = j['html']
css = open('/home/agents/scratch/katex-font/rsout/' + ('flat.css' if FLAT else ('font_old.css' if 'oldfont' in sys.argv else 'font.css'))).read().replace('url(MathRun.woff2)', 'url(file://%s)' % os.path.abspath(fontf)).replace('url(fonts/', 'url(file:///home/agents/scratch/katex-compact/css/fonts/')
base = 'body{margin:0;font:16px/1.5 system-ui,sans-serif}.row{width:%dpx;padding:6px 20px;margin:0;border-bottom:1px solid #ccc}.bl{display:inline-block;width:0;height:0}' % W
if 'owa' in sys.argv: base += '.row{overflow-wrap:anywhere;word-break:break-word}'
JS0 = r'''window.SK=new Set(SPACERS);window.info=function(){const out=[];for(const row of document.querySelectorAll('.row')){const r0=row.getBoundingClientRect();const k=row.querySelector('.katex');const bl=row.querySelector('.bl').getBoundingClientRect();
const kr=k?k.getBoundingClientRect():null;const starts=[];if(k){const w=document.createTreeWalker(k,NodeFilter.SHOW_TEXT);let n;const rg=document.createRange();let prev=null;
while(n=w.nextNode()){const t=n.nodeValue;for(let i=0;i<t.length;i++){const cp=t.codePointAt(i);const len=cp>0xffff?2:1;if(cp===0x20||cp===0xa0||cp===0x200b||cp===0x2060||cp===0x202f||SK.has(cp)){i+=len-1;continue}
rg.setStart(n,i);rg.setEnd(n,i+len);const b=rg.getBoundingClientRect();i+=len-1;
if(prev&&b.left<prev.left+1&&b.bottom>prev.bottom+12)starts.push(cp);prev=b}}}
out.push({id:+row.dataset.id,h:r0.height,base:bl.bottom-r0.top,katex:kr?[kr.left-r0.left,kr.top-r0.top,kr.right-r0.left,kr.bottom-r0.top]:null,starts})}return JSON.stringify(out)}'''
spacers = sorted(set(enc.pos.values()) | set(enc.neg.values()) | {cp for n, cp in enc.named} | {cp for (f, c), (cp, ad, ic, noic, alt) in enc.base.items() if c in (0x20, 0xA0)})
JS = 'var SPACERS=%s;' % json.dumps(spacers) + JS0
ids = sorted(i for i in R if not F[i]['display'] or True)
kp = '<!doctype html><meta charset=utf-8><link rel=stylesheet href="file:///home/agents/scratch/katex-font/katex/katex.min.css"><style>%s%s</style><script>%s</script><div id=c></div><script src="file:///home/agents/scratch/katex-font/katex/katex.min.js"></script><script>const T=%s;const c=document.getElementById("c");for(const [l,t] of T){const p=document.createElement("p");p.className="row";p.dataset.id=l;p.innerHTML="Some words before, <i class=bl></i><span class=h></span> and some words after, so that this line goes on for a while.";p.querySelector(".h").innerHTML=katex.renderToString(t,{displayMode:false,output:"html",throwOnError:false,maxExpand:1000});c.appendChild(p)}</script>' % (base, '' if KAUTO else '.katex{text-rendering:geometricPrecision!important}', JS, json.dumps([[i, F[i]['tex']] for i in ids]))
fp = '<!doctype html><meta charset=utf-8><style>%s%s</style><script>%s</script><div id=c>%s</div>' % (base, css, JS, ''.join('<p class=row data-id=%d>Some words before, <i class=bl></i>%s and some words after, so that this line goes on for a while.</p>' % (i, R[i]) for i in ids))
open(out + '/k.html', 'w').write(kp); open(out + '/f.html', 'w').write(fp)
res = {}
with chrome(9651) as c:
    c.ws.call('Emulation.setDeviceMetricsOverride', width=W + 100, height=900, deviceScaleFactor=1, mobile=False)
    for kind in 'kf':
        c.goto('%s/%s.html' % (out, kind)); res[kind] = {o['id']: o for o in json.loads(c.ev('info()'))}
a, b = res['k'], res['f']
def srcs(starts): return [rev.get(cp, cp) if (0xE000 <= cp <= 0xF8FF) else cp for cp in starts]
wrapped = [i for i in a if a[i]['starts']]
same = [i for i in wrapped if srcs(a[i]['starts']) == srcs(b[i]['starts'])]
extra = [i for i in a if not a[i]['starts'] and b[i]['starts']]
dh = [abs(a[i]['h'] - b[i]['h']) for i in a]; db = [abs(a[i]['base'] - b[i]['base']) for i in a]
print('width %d: formulas that wrap in KaTeX JS: %d; same line starts: %d; different: %d %s; wrap only in hybrid: %d' % (W, len(wrapped), len(same), len(wrapped) - len(same), [i for i in wrapped if i not in same][:10], len(extra)))
same_lines = [i for i in a if abs(a[i]['h'] - b[i]['h']) < 12]
dh2 = [abs(a[i]['h'] - b[i]['h']) for i in same_lines]; db2 = [abs(a[i]['base'] - b[i]['base']) for i in same_lines]
print('paragraph height differs (>0.05 px): %d of %d (max %.2f); text baseline marker differs (>0.05): %d (max %.3f)' % (sum(1 for d in dh if d > .05), len(dh), max(dh), sum(1 for d in db if d > .05), max(db)))
print('  of the %d paragraphs with the same number of lines: height differs by >0.05 px in %d (max %.2f), by >1 px in %d; baseline of the text differs in %d (max %.3f); %d paragraphs have another number of lines' % (len(same_lines), sum(1 for d in dh2 if d > .05), max(dh2), sum(1 for d in dh2 if d > 1), sum(1 for d in db2 if d > .05), max(db2), len(a) - len(same_lines)))
json.dump({'k': a, 'f': b}, open(out + '/wrap.json', 'w'))
