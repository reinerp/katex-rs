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
          'min-width': 'm', 'width': 'w', 'border-bottom-width': 'x', 'position': 'p'}


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
        out.append(minify(','.join(sels + extra) + '{' + body + '}'))
    for prop, vals in values():
        p = PREFIX[prop]
        for v in vals:
            out.append('.katex .%s%s{%s:%s}' % (p, encode(v), prop, v))
    css_out = '\n'.join(out) + '\n'
    open(a[1], 'w').write(css_out)
    print('wrote', a[1], len(css_out), 'bytes')


main()
