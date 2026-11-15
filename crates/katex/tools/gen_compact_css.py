#!/usr/bin/env python3
"""Generate the stylesheet for the compact output (src/compact.rs) from KaTeX 0.18.5's katex.css.

usage: gen_compact_css.py KATEX_CSS OUT_CSS [--fonts PREFIX]

The result is a superset of katex.css: every rule is kept, and each selector that names a class
that the compact output renames gets the short name as an alias. The compact-only rules and the
classes that stand for common inline declarations (the VALUES table in src/compact.rs) are
appended. Standard library only.
"""
import os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, '..', 'src', 'compact.rs')


def rust_table(name):
    s = open(SRC).read()
    m = re.search(r'// %s-BEGIN\n(.*?)// %s-END' % (name, name), s, re.S)
    return m.group(1)


def short_names():
    return dict(re.findall(r'\("([^"]+)",\s*"([^"]+)"\)', rust_table('SHORT')))


def values():
    out = []
    for prop, vals in re.findall(r'\(\s*"([a-z-]+)",\s*&\[(.*?)\],?\s*\)', rust_table('VALUES'), re.S):
        out.append((prop, re.findall(r'"([^"]+)"', vals)))
    return out


def table(name):
    return re.findall(r'\("([^"]*)",\s*"([^"]*)"\)', rust_table(name))


def b36(n):
    digits = '0123456789abcdefghijklmnopqrstuvwxyz'
    out = ''
    while True:
        out = digits[n % 36] + out
        n //= 36
        if n == 0:
            return out


def minify(css):
    css = re.sub(r'/\*.*?\*/', '', css, flags=re.S)
    css = re.sub(r'\s+', ' ', css)
    css = re.sub(r'\s*([{};,>])\s*', r'\1', css)
    css = re.sub(r':\s+', ':', css)
    css = css.replace(';}', '}')
    return css.strip()


def encode(v):
    """Class-name suffix of an em value: -0.25em -> n_25, 2.55em -> 2_55, 0 -> 0."""
    n = v[:-2] if v.endswith('em') else v
    neg = n.startswith('-')
    n = n.lstrip('-')
    if n.startswith('0.'):
        n = n[1:]
    return ('n' if neg else '') + n.replace('.', '_')


PREFIX = {'height': 'h', 'vertical-align': 'a', 'top': 'y', 'margin-left': 'l', 'margin-right': 'r',
          'min-width': 'm', 'width': 'w', 'border-bottom-width': 'x', 'padding-top': 'q'}


def main():
    a = sys.argv[1:]
    fonts = 'fonts/'
    if '--fonts' in a:
        i = a.index('--fonts'); fonts = a[i + 1]; del a[i:i + 2]
    css = open(a[0]).read()
    css = re.sub(r'/\*.*?\*/', '', css, flags=re.S)
    short = short_names()
    out = []
    # font faces: woff2 only
    for face in re.findall(r'@font-face\s*\{.*?\}', css, flags=re.S):
        fam = re.search(r'font-family:\s*"([^"]+)"', face).group(1)
        weight = re.search(r'font-weight:\s*(\w+)', face)
        style = re.search(r'font-style:\s*(\w+)', face)
        file_ = re.search(r'url\(fonts/([^)]+\.woff2)\)', face).group(1)
        out.append('@font-face{font-family:%s;src:url(%s%s)format("woff2");%s%sfont-display:block}' % (
            fam, fonts, file_, 'font-weight:%s;' % weight.group(1) if weight else '',
            'font-style:%s;' % style.group(1) if style else ''))
    rest = re.sub(r'@font-face\s*\{.*?\}', '', css, flags=re.S)
    for sel, body in re.findall(r'([^{}]+)\{([^{}]*)\}', rest):
        sels = [s.strip() for s in sel.split(',')]
        extra = []
        for s in sels:
            t = s
            # katex-sizing + reset-sizeA + sizeB (or fontsize-ensurer) -> FAB
            m = re.search(r'\.katex-sizing\.reset-size(\d+)\.size(\d+)', s)
            if m:
                d = lambda n: '0123456789ab'[int(n)]
                t = s[:m.start()] + '.F' + d(m.group(1)) + d(m.group(2)) + s[m.end():]
            for old, new in short.items():
                t = re.sub(r'\.%s(?![\w-])' % re.escape(old), '.' + new, t)
            if t != s:
                extra.append(t)
            # The vertical list is one inline block, X, in the compact output: the rules that
            # look at its table or its cell (but not the table's own) apply to it.
            if s not in ('.katex .vlist-t', '.katex .vlist-t2', '.katex .vlist', '.katex .vlist-r', '.katex .vlist-s'):
                x = re.sub(r'\.vlist(-t)?(?![\w-])', '.X', s)
                if x != s:
                    extra.append(x)
                    if '.vlist-t' in s:
                        extra.append(x.replace('.X', '.Y'))
        out.append(minify(','.join(sels + extra) + '{' + body + '}'))
    out.append('.katex .X,.katex .Y{display:inline-block;contain:layout;position:relative}')
    out.append('.katex .Y>span{display:inline-block}')
    out.append('.katex .Y::before{content:"";display:inline-block;overflow:hidden;width:0}')
    out.append('.katex .W::before{content:"";display:inline-block;overflow:hidden;width:0}')
    for i, (h, v) in enumerate(table('STRUTS')):
        out.append('.katex .u%s::before{content:"";display:inline-block;height:%s;%s}' % (b36(i), h, 'vertical-align:%s' % v if v else ''))
    for i, (h, v) in enumerate(table('PSTRUTS')):
        out.append('.katex .k%s::before{height:%s}' % (b36(i), h))
    for name, letter, props in (('GEOMS', 'g', ('padding-top', 'height', 'vertical-align')), ('ITEMS', 'i', ('top', 'margin-left', 'margin-right'))):
        for i, row in enumerate(re.findall(r'\[((?:"[^"]*",?\s*){3})\]', rust_table(name))):
            vals = re.findall(r'"([^"]*)"', row)
            decl = ';'.join('%s:%s' % (p, v) for p, v in zip(props, vals) if v)
            out.append('.katex .%s%s{%s}' % (letter, b36(i), decl))
    for prop, vals in values():
        p = PREFIX[prop]
        for i, v in enumerate(vals):
            out.append('.katex .%s%s{%s:%s}' % (p, b36(i), prop, v))
    css_out = '/* Compact output rules and KaTeX 0.18.5 katex.css (MIT, https://katex.org); generated by tools/gen_compact_css.py */\n' + '\n'.join(out) + '\n'
    open(a[1], 'w').write(css_out)
    print('wrote', a[1], len(css_out), 'bytes')


main()
