"""Encoder prototype: a list of positioned glyphs -> one string of code points for the generated math font.

This is what a Rust encoder would do: it walks the box tree (or here, KaTeX's measured glyph rectangles) and keeps
an exact cursor in font units. Every glyph is written as a code point; whenever the next glyph's x differs from the
cursor, spacer glyphs (named, or the binary +-2^k set) close the gap, negative ones moving the cursor back so that
superscripts and subscripts can overlap.

A glyph is [font, char, size, x_em, y_em, width_em, hint]: the source font, its character, its size relative to the
formula's, the left edge and the baseline height in the formula's em.
"""

import collections
import json


def popcount(n): return bin(n).count('1')


class Encoder:
    def __init__(self, map_path_or_dict):
        m = json.load(open(map_path_or_dict)) if isinstance(map_path_or_dict, str) else map_path_or_dict
        self.m = m
        self.upem = m['unitsPerEm']
        self.base = {}
        for font, d in m['glyphs'].items():
            for ch, e in d.items():
                self.base[(font, int(ch))] = (e['cp'], round(e['adv'] * self.upem), round(e.get('ic', 0) * self.upem), e.get('noic'), e.get('alt'))
        self.var = {}
        for v in m['variants']:
            for k, (cp, adv) in v['glyphs'].items():
                font, ch = k.rsplit(':', 1)
                self.var[(font, int(ch), v['size'], round(v['shift'], 3))] = (cp, round(adv * self.upem))
        sp = m['spacers']
        self.named = sorted((round(a * self.upem), cp) for cp, a in [(v[0], v[1]) for v in sp['named'].values()])
        self.pos = {int(k): v for k, v in sp['pos'].items()}
        self.neg = {int(k): v for k, v in sp['neg'].items()}
        self.maxbin = max(self.pos)
        self.overlays = collections.defaultdict(list)   # (base 'Font:char', accent 'Font:char') -> [(dx, dy, cp, adv units)]
        for o in m.get('overlays', []):
            self.overlays[(o['base'], o['over'])].append((o['dx'], o['dy'], o['cp'], round(o['adv'] * self.upem)))
        self.rules = collections.defaultdict(dict)   # (thickness, bottom) -> {k: (cp, advance units)}
        for r in m.get('rules', []):
            self.rules[(r['thickness'], r['shift'])][r['k']] = (r['cp'], round(r['width'] * self.upem))

    # ---- spacers
    def bin_cost(self, units):
        n = abs(units)
        big, rem = divmod(n, self.maxbin)
        return big + popcount(rem)

    def spacer(self, units, tol=1):
        """Code points advancing the cursor by `units`, to within `tol` units (the rest as binary spacers)."""
        if abs(units) <= tol: return [], 0
        best = None
        cands = [(0, None)] + [(n, cp) for n, cp in self.named if (n > 0) == (units > 0) and abs(n) <= abs(units) + tol]
        for n, cp in cands:
            rem = units - n
            if abs(rem) <= tol: rem = 0
            elif abs(rem) % 2: rem += 1 if rem > 0 else -1   # an even number of units: one binary spacer fewer
            cost = (1 if cp else 0) + self.bin_cost(rem)
            if best is None or cost < best[0]: best = (cost, n, cp, rem)
        _, n, cp, rem = best
        out = []; moved = 0
        if cp is not None: out.append(cp); moved += n
        table = self.pos if rem > 0 else self.neg
        r = abs(rem); step = self.maxbin
        while r:
            while step > r: step >>= 1
            out.append(table[step]); r -= step
            moved += step if rem > 0 else -step
        return out, moved

    # ---- glyphs
    def lookup(self, g, noic=False, alt=False):
        font, ch, size, x, y = g[0], g[1], g[2], g[3], g[4]
        if abs(size - 1) < 1e-6 and abs(y) < 0.0006:
            b = self.base.get((font, ch))
            if b is None: return None
            if noic and b[3] is not None: return (b[3], b[1] - b[2])
            if alt and b[4] is not None: return (b[4], b[1])
            return (b[0], b[1])
        v = self.var.get((font, ch, round(size, 4), round(y, 3)))
        if v is None:
            v = self.var.get((font, ch, round(size, 4), round(y + 0.001, 3))) or self.var.get((font, ch, round(size, 4), round(y - 0.001, 3)))
        return v

    def merge_overlays(self, glyphs):
        """Replace (base glyph, accent glyph) pairs that exist as one composite glyph by ['overlay', cp, advance units, x, y]."""
        out = []
        for i, g in enumerate(glyphs):
            if g[6:7] == ['accbody'] and out and out[-1][0] != 'overlay' and out[-1][0] != 'rule':
                b = out[-1]
                for dx, dy, cp, adv in self.overlays.get(('%s:%d' % (b[0], b[1]), '%s:%d' % (g[0], g[1])), []):
                    if abs(g[3] - b[3] - dx) < 0.01 and abs(g[4] - b[4] - dy) < 0.01:
                        out[-1] = ['overlay', cp, adv, b[3], b[4], 0, '']
                        break
                else: out.append(g)
            else: out.append(g)
        return out

    def encode(self, glyphs, order='dom', tol=None, width=None, alt=False):
        if tol is None: tol = max(1, round(0.00075 * self.upem))   # 1/64 px of the measured layout, in font units
        """Returns (code point list, missing glyph keys, number of spacer code points)."""
        if self.overlays: glyphs = self.merge_overlays(glyphs)
        idx = list(range(len(glyphs)))
        if order == 'x': idx.sort(key=lambda i: (round(glyphs[i][3], 4), i))
        cps = []; missing = []; cursor = 0; nsp = 0; prev_end = None; trace = []   # trace[k] = index into glyphs of cps[k], None for a spacer
        for pos, i in enumerate(idx):
            g = glyphs[i]
            xu = round(g[3] * self.upem)
            # measured positions are quantised to 1/64 px: a gap that is within `tol` of the previous glyph's end is no gap,
            # as long as the cursor stays within 2*tol of the target (an exact encoder in Rust would not need this)
            rel = xu - prev_end if prev_end is not None else xu
            if abs(rel) <= tol and abs(xu - cursor) <= 2 * tol: sp, moved = [], 0
            else: sp, moved = self.spacer(xu - cursor, tol)
            cps += sp; nsp += len(sp); cursor += moved; trace += [None] * len(sp)
            nxt = glyphs[idx[pos + 1]] if pos + 1 < len(idx) else None
            use_noic = False
            b = self.base.get((g[0], g[1]))
            if nxt is not None and b is not None and b[2] and abs(g[2] - 1) < 1e-6:
                nx = round(nxt[3] * self.upem)
                if abs(nx - (xu + b[1] - b[2])) < abs(nx - (xu + b[1])): use_noic = True
            if g[0] == 'rule':   # ['rule', thickness, 1, x, bottom, width]: pieces of 2^k/512 em from a (thickness, bottom) family
                fam = self.rules.get((g[1], round(g[4], 3)))
                if fam is None: missing.append(('rule', g[1], round(g[4], 3))); continue
                left = round(g[5] * self.upem); done = 0
                for k in sorted(fam, reverse=True):
                    cpk, ak = fam[k]
                    while left - done >= ak - ak // 2:   # the last piece may overshoot by half of itself at most
                        cps.append(cpk); trace.append(None); cursor += ak; done += ak
                continue
            if g[0] == 'overlay':
                cps.append(g[1]); cursor += g[2]; trace.append(i); prev_end = xu + g[2]
                continue
            r = self.lookup(g, use_noic, alt)
            if r is None:
                missing.append((g[0], g[1], g[2], round(g[4], 3)))
                continue
            cps.append(r[0]); cursor += r[1]; trace.append(i); prev_end = xu + r[1]
        if width is not None:   # trailing space (script space, margins) so the formula is as wide as KaTeX's
            sp, moved = self.spacer(round(width * self.upem) - cursor, tol)
            cps += sp; nsp += len(sp); trace += [None] * len(sp)
        return cps, missing, nsp, trace

    def best(self, glyphs, width=None, tol=None):
        """Shortest of the encodings (document order or x order). A formula that moves the cursor back (negative spacers)
        uses the private-use twins of real letters: Chrome drops the negative advance of a run between two real letters."""
        negcps = set(self.neg.values()) | {cp for n, cp in self.named if n < 0}
        res = []
        for order in ('dom', 'x'):
            r = self.encode(glyphs, order, tol, width)
            if negcps & set(r[0]): r = self.encode(glyphs, order, tol, width, alt=True)
            res.append(r)
        return min(res, key=lambda r: len(r[0]))

    def gaps(self, glyphs, width=None):
        """Histogram input: the gaps (units) between the cursor and each glyph, with all-binary spacers."""
        out = []; cursor = 0
        for g in glyphs:
            xu = round(g[3] * self.upem)
            r = self.lookup(g, False)
            if r is None: continue
            out.append(xu - cursor); cursor = xu + r[1]
        if width is not None: out.append(round(width * self.upem) - cursor)
        return out


def to_html(cps):
    s = ''.join(chr(c) for c in cps)
    return s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
