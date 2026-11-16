# mfextract.py OUTDIR : render the corpus with KaTeX JS 0.18.5 (inline and natural display mode) and extract per-glyph geometry
# (em of the formula font size): ch, font, size ratio, x, baseline height, advance; plus rules, svgs and class hints.
import sys, os, json
sys.path.insert(0, '/home/agents/scratch/katex-font/tools')
from cu import *
out = sys.argv[1]
F = [json.loads(l) for l in open('/home/agents/scratch/katex-harden/data/formulas.jsonl')]
JS = r'''
window.FM = (name, w, st) => (w === 'bold' || +w >= 600 ? 'B' : '') + (st === 'italic' ? 'I' : '');
window.ascCache = {};
window.asc = function(cs) {
  const key = cs.fontFamily + cs.fontWeight + cs.fontStyle + cs.fontSize;
  if (key in ascCache) return ascCache[key];
  const d = document.getElementById('cal'); d.innerHTML = '<span><em style="font:inherit">H</em><i style="display:inline-block;width:0;height:0"></i></span>';
  const sp = d.firstChild; sp.style.fontFamily = cs.fontFamily; sp.style.fontWeight = cs.fontWeight; sp.style.fontStyle = cs.fontStyle; sp.style.fontSize = cs.fontSize;
  const r = document.createRange(); const tn = d.querySelector('em').firstChild; r.setStart(tn, 0); r.setEnd(tn, 1);
  const a = d.querySelector('i').getBoundingClientRect().bottom - r.getBoundingClientRect().top;
  return ascCache[key] = a;
};
window.fontName = function(cs) {
  const fam = cs.fontFamily.split(',')[0].replace(/["']/g, '').trim().replace('KaTeX_', '');
  const b = cs.fontWeight === 'bold' || +cs.fontWeight >= 600, it = cs.fontStyle === 'italic';
  if (fam === 'Main') return b && it ? 'Main-BoldItalic' : b ? 'Main-Bold' : it ? 'Main-Italic' : 'Main-Regular';
  if (fam === 'Math') return b ? 'Math-BoldItalic' : 'Math-Italic';
  if (fam === 'Caligraphic') return b ? 'Caligraphic-Bold' : 'Caligraphic-Regular';
  if (fam === 'Fraktur') return b ? 'Fraktur-Bold' : 'Fraktur-Regular';
  if (fam === 'SansSerif') return b ? 'SansSerif-Bold' : it ? 'SansSerif-Italic' : 'SansSerif-Regular';
  if (fam === 'AMS') return 'AMS-Regular';
  if (/^Size\d$/.test(fam)) return fam + '-Regular';
  return fam + '-Regular';
};
window.extract = function(tex, display) {
  const row = document.getElementById('row');
  row.innerHTML = 'x <i id=bl style="display:inline-block;width:0;height:0"></i><span id=host></span>';
  let html; try { html = katex.renderToString(tex, {displayMode: display, throwOnError: true, output: 'html', trust: false, maxExpand: 1000, maxSize: 100}); } catch (e) { return {err: String(e).slice(0, 100)}; }
  const host = document.getElementById('host'); host.innerHTML = html;
  const k = host.querySelector('.katex'); const blb = document.getElementById('bl').getBoundingClientRect().bottom;
  const kr = k.getBoundingClientRect(); const fs = parseFloat(getComputedStyle(k).fontSize);
  const glyphs = []; const w = document.createTreeWalker(k, NodeFilter.SHOW_TEXT); let n; const rg = document.createRange();
  while (n = w.nextNode()) {
    const el = n.parentElement; const cs = getComputedStyle(el); const px = parseFloat(cs.fontSize); const fn = fontName(cs); const a = asc(cs);
    let hint = '';
    for (let e = el; e && e !== k; e = e.parentElement) { if (e.classList.contains('accent-body')) { hint = 'accbody'; break; } if (e.classList.contains('accent') && !hint) hint = 'accbase'; if (e.classList.contains('delimsizing') || e.classList.contains('delimcenter') || e.classList.contains('op-symbol') || e.classList.contains('large-op')) hint = hint || 'big'; }
    const t = n.nodeValue;
    for (let i = 0; i < t.length; i++) {
      const cp = t.codePointAt(i); if (cp > 0xFFFF) i++;
      if (cp === 0x200b || cp === 0xa0 || cp === 0x20) continue;
      rg.setStart(n, i); rg.setEnd(n, i + (cp > 0xFFFF ? 2 : 1)); const b = rg.getBoundingClientRect();
      glyphs.push([fn, cp, +(px / fs).toFixed(4), +((b.left - kr.left) / fs).toFixed(5), +((blb - (b.top + a)) / fs).toFixed(5), +(b.width / fs).toFixed(5), hint]);
    }
  }
  const rules = []; for (const e of k.querySelectorAll('.frac-line,.overline-line,.underline-line,.hline,.rule,.sqrt-line,.stretchy,.brace-left,.brace-center,.brace-right,.tag,.boxpad,.fbox,.cancel-pad,.angl,.sout')) { const r = e.getBoundingClientRect(); rules.push([e.className.split(' ')[0], +((r.left - kr.left) / fs).toFixed(5), +(r.width / fs).toFixed(5), +((blb - r.bottom) / fs).toFixed(5), +(r.height / fs).toFixed(5)]); }
  const svgs = k.querySelectorAll('svg').length;
  const kb = [+((kr.right - kr.left) / fs).toFixed(5), +((blb - kr.top) / fs).toFixed(5), +((kr.bottom - blb) / fs).toFixed(5)];
  return {g: glyphs, r: rules, svg: svgs, box: kb, fs, els: k.getElementsByTagName('*').length + 1};
};
'''
page = out + '/ex.html'
open(page, 'w').write('<!doctype html><meta charset=utf-8><link rel=stylesheet href="file:///home/agents/scratch/katex-font/katex/katex.min.css"><style>body{margin:0;font:16px/1.5 system-ui,sans-serif;text-rendering:geometricPrecision}#row{white-space:nowrap;padding:10px 20px}.katex-display{display:inline-block!important;margin:0!important}.katex-display>.katex{display:inline-block!important;text-align:left!important}.katex{text-rendering:geometricPrecision!important}#cal{position:absolute;left:0;top:0;white-space:nowrap}</style><div id=row></div><div id=cal></div><script src="file:///home/agents/scratch/katex-font/katex/katex.min.js"></script><script>' + JS + '</script>')
res = {}
with chrome(9513, '--font-render-hinting=none') as c:
    c.goto(page)
    c.ev('''Promise.all([['KaTeX_Main','normal','normal'],['KaTeX_Main','bold','normal'],['KaTeX_Main','normal','italic'],['KaTeX_Main','bold','italic'],['KaTeX_Math','normal','italic'],['KaTeX_Math','bold','italic'],['KaTeX_AMS','normal','normal'],['KaTeX_Caligraphic','normal','normal'],['KaTeX_Caligraphic','bold','normal'],['KaTeX_Fraktur','normal','normal'],['KaTeX_Fraktur','bold','normal'],['KaTeX_SansSerif','normal','normal'],['KaTeX_SansSerif','bold','normal'],['KaTeX_SansSerif','normal','italic'],['KaTeX_Script','normal','normal'],['KaTeX_Size1','normal','normal'],['KaTeX_Size2','normal','normal'],['KaTeX_Size3','normal','normal'],['KaTeX_Size4','normal','normal'],['KaTeX_Typewriter','normal','normal']].map(([f,w,s])=>document.fonts.load(s+' '+w+' 20px '+f,'x'))).then(()=>document.fonts.ready)''', await_=True)
    for mode, key in ((False, 'inline'), (True, 'display')):
        outl = []
        for i in range(0, len(F), 100):
            part = [[f['id'], f['tex'], f['display']] for f in F[i:i + 100]]
            js = 'JSON.stringify(%s.map(([id,tex,disp])=>Object.assign({id,disp:+disp},extract(tex,%s ? true : false))))' % (json.dumps(part), 'disp' if mode else 'false')
            outl += json.loads(c.ev(js))
        json.dump(outl, open('%s/glyphs_%s.json' % (out, key), 'w'), separators=(',', ':'))
        print(key, len(outl), 'errors', sum(1 for o in outl if 'err' in o), 'glyphs', sum(len(o.get('g', [])) for o in outl))
    print(c.console()[:5])
