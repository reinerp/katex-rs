#!/usr/bin/env python3
"""Generate src/generated/mathrun_table.rs, the glyph table of the font-run encoder (src/mathrun.rs), from the map that
gen_math_font.py writes next to the font (data/math-font/KaTeXMathRun-<version>.map.json).

    python3 -I crates/katex/tools/gen_mathrun_table.py

Not part of the build: run it by hand after generating the font, and commit the output. Python standard library only.
"""

import glob
import json
import os
import sys

CRATE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = os.path.join(CRATE, 'data', 'math-font')
OUT = os.path.join(CRATE, 'src', 'generated', 'mathrun_table.rs')
# the source fonts the encoder can resolve from KaTeX's classes; the index is the id used in the tables
FONTS = ['Main-Regular', 'Math-Italic', 'AMS-Regular', 'Size1-Regular', 'Size2-Regular', 'Size3-Regular', 'Size4-Regular']



METRIC_FONTS = ['Main-Regular', 'Math-Italic', 'AMS-Regular', 'Size1-Regular', 'Size2-Regular', 'Size3-Regular', 'Size4-Regular']


def round_te(x):
    """f64::round_ties_even (Python's round of a float is the same)."""
    return round(x)


def popcount_rank(mask, g):
    return bin(mask & ((1 << g) - 1)).count('1')


def implicit_table(m, stem, version, upem):
    """The table for a font with the implicit layout (gen_math_font.py, layout "implicit"): a few dozen numbers and the repertoires of the script
    variants as bit masks. Every code point and advance of the map is derived here by the rule the Rust side uses (mathrun.rs), and the script aborts
    if one differs."""
    met = json.load(open(os.path.join(CRATE, 'data', 'font_metrics_data.json')))
    layout = m['layout']
    u = lambda em: round(em * upem)
    # the glyph index g of every base glyph: the base planes are consecutive, in the order of FONTS
    glyphs = [(f, int(ch)) for f in FONTS for ch in sorted(int(k) for k in m['glyphs'].get(f, {}))]
    gi = {k: i for i, k in enumerate(glyphs)}
    count = [len(m['glyphs'].get(f, {})) for f in FONTS]
    offset = [sum(count[:i]) for i in range(len(FONTS))]
    base_start = layout['base:' + FONTS[0]]
    for i, f in enumerate(FONTS):
        if layout['base:' + f] != base_start + offset[i]:
            sys.exit('base planes are not consecutive at %s' % f)
    ams = [ch for f, ch in glyphs if f == 'AMS-Regular']
    ams_i = FONTS.index('AMS-Regular')
    for i, f in enumerate(FONTS):
        if i != ams_i and [ch for g, ch in glyphs if g == f] != sorted(int(k) for k in met[f]):
            sys.exit('the glyphs of %s are not the keys of its metrics table' % f)
    if not set(ams) <= set(int(k) for k in met['AMS-Regular']): sys.exit('AMS glyphs without metrics')
    # base: code point, advance (correction to the rule), noic
    corr = []
    rank_of = {}
    for i, f in enumerate(FONTS):
        chars = ams if i == ams_i else sorted(int(k) for k in met[f])
        for r, ch in enumerate(chars): rank_of[(f, ch)] = r
    ic_font = 'Math-Italic'
    noic_start = layout['noic']
    def base_rule(f, ch):
        r = rank_of[(f, ch)]; mm = met[f][str(ch)]
        ic = round_te(mm[2] * upem) if (f == ic_font and mm[2] > 0) else 0
        return r, ic, round_te(mm[4] * upem) + ic
    for (f, ch) in glyphs:
        e = m['glyphs'][f][str(ch)]
        r, ic, adv0 = base_rule(f, ch)
        g = gi[(f, ch)]
        if e['cp'] != base_start + g: sys.exit('base code point of %s:%d is not plane + rank' % (f, ch))
        if u(e.get('ic', 0)) != ic: sys.exit('italic correction of %s:%d differs' % (f, ch))
        if ('noic' in e) != (ic > 0) or (ic > 0 and e['noic'] != noic_start + r): sys.exit('noic of %s:%d' % (f, ch))
        corr.append(u(e['adv']) - adv0)
    base_adv = {k: u(m['glyphs'][k[0]][str(k[1])]['adv']) for k in glyphs}
    # script variants: pairs sorted by (size, shift); each has a plane (code point of the first) and the repertoire as a mask over g
    words = (len(glyphs) + 63) // 64
    pairs = []
    var_by = {}
    for v in m['variants']:
        size = round(v['size'] * 1000); shift = round(v['shift'] * 1000)
        mask = 0
        for k in v['glyphs']:
            f, ch = k.rsplit(':', 1); mask |= 1 << gi[(f, int(ch))]
        var_by[(size, shift)] = (v, mask)
    baked = [b for b in m.get('baked', [])]
    plane_of = {}
    for name, start in layout.items(): plane_of[name] = start
    # the planes of the script variants are named script:SIZE:SHIFT in the map's layout, with the floats of the spec
    vplane = {}
    for name, start in layout.items():
        if name.startswith('script:'):
            _, sz, sh = name.split(':'); vplane[(round(float(sz) * 1000), round(float(sh) * 1000))] = start
    sbplane = {}
    for name, start in layout.items():
        if name.startswith('baked:ss:'):
            _, _, sz, sh = name.split(':'); sbplane[(round(float(sz) * 1000), round(float(sh) * 1000))] = start
    gaps = {tuple(b[4:6]) for b in baked if b[2] != 1.0 and b[4] == 0}
    if len(gaps) > 1: sys.exit('several script-space gaps: %r' % gaps)
    script_gap = gaps.pop()[1] if gaps else 0
    table = []
    for (size, shift) in sorted(var_by):
        v, mask = var_by[(size, shift)]
        start = vplane[(size, shift)]
        sf = size / 1000
        for k, (cp, adv) in v['glyphs'].items():
            f, ch = k.rsplit(':', 1); g = gi[(f, int(ch))]
            if cp != start + popcount_rank(mask, g): sys.exit('variant code point of %s at %s' % (k, (size, shift)))
            if u(adv) != round_te(base_adv[(f, int(ch))] * sf): sys.exit('variant advance of %s at %s: %s vs %s' % (k, (size, shift), u(adv), round_te(base_adv[(f, int(ch))] * sf)))
        table.append((size, shift, start, sbplane.get((size, shift), 0), mask))
    # glyphs with spacing: classes (bin, rel, punct) as masks and patterns
    patterns = {}
    for b in baked:
        f, ch, size, shift, left, right, cp, adv = b
        if size == 1.0:
            if shift != 0.0: sys.exit('baked glyph at size 1 with a shift')
            patterns.setdefault((left, right), []).append((gi[(f, ch)], cp))
        else:
            sz, sh = round(size * 1000), round(shift * 1000)
            if left != 0 or right != script_gap: sys.exit('script-space gaps differ')
            if sz == 1000: sys.exit('script space at size 1')
            v, mask = var_by[(sz, sh)]
            if cp != sbplane[(sz, sh)] + popcount_rank(mask, gi[(f, ch)]): sys.exit('script-space code point of %s:%d' % (f, ch))
    pat_rows = []
    for (left, right), lst in sorted(patterns.items()):
        gs = sorted(lst)
        mask = 0
        for g, cp in gs: mask |= 1 << g
        start = gs[0][1]
        for g, cp in gs:
            if cp != start + popcount_rank(mask, g): sys.exit('spacing code point (%d,%d) glyph %d' % (left, right, g))
        pat_rows.append((left, right, start, mask))
    # the spacers
    sp = m['spacers']
    named = sorted((u(a), cp) for cp, a in sp['named'].values())
    neg_named = [t for t in named if t[0] < 0]; pos_named = [t for t in named if t[0] > 0]
    if len(neg_named) + len(pos_named) != len(named): sys.exit('a named spacer of advance 0')
    neg_start = neg_named[0][1] if neg_named else sp['neg']['1'] - 0
    pos_start = pos_named[0][1] if pos_named else sp['pos']['1']
    for i, (a, cp) in enumerate(neg_named):
        if cp != neg_start + i: sys.exit('negative named spacers are not consecutive')
    for i, (a, cp) in enumerate(pos_named):
        if cp != pos_start + i: sys.exit('positive named spacers are not consecutive')
    pos_bin = pos_start + len(pos_named); neg_bin = neg_start + len(neg_named)
    for k in range(15):
        if sp['pos'][str(1 << k)] != pos_bin + k or sp['neg'][str(1 << k)] != neg_bin + k: sys.exit('binary spacers are not consecutive')
    mw = lambda mask: '[%s]' % ', '.join('0x%x' % ((mask >> (64 * i)) & ((1 << 64) - 1)) for i in range(words))
    out = ['// Generated by tools/gen_mathrun_table.py from data/math-font/%s.map.json. Do not edit.' % stem,
           '// The font has the implicit layout (tools/gen_math_font.py, layout "implicit"): every code point and advance below is derived by the rule of src/mathrun.rs, and',
           '// the generator checked all %d base glyphs, %d script variants, %d glyphs with spacing and the spacers of the map against it.\n' % (len(glyphs), sum(len(v['glyphs']) for v, _ in var_by.values()), len(baked)),
           '/// The version of the generated font (a hash of its code points and advances): part of its file and family name.',
           'pub const FONT_VERSION: &str = "%s";' % version,
           '/// The `font-family` name of the font, and the stem of its file name.',
           'pub const FONT_FAMILY: &str = "%s";' % stem,
           '/// The file name of the font (the family name and `.woff2`).',
           'pub const FONT_FILE_NAME: &str = "%s.woff2";' % stem,
           '/// The font, as WOFF 2.',
           'pub static FONT_WOFF2: &[u8] = include_bytes!("../../data/math-font/%s.woff2");' % stem,
           '/// A font without outlines for the private-use code points, an optional fallback (see the module documentation).',
           'pub static BLANK_FONT_WOFF2: &[u8] = include_bytes!("../../data/math-font/KaTeXMathBlank.woff2");',
           '/// The map of the font (for the test that derives the whole font by the layout rule).',
           '#[cfg(test)]',
           'pub static MAP_JSON: &str = include_str!("../../data/math-font/%s.map.json");' % stem,
           '/// Font units per em.',
           'pub const UPEM: i64 = %d;' % upem,
           '/// Ascent and descent of the font, in em.',
           'pub const ASCENT: f64 = %s;' % repr(float(m['ascent'])),
           '/// (see above)',
           'pub const DESCENT: f64 = %s;' % repr(float(m['descent'])),
           '',
           '/// The source fonts by the id the encoder uses: the font whose metrics table ranks the glyphs of its plane.',
           'pub static FONT_FAMILIES: [&str; %d] = [%s];' % (len(FONTS), ', '.join('"%s"' % f for f in FONTS)),
           '/// The glyph index g of the first glyph of each font: the sum of the counts of the fonts before it. The base glyph of g is at `BASE_START + g`.',
           'pub static FONT_OFFSET: [u16; %d] = [%s];' % (len(FONTS), ', '.join(map(str, offset))),
           '/// The code point of the glyph g = 0 (the first of the first font); the base planes are consecutive.',
           'pub const BASE_START: u32 = %d;' % base_start,
           '/// The characters of AMS-Regular that the font has (the others of its metrics table are not in it), sorted: the rank in this list is the glyph\'s rank.',
           'pub static AMS_CHARS: [u16; %d] = [%s];' % (len(ams), ', '.join(map(str, ams))),
           '/// The glyph without the italic correction of the Math-Italic glyph of rank r (where it has one) is at `NOIC_START + r`.',
           'pub const NOIC_START: u32 = %d;' % noic_start,
           '/// By g: the advance of the base glyph minus (the width of its metrics times the units per em, rounded, plus the italic correction in units).',
           'pub static ADV_CORR: [i16; %d] = [%s];' % (len(corr), ', '.join(map(str, corr))),
           '',
           '/// The script variants, by (size in thousandths, shift in thousandths of an em): the code point of the first glyph of the pair\'s plane, the code point of the first glyph of',
           '/// its script-space plane (0: none). A glyph with a variant at the pair is at the start plus its rank in the pair\'s repertoire (`PAIR_MASKS`: bit g is set for it).',
           'pub static PAIRS: [(u16, i16, u32, u32); %d] = [%s];' % (len(table), ', '.join('(%d, %d, %d, %d)' % t[:4] for t in table)),
           '/// The repertoires of `PAIRS`, as bit masks over g.',
           'pub static PAIR_MASKS: [[u64; %d]; %d] = [%s];' % (words, len(table), ', '.join(mw(t[4]) for t in table)),
           '/// The gap after a glyph that carries the script space, in font units.',
           'pub const SCRIPT_GAP: i16 = %d;' % script_gap,
           '/// The glyphs that carry TeX\'s spacing (size 1, no shift): (gap before, gap after in font units, code point of the first glyph, mask over g), sorted by the gaps. A glyph is at',
           '/// the start plus its rank in the mask.',
           'pub static SPACING: [(i16, i16, u32, [u64; %d]); %d] = [%s];' % (words, len(pat_rows), ', '.join('(%d, %d, %d, %s)' % (l, r, s_, mw(mk)) for l, r, s_, mk in pat_rows)),
           '',
           '/// Spacers with their own code point: the advances in font units, sorted: the negative ones first, at `NEG_NAMED_START`, the others at `POS_NAMED_START`, each in this order.',
           'pub static NAMED: [i32; %d] = [%s];' % (len(named), ', '.join(str(a) for a, _ in named)),
           '/// How many of `NAMED` are negative (they come first).',
           'pub const NEG_NAMED: usize = %d;' % len(neg_named),
           '/// The code point of the first negative named spacer.',
           'pub const NEG_NAMED_START: u32 = %d;' % neg_start,
           '/// The code point of the first positive named spacer.',
           'pub const POS_NAMED_START: u32 = %d;' % pos_start,
           '/// Spacers of +2^k and -2^k font units, k = 0..15, at these plus k.',
           'pub const POS_BIN_START: u32 = %d;' % pos_bin,
           '/// (see above)',
           'pub const NEG_BIN_START: u32 = %d;' % neg_bin, '']
    return out


def main():
    maps = sorted(glob.glob(os.path.join(DATA, 'KaTeXMathRun-*.map.json')))
    if len(maps) != 1:
        sys.exit('expected exactly one KaTeXMathRun-<version>.map.json in %s, found %d' % (DATA, len(maps)))
    path = maps[0]
    stem = os.path.basename(path)[:-len('.map.json')]
    version = stem.split('-', 1)[1]
    if not os.path.exists(os.path.join(DATA, stem + '.woff2')):
        sys.exit('missing %s.woff2' % stem)
    m = json.load(open(path))
    upem = m['unitsPerEm']
    u = lambda em: round(em * upem)
    for f in m['glyphs']:
        if f not in FONTS:
            sys.exit('the map has glyphs of %s, which mathrun.rs cannot resolve' % f)
    if m.get('layout'):
        out = implicit_table(m, stem, version, upem)
        open(OUT, 'w').write('\n'.join(out))
        print('wrote', OUT, '(implicit layout) version', version)
        return
    base = []
    for f, d in m['glyphs'].items():
        for ch, e in d.items():
            base.append((FONTS.index(f), int(ch), e['cp'], u(e['adv']), u(e.get('ic', 0)), e.get('noic', 0), e.get('alt', 0)))
    base.sort()
    var = []
    for v in m['variants']:
        size = round(v['size'] * 1000)
        shift = round(v['shift'] * 1000)
        for k, (cp, adv) in v['glyphs'].items():
            f, ch = k.rsplit(':', 1)
            var.append((FONTS.index(f), int(ch), size, shift, cp, u(adv)))
    var.sort()
    baked = sorted((FONTS.index(f), int(ch), round(size * 1000), round(shift * 1000), left, right, cp)
                   for f, ch, size, shift, left, right, cp, adv in m.get('baked', []))
    sp = m['spacers']
    named = sorted((u(a), cp) for cp, a in sp['named'].values())
    pos = [sp['pos'][str(1 << k)] for k in range(15)]
    neg = [sp['neg'][str(1 << k)] for k in range(15)]
    out = ['// Generated by tools/gen_mathrun_table.py from data/math-font/%s.map.json. Do not edit.\n' % stem,
           '/// The version of the generated font (a hash of its code points and advances): part of its file and family name.',
           'pub const FONT_VERSION: &str = "%s";' % version,
           '/// The `font-family` name of the font, and the stem of its file name.',
           'pub const FONT_FAMILY: &str = "%s";' % stem,
           '/// The file name of the font (the family name and `.woff2`).',
           'pub const FONT_FILE_NAME: &str = "%s.woff2";' % stem,
           '/// The font, as WOFF 2.',
           'pub static FONT_WOFF2: &[u8] = include_bytes!("../../data/math-font/%s.woff2");' % stem,
           '/// A font without outlines for the private-use code points, an optional fallback (see the module documentation).',
           'pub static BLANK_FONT_WOFF2: &[u8] = include_bytes!("../../data/math-font/KaTeXMathBlank.woff2");',
           '/// Font units per em.',
           'pub const UPEM: i64 = %d;' % upem,
           '/// Ascent and descent of the font, in em.',
           'pub const ASCENT: f64 = %s;' % repr(float(m['ascent'])),
           '/// (see above)',
           'pub const DESCENT: f64 = %s;' % repr(float(m['descent'])),
           '',
           '/// (font, character, code point, advance with the italic correction, the italic correction in it, code point without it, private-use twin), sorted.',
           'pub static BASE: &[(u8, u32, u32, i32, i32, u32, u32)] = &[']
    out += ['    (%d, %d, %d, %d, %d, %d, %d),' % b for b in base]
    out += ['];', '',
            '/// (font, character, size in thousandths, shift in thousandths of an em, code point, advance), sorted.',
            'pub static VARIANTS: &[(u8, u32, u16, i16, u32, i32)] = &[']
    out += ['    (%d, %d, %d, %d, %d, %d),' % v for v in var]
    out += ['];', '',
            '/// Glyphs that carry a gap before and after them (TeX\'s spacing, the script space): (font, character, size in thousandths, shift in thousandths of an em,',
            '/// gap before and gap after in font units, code point), sorted. The glyph is the one at that size and shift moved right by the gap before',
            '/// and with the advance of the glyph plus both gaps.',
            'pub static BAKED: &[(u8, u32, u16, i16, i16, i16, u32)] = &[']
    out += ['    (%d, %d, %d, %d, %d, %d, %d),' % b for b in baked]
    out += ['];', '', '/// Spacers with their own code point: (advance in font units, code point), sorted by advance.',
            'pub static NAMED: &[(i32, u32)] = &[']
    out += ['    (%d, %d),' % n for n in named]
    out += ['];', '', '/// Spacers of +2^k and -2^k font units, k = 0..15.',
            'pub static POS: [u32; 15] = [%s];' % ', '.join(map(str, pos)),
            '/// (see above)',
            'pub static NEG: [u32; 15] = [%s];' % ', '.join(map(str, neg)), '']
    open(OUT, 'w').write('\n'.join(out))
    print('wrote', OUT, len(base), 'base glyphs,', len(var), 'variants,', len(named), 'named spacers; version', version)


if __name__ == '__main__':
    main()
