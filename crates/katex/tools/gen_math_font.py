#!/usr/bin/env python3
"""Compile KaTeX's TTF fonts into one generated math font (see docs/FONT.md; the Rust encoder is src/mathrun.rs).

    python3 -I crates/katex/tools/gen_math_font.py --fonts DIR_WITH_KaTeX_TTFS --spec SPEC.json --out OUTDIR [--name NAME]

SPEC.json (all keys optional):
  fonts        source fonts to include, e.g. ["Main-Regular", "Math-Italic", "AMS-Regular", "Size1-Regular"]
  subset       {font: [chars]} restricts a font's base glyphs to those characters
  ic_fonts     fonts whose glyphs get their italic correction added to the advance (default ["Math-Italic"])
  script_sizes sizes (of the base) of script variants, with "pairs": [[size, shift_em], ...]
  script_glyphs ["Font:char", ...] glyphs that get script variants (default: letters, digits, common symbols)
  spacers      advances in em of named spacer glyphs ({name: em})
  rules        [[thickness_em, shift_em], ...] families of rule glyphs
  ascent, descent  hhea/OS/2 vertical metrics in em
  overlays     [{"base": "Font:char", "over": "Font:char", "dx": em, "dy": em, "size": s}, ...] accent composites
  spacing      {"bin"|"rel"|"punct": [[left_em, right_em], ...]}: for every glyph of those classes (KaTeX's symbols table) a glyph with a gap
               before and after it
  script_space [[size, shift, gap_em], ...]: for the glyphs of those script variants a glyph with the script space after it
  pua_only     every glyph at a private-use code point (what the Rust encoder needs; default false)
  alt_letters  private-use twins of the real letters (only without pua_only)

Writes NAME-<version>.ttf, .woff, .woff2 (needs the `brotli` command line tool: $BROTLI or PATH), .map.json and .css, where
<version> is a hash of the map.
Python standard library only.
"""

import argparse
import base64
import hashlib
import json
import os
import struct
import subprocess
import sys
import unicodedata
from collections import OrderedDict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sfnt

UPEM = 4000
K = UPEM // 1000
HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_ICFONTS = ['Math-Italic']
ALL_FONTS = ['Main-Regular', 'Math-Italic', 'AMS-Regular', 'Size1-Regular', 'Size2-Regular', 'Size3-Regular', 'Size4-Regular',
             'Main-Bold', 'Main-Italic', 'Main-BoldItalic', 'Math-BoldItalic', 'Caligraphic-Regular', 'Caligraphic-Bold',
             'Fraktur-Regular', 'Fraktur-Bold', 'SansSerif-Regular', 'SansSerif-Bold', 'SansSerif-Italic', 'Script-Regular',
             'Typewriter-Regular']
BMP_PUA = (0xE000, 0xF8FF)
PLANE15 = 0xF0000


def math_italic_codepoints():
    """base character -> Mathematical Italic code point (real Unicode, so copying a formula gives readable text)."""
    m = {}
    for cp in range(0x1D400, 0x1D800):
        try:
            nm = unicodedata.name(chr(cp))
        except ValueError:
            continue
        if nm.startswith('MATHEMATICAL ITALIC ') and 'BOLD' not in nm and 'SANS' not in nm:
            base = unicodedata.normalize('NFKC', chr(cp))
            if len(base) == 1: m[ord(base)] = cp
    m[ord('h')] = 0x210E
    return m


class Slot:
    __slots__ = ('cp', 'key', 'adv', 'contours', 'comps', 'gid', 'bb', 'neg', 'note')

    def __init__(self, key, adv, contours=None, comps=None, neg=0, note=''):
        self.cp = None; self.key = key; self.adv = adv; self.contours = contours; self.comps = comps
        self.gid = None; self.bb = None; self.neg = neg; self.note = note


class Builder:
    def __init__(self, fonts_dir, spec):
        self.spec = spec
        self.dir = fonts_dir
        self.metrics = json.load(open(os.path.join(HERE, '..', 'data', 'font_metrics_data.json')))
        self.names = spec.get('fonts') or ALL_FONTS[:4]
        self.ic_fonts = spec.get('ic_fonts', DEFAULT_ICFONTS)
        self.src = {n: sfnt.SourceFont(os.path.join(fonts_dir, 'KaTeX_%s.ttf' % n)) for n in self.names}
        self.slots = []          # all slots in allocation order
        self.base = {}           # (font, char) -> Slot (the glyph with the font's own advance plus baked italic correction)
        self.noic = {}           # (font, char) -> Slot
        self.alt = {}            # (font, char) -> Slot: private-use twin of a glyph that has a real letter code point
        self.variants = {}       # (font, char, size, shift) -> Slot
        self.spacer = {}         # name -> Slot
        self.outline_owner = {}  # contour key -> Slot that owns the outline
        self.stats = {'outlines': 0, 'shared_outlines': 0, 'shared_bytes_saved': 0}
        self.planes = OrderedDict()   # plane name -> [slots]
        self.mi = math_italic_codepoints()

    # -------- helpers
    def metric(self, font, ch):
        return self.metrics.get(font, {}).get(str(ch))

    def new_slot(self, plane, slot):
        self.slots.append(slot)
        self.planes.setdefault(plane, []).append(slot)
        return slot

    def outline_slot(self, key, adv, contours, plane, note=''):
        """A glyph with the outline `contours`; when an identical outline exists, a one-component composite of that glyph."""
        ck = tuple(tuple(c) for c in contours)
        owner = self.outline_owner.get(ck) if contours and not self.spec.get('no_dedup') else None
        if owner is not None:
            self.stats['shared_outlines'] += 1
            s = Slot(key, adv, comps=[(owner, 0, 0, 1.0, 1.0)], note=note)
        else:
            s = Slot(key, adv, contours=contours, note=note)
            if contours:
                self.outline_owner[ck] = s
                self.stats['outlines'] += 1
        return self.new_slot(plane, s)

    # -------- glyph sets
    def add_base_glyphs(self):
        for name in self.names:
            f = self.src[name]
            keep = (self.spec.get('subset') or {}).get(name)
            for ch in sorted(f.cmap):
                if ch < 0x21 and ch != 0x20: continue
                if keep is not None and ch not in keep: continue
                gid = f.cmap[ch]
                units = f.adv[gid] * K
                ic = 0
                m = self.metric(name, ch)
                if name in self.ic_fonts and m and m[2] > 0:
                    ic = round(m[2] * UPEM)
                cont = [[(x * K, y * K, on) for x, y, on in c] for c in f.contours(gid)]
                s = self.outline_slot((name, ch), units + ic, cont, 'base:' + name, note='ic' if ic else '')
                s.neg = ic       # remembered baked italic correction (units)
                self.base[(name, ch)] = s
        # no-italic-correction variants: the same outline, the advance without the baked correction
        for (name, ch), b in list(self.base.items()):
            if b.neg:
                adv = b.adv - b.neg
                s = Slot((name, ch, 'noic'), adv, comps=[(b, 0, 0, 1.0, 1.0)])
                self.new_slot('noic', s)
                self.noic[(name, ch)] = s

    def add_alt_letters(self):
        """Chrome shapes a run per script and clamps a run's total advance at zero, so a negative spacer between two
        real Latin or Greek letters is lost. Twins of those letters at private-use code points (Unicode script Unknown,
        one run with the spacers) let the encoder avoid that."""
        for (name, ch), b in list(self.base.items()):
            if name == 'Main-Regular' and self.real_cp(name, ch) is not None and unicodedata.category(chr(ch)).startswith('L'):
                s = Slot((name, ch, 'alt'), b.adv, comps=[(b, 0, 0, 1.0, 1.0)])
                self.new_slot('alt', s)
                self.alt[(name, ch)] = s

    def real_cp(self, name, ch):
        """The real Unicode code point a base glyph is given, or None for a private-use one."""
        if self.spec.get('pua_only'): return None
        bad_real = lambda c: c < 0x21 or 0x7F <= c <= 0xA0 or c == 0xAD or 0x300 <= c <= 0x36F or 0x2000 <= c <= 0x200F or 0x2028 <= c <= 0x202F or 0xD800 <= c <= 0xF8FF
        if name == 'Main-Regular' and not bad_real(ch): return ch
        if name == 'Math-Italic' and ch in self.mi: return self.mi[ch]
        return None

    def add_script_variants(self):
        sp = self.spec
        pairs = sp.get('pairs') or []
        want = sp.get('script_glyphs')
        for pr in pairs:
            size, shift = pr[0], pr[1]
            pwant = set(pr[2]) if len(pr) > 2 and pr[2] is not None else (set(want) if want is not None else None)
            for (name, ch), b in self.base.items():
                if pwant is not None and '%s:%d' % (name, ch) not in pwant: continue
                sc = round(size * 16384) / 16384
                adv = round(b.adv * size)
                dy = round(shift * UPEM)
                if sp.get('flatten') and b.contours is not None:   # experiment: pre-scaled simple outlines instead of composites
                    cont = [[(round(x * size), round(y * size) + dy, on) for x, y, on in c] for c in b.contours]
                    s = Slot((name, ch, size, shift), adv, contours=cont)
                else:
                    s = Slot((name, ch, size, shift), adv, comps=[(b, 0, dy, sc, sc)])
                self.new_slot('script:%s:%s' % (size, shift), s)
                self.variants[(name, ch, size, shift)] = s

    def symbol_groups(self):
        """(font name, character) -> the atom groups KaTeX's math symbols table gives it (`bin`, `rel`, `punct`, ...)."""
        groups = {}
        names = {'main': 'Main-Regular', 'ams': 'AMS-Regular'}
        for e in json.load(open(os.path.join(HERE, '..', 'data', 'symbols.json'))):
            r = e.get('replace')
            if e['mode'] != 'math' or e['font'] not in names or not r: continue
            if r.startswith('\\u'): r = chr(int(r[2:], 16))
            if len(r) == 1: groups.setdefault((names[e['font']], ord(r)), set()).add(e['group'])
        return groups

    def add_baked(self):
        """Glyphs that carry TeX's spacing: the base glyph at an x offset and with a larger advance, so that `a + b` is three
        glyphs and not three glyphs and the spacers of both gaps. spec["spacing"] maps an atom group (bin, rel, punct) to the
        (left, right) gaps in em that get a glyph, for every glyph of the font that the symbols table puts in the group;
        spec["script_space"] is [[size, shift, gap], ...]: the glyphs of those script variants with a gap after them (KaTeX's
        script space) when they end a script."""
        sp = self.spec
        self.baked = {}
        groups = self.symbol_groups()
        for (name, ch), b in self.base.items():
            for grp in sorted(groups.get((name, ch), ())):
                for left, right in (sp.get('spacing') or {}).get(grp, []):
                    lu, ru = round(left * UPEM), round(right * UPEM)
                    if (name, ch, 1.0, 0.0, lu, ru) in self.baked: continue
                    s = Slot((name, ch, 1.0, 0.0, lu, ru), b.adv + lu + ru, comps=[(b, lu, 0, 1.0, 1.0)])
                    self.new_slot('baked', s)
                    self.baked[(name, ch, 1.0, 0.0, lu, ru)] = s
        for size, shift, gap in sp.get('script_space') or []:
            for (name, ch, sz, sh), v in list(self.variants.items()):
                if sz != size or sh != shift: continue
                b = self.base[(name, ch)]
                gu = round(gap * UPEM)
                s = Slot((name, ch, size, shift, 0, gu), v.adv + gu, comps=[(b, 0, round(shift * UPEM), round(size * 16384) / 16384, round(size * 16384) / 16384)])
                self.new_slot('baked', s)
                self.baked[(name, ch, size, shift, 0, gu)] = s

    def add_spacers(self):
        sp = self.spec
        for nm, em in (sp.get('spacers') or {}).items():
            if em < 0:
                s = Slot(('space', nm), 0, contours=[], neg=round(em * UPEM))
                self.new_slot('negspacers', s)
            else:
                s = Slot(('space', nm), round(em * UPEM), contours=[])
                self.new_slot('spacers', s)
            self.spacer[nm] = s
        # U+200B: where the encoder writes a line break opportunity (`compact`, class `mw`). A font that has it keeps the browser from
        # asking another font for it, which would put that font's ascent and descent in the line.
        self.zwsp = self.new_slot('zwsp', Slot(('space', 'zwsp'), 0, contours=[]))
        self.zwsp.cp = 0x200B
        # U+202F: the content of the strut of a formula that wraps (it is a no-break character, so that the strut stays with the text
        # that follows it, and has no advance)
        self.wj = self.new_slot('zwsp', Slot(('space', 'nnbsp'), 0, contours=[]))
        self.wj.cp = 0x202F
        self.pos_bin = {}; self.neg_bin = {}
        for k in range(0, 15):
            s = Slot(('space', '+%d' % (1 << k)), 1 << k, contours=[])
            self.new_slot('spacers', s); self.pos_bin[1 << k] = s
        for k in range(0, 15):
            s = Slot(('space', '-%d' % (1 << k)), 0, contours=[], neg=-(1 << k))
            self.new_slot('negspacers', s); self.neg_bin[1 << k] = s

    def add_rules(self):
        """Rule glyphs: one unit rectangle (2 em wide) scaled in x and y by a composite with an x/y scale, at a vertical
        offset. A family is (thickness, bottom edge height) and has widths 2^k/512 em (k = 0..10), so a bar of any width
        is a few glyphs."""
        fams = self.spec.get('rules') or []
        self.rules = {}
        if not fams: return
        unit = Slot(('rule', 'unit'), 2 * UPEM, contours=[[(0, 0, True), (0, UPEM, True), (2 * UPEM, UPEM, True), (2 * UPEM, 0, True)]])
        self.new_slot('rules', unit)
        for th, shift in fams:
            for k in range(0, 11):
                w = (1 << k) / 512
                s = Slot(('rule', th, shift, k), round(w * UPEM), comps=[(unit, 0, round(shift * UPEM), w / 2, th)])
                self.new_slot('rules', s)
                self.rules[(th, shift, k)] = s

    def add_overlays(self):
        """Accent composites: two components (base glyph, accent glyph at dx, dy and size)."""
        self.overlays = {}
        for o in self.spec.get('overlays') or []:
            fb, cb = o['base'].rsplit(':', 1); fo, co = o['over'].rsplit(':', 1)
            b = self.base.get((fb, int(cb))); a = self.base.get((fo, int(co)))
            if b is None or a is None: continue
            size = o.get('size', 1.0)
            sc = round(size * 16384) / 16384
            comps = [(b, 0, 0, 1.0, 1.0), (a, round(o['dx'] * UPEM), round(o['dy'] * UPEM), sc, sc)]
            s = Slot(('overlay', o['base'], o['over'], o['dx'], o['dy']), b.adv, comps=comps)
            self.new_slot('overlays', s)
            self.overlays[(o['base'], o['over'], o['dx'], o['dy'])] = s

    # -------- code points
    def allocate(self):
        used = {0x200B, 0x202F}
        mi = self.mi
        def claim(s, cp):
            if cp in used: return False
            used.add(cp); s.cp = cp; return True
        for (name, ch), s in self.base.items():
            r = self.real_cp(name, ch)
            if r is not None: claim(s, r)
        order = ['spacers', 'noic', 'alt'] + [p for p in self.planes if p.startswith('base:')] + \
                [p for p in self.planes if p.startswith('script:')] + ['baked', 'rules', 'overlays', 'negspacers']
        nxt = BMP_PUA[0]
        def next_cp():
            nonlocal nxt
            while True:
                c = nxt; nxt += 1
                if nxt - 1 == BMP_PUA[1] + 1: nxt = PLANE15
                if c not in used:
                    used.add(c); return c
        for p in order:
            for s in self.planes.get(p, []):
                if s.cp is None: s.cp = next_cp()
        # glyph ids run in code point order, .notdef first
        # negative spacers (advance 0) go last so numberOfHMetrics can leave out their hmtx entries; their code points stay one cmap group
        self.ordered = sorted(self.slots, key=lambda s: (s.key[0] == 'space' and s.neg < 0, s.cp))
        for i, s in enumerate(self.ordered): s.gid = i + 1

    # -------- serialisation
    def compile(self, family):
        spec = self.spec
        asc = round(spec.get('ascent', 0.6) * UPEM); desc = round(spec.get('descent', 0.6) * UPEM)
        glyf = bytearray(); loca = [0]
        hm = []   # (advance, lsb)
        # .notdef: an empty box
        nd = [[(500, 0, True), (500, 5000, True), (3500, 5000, True), (3500, 0, True)], [(800, 300, True), (3200, 300, True), (3200, 4700, True), (800, 4700, True)]]
        rec, bb = sfnt.encode_simple(nd)
        glyf += rec + b'\0' * (-len(rec) % 4); loca.append(len(glyf)); hm.append((4000, bb[0]))
        max_pts = 0; max_con = 0
        self.compile_bbox()
        for s in self.ordered:
            if s.contours is not None and s.comps is None:
                rec, bb = sfnt.encode_simple(s.contours)
                max_pts = max(max_pts, sum(len(c) for c in s.contours)); max_con = max(max_con, len(s.contours))
            else:
                rec = sfnt.encode_composite([(c[0].gid, c[1], c[2], c[3], c[4]) for c in s.comps], s.bb)
                bb = s.bb
            glyf += rec + b'\0' * (-len(rec) % 4)
            loca.append(len(glyf))
            hm.append((s.adv, bb[0] if rec else 0))
        n = len(hm)
        # loca: short when the glyf table is under 128 KB (offsets / 2 fit in 16 bits)
        short = len(glyf) < 131072 and all(o % 2 == 0 for o in loca)
        loca_b = b''.join(struct.pack('>H', o // 2) for o in loca) if short else b''.join(struct.pack('>I', o) for o in loca)
        # hmtx: trailing run of glyphs with the advance of the last one stores only the lsb
        nh = n
        while nh > 1 and hm[nh - 2][0] == hm[n - 1][0]: nh -= 1
        hmtx = b''.join(struct.pack('>Hh', a, l) for a, l in hm[:nh]) + b''.join(struct.pack('>h', l) for a, l in hm[nh:])
        allbb = [self.bb_of(s) for s in self.ordered]
        gx0 = min(b[0] for b in allbb); gy0 = min(b[1] for b in allbb); gx1 = max(b[2] for b in allbb); gy1 = max(b[3] for b in allbb)
        head = struct.pack('>IIIIHHqqhhhhHHhhh', 0x10000, 0x10000, 0, 0x5F0F3CF5, 3, UPEM, 0, 0, gx0, gy0, gx1, gy1, 0, 8, 2, 0 if short else 1, 0)
        head = head[:8] + struct.pack('>II', 0, 0x5F0F3CF5) + head[16:]
        hhea = struct.pack('>IhhhHhhhhhhhhhhhH', 0x10000, asc, -desc, 0, max(a for a, l in hm), min(l for a, l in hm), 0,
                           gx1, 1, 0, 0, 0, 0, 0, 0, 0, nh)
        maxp = struct.pack('>IHHHHHHHHHHHHHH', 0x10000, n, max_pts, max_con, 0, 0, 2, 0, 0, 0, 0, 0, 0, 1, 1)
        pairs = [(s.cp, s.gid) for s in self.ordered]
        cmap, ngroups = sfnt.build_cmap(pairs)
        self.cmap_groups = ngroups
        adjust = {s.gid: s.neg for s in self.ordered if s.neg < 0 and s.key[0] == 'space'}
        tables = {'head': head, 'hhea': hhea, 'maxp': maxp, 'hmtx': hmtx, 'cmap': cmap, 'glyf': bytes(glyf), 'loca': loca_b,
                  'name': sfnt.name_table(family), 'post': struct.pack('>IIhhIIIII', 0x30000, 0, -asc // 4, 400, 0, 0, 0, 0, 0)}
        tables['OS/2'] = self.os2(asc, desc, pairs, gx1)
        if adjust and not spec.get('no_gpos'): tables['GPOS'] = sfnt.build_gpos(adjust, tuple(t.encode() for t in spec['gpos_features'])) if spec.get('gpos_features') else sfnt.build_gpos(adjust)
        self.sizes = {k: len(v) for k, v in tables.items()}
        self.short_loca = short
        return sfnt.build_sfnt(tables)

    def bb_of(self, s):
        return s.bb if s.bb is not None else (sfnt.bbox_of(s.contours) if s.contours else (0, 0, 0, 0))

    def compile_bbox(self):
        done = set()
        def bb(s):
            if s.bb is not None: return s.bb
            if s.comps is None:
                s.bb = sfnt.bbox_of(s.contours) if s.contours else (0, 0, 0, 0)
                return s.bb
            xs0 = []; ys0 = []; xs1 = []; ys1 = []
            for ref, dx, dy, sx, sy in s.comps:
                b = bb(ref)
                if b == (0, 0, 0, 0): continue
                sxq = round(sx * 16384) / 16384; syq = round(sy * 16384) / 16384
                xs0.append(round(b[0] * sxq) + dx); xs1.append(round(b[2] * sxq) + dx)
                ys0.append(round(b[1] * syq) + dy); ys1.append(round(b[3] * syq) + dy)
            s.bb = (min(xs0), min(ys0), max(xs1), max(ys1)) if xs0 else (0, 0, 0, 0)
            for v in s.bb:
                if not -32768 <= v <= 32767: raise ValueError('bbox out of int16 for %r: %r' % (s.key, s.bb))
            return s.bb
        for s in self.slots: bb(s)

    def os2(self, asc, desc, pairs, xmax):
        lo = min(c for c, g in pairs if c < 0xFFFF); hi = max([c for c, g in pairs if c <= 0xFFFF] or [0])
        avg = sum(s.adv for s in self.ordered) // max(1, len(self.ordered))
        return self._os2(asc, desc, lo, hi, avg)

    def _os2(self, asc, desc, lo, hi, avg):
        b = struct.pack('>HhHHH', 4, avg, 400, 5, 0)
        b += struct.pack('>hhhhhhhhhh', 5600, 5200, 0, 1000, 5600, 5200, 0, 3200, 400, 2400)  # sub/super sizes and offsets, strikeout
        b += struct.pack('>h', 0) + b'\0' * 10                                               # family class, panose
        b += struct.pack('>IIII', 0, 0, 0, 0) + b'    '                                      # unicode ranges, vendor
        b += struct.pack('>HHHhhhHH', 0x40 | 0x80, lo, hi, asc, -desc, 0, asc, desc)         # REGULAR | USE_TYPO_METRICS
        b += struct.pack('>II', 1, 0)                                                         # code pages: latin 1
        b += struct.pack('>hhHHH', 4000, 5600, 0, 0x20, 1)
        return b


def blank_font(ranges=((0xE000, 0xF8FF), (0xF0000, 0xFFFFD)), name='KaTeXMathBlank'):
    """A font whose only job is to show nothing for the private-use code points, used as the fallback of the maths font so
    that spacers do not draw as missing-glyph boxes while the real font loads or when it fails."""
    box, _ = sfnt.encode_simple([[(50, 0, True), (50, 700, True), (450, 700, True), (450, 0, True)]])   # OTS wants a non-empty glyf
    glyf = box + b'\0' * (-len(box) % 4)
    loca = struct.pack('>HHH', 0, len(glyf) // 2, len(glyf) // 2)
    hmtx = struct.pack('>HhHh', 500, 50, 0, 0)
    head = struct.pack('>IIIIHHqqhhhhHHhhh', 0x10000, 0x10000, 0, 0x5F0F3CF5, 3, 1000, 0, 0, 0, 0, 500, 700, 0, 8, 2, 0, 0)
    head = head[:8] + struct.pack('>II', 0, 0x5F0F3CF5) + head[16:]
    hhea = struct.pack('>IhhhHhhhhhhhhhhhH', 0x10000, 800, -200, 0, 500, 0, 0, 500, 1, 0, 0, 0, 0, 0, 0, 0, 2)
    maxp = struct.pack('>IHHHHHHHHHHHHHH', 0x10000, 2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 1, 1)
    b = Builder.__new__(Builder)
    tables = {'head': head, 'hhea': hhea, 'maxp': maxp, 'hmtx': hmtx, 'cmap': sfnt.build_cmap13(ranges), 'glyf': glyf, 'loca': loca,
              'name': sfnt.name_table(name), 'post': struct.pack('>IIhhIIIII', 0x30000, 0, -100, 50, 0, 0, 0, 0, 0)}
    tables['OS/2'] = Builder._os2(b, 800, 200, 0xE000, 0xF8FF, 0)
    return sfnt.build_sfnt(tables)


def brotli_cli(data):
    """Brotli at quality 11 with the largest window through the `brotli` command line tool ($BROTLI or PATH)."""
    exe = os.environ.get('BROTLI', 'brotli')
    try:
        return subprocess.run([exe, '-q', '11', '-w', '24', '-c'], input=data, stdout=subprocess.PIPE, check=True).stdout
    except FileNotFoundError:
        sys.exit("gen_math_font.py: the brotli command line tool is needed for the WOFF 2 output and was not found (%r). Install the "
                 "distribution package `brotli` (Google's reference CLI) or set BROTLI=/path/to/brotli. It is a build-time tool only: the "
                 "generated .woff2 is checked in." % exe)


def build(fonts_dir, spec, outdir, name):
    global UPEM, K
    UPEM = spec.get('upem', 4000); K = UPEM // 1000
    b = Builder(fonts_dir, spec)
    b.add_base_glyphs()
    if spec.get('alt_letters', True): b.add_alt_letters()
    b.add_spacers()
    b.add_script_variants()
    b.add_baked()
    b.add_rules()
    b.add_overlays()
    b.allocate()
    ttf = b.compile(name)
    os.makedirs(outdir, exist_ok=True)
    tmp = os.path.join(outdir, name)
    write_map(b, tmp + '.map.json', name)
    # the version is a hash of what the encoder depends on (code points and advances), part of the file and family name
    ver = hashlib.sha256(open(tmp + '.map.json', 'rb').read()).hexdigest()[:8]
    stem = '%s-%s' % (name, ver)
    p = os.path.join(outdir, stem)
    os.replace(tmp + '.map.json', p + '.map.json')
    open(p + '.ttf', 'wb').write(ttf)
    open(p + '.woff', 'wb').write(sfnt.to_woff1(ttf))
    open(p + '.woff2', 'wb').write(sfnt.to_woff2(ttf, brotli_cli, transform=True))
    open(p + '.untransformed.woff2', 'wb').write(sfnt.to_woff2(ttf, brotli_cli, transform=False))
    blank = blank_font()
    bp = os.path.join(outdir, 'KaTeXMathBlank')
    open(bp + '.ttf', 'wb').write(blank)
    open(bp + '.woff2', 'wb').write(sfnt.to_woff2(blank, brotli_cli, transform=True))
    open(p + '.css', 'w').write(css_snippet(stem))
    b.version = ver; b.stem = stem
    return b, ttf


def em(units): return round(units / UPEM, 6)


def write_map(b, path, name):
    m = OrderedDict()
    m['unitsPerEm'] = UPEM
    m['family'] = name
    m['ascent'] = b.spec.get('ascent', 0.6); m['descent'] = b.spec.get('descent', 0.6)
    glyphs = OrderedDict()
    for (font, ch), s in b.base.items():
        e = {'cp': s.cp, 'adv': em(s.adv)}
        if s.neg: e['ic'] = em(s.neg)
        n = b.noic.get((font, ch))
        if n: e['noic'] = n.cp
        al = b.alt.get((font, ch))
        if al: e['alt'] = al.cp
        glyphs.setdefault(font, OrderedDict())[str(ch)] = e
    m['glyphs'] = glyphs
    var = OrderedDict()
    for (font, ch, size, shift), s in b.variants.items():
        var.setdefault('%s,%s' % (size, shift), OrderedDict())['%s:%d' % (font, ch)] = [s.cp, em(s.adv)]
    m['variants'] = [{'size': float(k.split(',')[0]), 'shift': float(k.split(',')[1]), 'glyphs': v} for k, v in var.items()]
    m['breaks'] = {'zwsp': b.zwsp.cp, 'nnbsp': b.wj.cp}
    m['baked'] = [[k[0], k[1], k[2], k[3], round(k[4]), round(k[5]), s.cp, em(s.adv)] for k, s in b.baked.items()]
    m['spacers'] = {'named': {k: [s.cp, em(s.adv + s.neg)] for k, s in b.spacer.items()},
                    'pos': {str(k): s.cp for k, s in b.pos_bin.items()}, 'neg': {str(k): s.cp for k, s in b.neg_bin.items()},
                    'unit': 1 / UPEM}
    m['rules'] = [{'thickness': th, 'shift': sh, 'k': k, 'cp': s.cp, 'width': em(s.adv)} for (th, sh, k), s in b.rules.items()]
    m['overlays'] = [{'base': k[0], 'over': k[1], 'dx': k[2], 'dy': k[3], 'cp': s.cp, 'adv': em(s.adv)} for k, s in b.overlays.items()]
    json.dump(m, open(path, 'w'), separators=(',', ':'))


def css_snippet(stem):
    """Reference CSS for the font (the Rust encoder's `mathrun::font_css` writes the same rules plus the strut classes). --h/--v are
    the formula's strut: its height and its vertical-align, in em of the formula."""
    return ("@font-face { font-family: %s; src: url(%s.woff2) format('woff2'); font-display: block; }\n"
            ".mf { font: normal 1.21em/0 %s; white-space: nowrap; text-indent: 0; text-rendering: geometricPrecision; letter-spacing: normal; }\n"
            ".mf::before { content: ''; display: inline-block; height: var(--h); vertical-align: var(--v); }\n"
            ".katex-display > .mf { display: block; text-align: center; }\n" % (stem, stem, stem))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--fonts', required=True); ap.add_argument('--spec'); ap.add_argument('--out', required=True)
    ap.add_argument('--name', default='KaTeXMathRun')
    a = ap.parse_args()
    spec = json.load(open(a.spec)) if a.spec else {}
    b, ttf = build(a.fonts, spec, a.out, a.name)
    print('version', b.version, 'glyphs', len(b.ordered) + 1, 'ttf', len(ttf), 'tables', b.sizes, 'cmap groups', b.cmap_groups, 'stats', b.stats)


if __name__ == '__main__':
    main()
