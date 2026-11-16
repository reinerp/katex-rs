"""Minimal sfnt (TrueType) reader and writer, Python standard library only.

Just enough for gen_math_font.py: read the glyf outlines, advances and cmap of KaTeX's TTF files, and
write a new TrueType font (glyf with simple and composite glyphs, hmtx, cmap 4 and 12, GPOS with one
single-adjustment lookup, OS/2, name, post 3) plus its WOFF 1 and (untransformed) WOFF 2 wrappers.
"""

import struct
import zlib


def u16(d, o): return struct.unpack_from('>H', d, o)[0]
def i16(d, o): return struct.unpack_from('>h', d, o)[0]
def u32(d, o): return struct.unpack_from('>I', d, o)[0]


# ---------------------------------------------------------------- reading

class SourceFont:
    """One KaTeX TTF: outlines flattened to contours, advances, cmap, and the byte counts of its hinting."""

    def __init__(self, path):
        d = self.data = open(path, 'rb').read()
        n = u16(d, 4)
        self.tables = {}
        for i in range(n):
            tag, _c, off, ln = struct.unpack_from('>4sIII', d, 12 + 16 * i)
            self.tables[tag.decode()] = (off, ln)
        t = self.tables
        self.upem = u16(d, t['head'][0] + 18)
        self.loca_long = i16(d, t['head'][0] + 50)
        self.num_glyphs = u16(d, t['maxp'][0] + 4)
        nh = u16(d, t['hhea'][0] + 34)
        ho = t['hmtx'][0]
        self.adv = []
        for g in range(self.num_glyphs):
            self.adv.append(u16(d, ho + 4 * min(g, nh - 1)))
        lo = t['loca'][0]
        if self.loca_long:
            self.loca = [u32(d, lo + 4 * i) for i in range(self.num_glyphs + 1)]
        else:
            self.loca = [2 * u16(d, lo + 2 * i) for i in range(self.num_glyphs + 1)]
        self.instr_bytes = 0
        self._contours = {}
        self.cmap = self._read_cmap()

    def hint_bytes(self):
        """Bytes of hinting in the source: glyph programs plus fpgm, prep, cvt and gasp."""
        extra = sum(self.tables[k][1] for k in ('fpgm', 'prep', 'cvt ', 'gasp') if k in self.tables)
        return self.instr_bytes, extra

    def _read_cmap(self):
        d = self.data
        co = self.tables['cmap'][0]
        best = {}
        for i in range(u16(d, co + 2)):
            pid, eid, off = struct.unpack_from('>HHI', d, co + 4 + 8 * i)
            o = co + off
            fmt = u16(d, o)
            m = {}
            if fmt == 4:
                segx2 = u16(d, o + 6)
                seg = segx2 // 2
                ends = [u16(d, o + 14 + 2 * k) for k in range(seg)]
                starts = [u16(d, o + 16 + segx2 + 2 * k) for k in range(seg)]
                deltas = [i16(d, o + 16 + 2 * segx2 + 2 * k) for k in range(seg)]
                ro = o + 16 + 3 * segx2
                ros = [u16(d, ro + 2 * k) for k in range(seg)]
                for k in range(seg):
                    for c in range(starts[k], ends[k] + 1):
                        if c == 0xFFFF: continue
                        if ros[k] == 0:
                            g = (c + deltas[k]) & 0xFFFF
                        else:
                            a = ro + 2 * k + ros[k] + 2 * (c - starts[k])
                            g = u16(d, a)
                            if g: g = (g + deltas[k]) & 0xFFFF
                        if g: m[c] = g
            elif fmt == 12:
                for k in range(u32(d, o + 12)):
                    s, e, g = struct.unpack_from('>III', d, o + 16 + 12 * k)
                    for c in range(s, e + 1): m[c] = g + c - s
            elif fmt == 0:
                for c in range(256):
                    if d[o + 6 + c]: m[c] = d[o + 6 + c]
            else:
                continue
            if len(m) > len(best): best = m
        return best

    def contours(self, gid):
        """Flattened outline of a glyph as a list of contours of (x, y, on_curve) in source units."""
        if gid in self._contours: return self._contours[gid]
        d = self.data
        a, b = self.loca[gid], self.loca[gid + 1]
        if b == a:
            r = []
        else:
            o = self.tables['glyf'][0] + a
            nc = i16(d, o)
            if nc >= 0:
                r = self._simple(o, nc)
            else:
                r = []
                p = o + 10
                while True:
                    fl, gi = struct.unpack_from('>HH', d, p); p += 4
                    if fl & 1:
                        a1, a2 = struct.unpack_from('>hh', d, p); p += 4
                    else:
                        a1, a2 = struct.unpack_from('>bb', d, p); p += 2
                    if not fl & 2: raise ValueError('point-matched composite')
                    m = [1.0, 0.0, 0.0, 1.0]
                    if fl & 8:
                        s = i16(d, p) / 16384; p += 2; m = [s, 0, 0, s]
                    elif fl & 0x40:
                        sx, sy = i16(d, p) / 16384, i16(d, p + 2) / 16384; p += 4; m = [sx, 0, 0, sy]
                    elif fl & 0x80:
                        v = [i16(d, p + 2 * k) / 16384 for k in range(4)]; p += 8; m = v
                    for c in self.contours(gi):
                        r.append([(round(m[0] * x + m[2] * y + a1), round(m[1] * x + m[3] * y + a2), on) for x, y, on in c])
                    if not fl & 0x20:
                        if fl & 0x100:
                            self.instr_bytes += u16(d, p)
                        break
        self._contours[gid] = r
        return r

    def _simple(self, o, nc):
        d = self.data
        ends = [u16(d, o + 10 + 2 * k) for k in range(nc)]
        il = u16(d, o + 10 + 2 * nc)
        self.instr_bytes += il
        p = o + 12 + 2 * nc + il
        n = ends[-1] + 1 if nc else 0
        flags = []
        while len(flags) < n:
            f = d[p]; p += 1
            flags.append(f)
            if f & 8:
                r = d[p]; p += 1
                flags += [f] * r
        xs = []; x = 0
        for f in flags:
            if f & 2:
                dx = d[p]; p += 1
                x += dx if f & 16 else -dx
            elif not f & 16:
                x += i16(d, p); p += 2
            xs.append(x)
        ys = []; y = 0
        for f in flags:
            if f & 4:
                dy = d[p]; p += 1
                y += dy if f & 32 else -dy
            elif not f & 32:
                y += i16(d, p); p += 2
            ys.append(y)
        out = []; s = 0
        for e in ends:
            out.append([(xs[k], ys[k], bool(flags[k] & 1)) for k in range(s, e + 1)])
            s = e + 1
        return out


# ---------------------------------------------------------------- writing glyphs

def bbox_of(contours):
    pts = [(x, y) for c in contours for x, y, _ in c]
    if not pts: return (0, 0, 0, 0)
    xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
    return (min(xs), min(ys), max(xs), max(ys))


def encode_simple(contours):
    """glyf record of a simple glyph without instructions; flags use the repeat flag and short vectors."""
    if not contours: return b'', (0, 0, 0, 0)
    bb = bbox_of(contours)
    out = bytearray(struct.pack('>hhhhh', len(contours), *bb))
    e = -1
    for c in contours:
        e += len(c); out += struct.pack('>H', e)
    out += struct.pack('>H', 0)
    flags = []; xb = bytearray(); yb = bytearray()
    px = py = 0
    for c in contours:
        for x, y, on in c:
            dx, dy = x - px, y - py; px, py = x, y
            f = 1 if on else 0
            if dx == 0: f |= 16
            elif -255 <= dx <= 255:
                f |= 2 | (16 if dx > 0 else 0); xb.append(abs(dx))
            else: xb += struct.pack('>h', dx)
            if dy == 0: f |= 32
            elif -255 <= dy <= 255:
                f |= 4 | (32 if dy > 0 else 0); yb.append(abs(dy))
            else: yb += struct.pack('>h', dy)
            flags.append(f)
    i = 0
    while i < len(flags):
        f = flags[i]; j = i
        while j + 1 < len(flags) and flags[j + 1] == f and j - i < 255: j += 1
        if j > i:
            out += bytes([f | 8, j - i])
        else:
            out.append(f)
        i = j + 1
    out += xb + yb
    return bytes(out), bb


def f2dot14(v):
    r = round(v * 16384)
    if not -32768 <= r <= 32767: raise ValueError('scale out of F2Dot14 range: %r' % v)
    return r


def encode_composite(comps, bb):
    """comps: list of (gid, dx, dy, sx, sy). Offsets are in the composite's own units; byte arguments when they fit."""
    out = bytearray(struct.pack('>hhhhh', -1, *bb))
    for k, (gid, dx, dy, sx, sy) in enumerate(comps):
        fl = 2 | (0x20 if k + 1 < len(comps) else 0)
        words = not (-128 <= dx <= 127 and -128 <= dy <= 127)
        if words: fl |= 1
        if sx == sy and sx != 1.0: fl |= 8
        elif sx != sy: fl |= 0x40
        out += struct.pack('>HH', fl, gid)
        out += struct.pack('>hh' if words else '>bb', dx, dy)
        if fl & 8: out += struct.pack('>h', f2dot14(sx))
        elif fl & 0x40: out += struct.pack('>hh', f2dot14(sx), f2dot14(sy))
    return bytes(out)


# ---------------------------------------------------------------- cmap

def cmap_groups(pairs):
    """pairs: sorted (cp, gid). One group per run where both advance by one."""
    groups = []
    for c, g in pairs:
        if groups and c == groups[-1][1] + 1 and g == groups[-1][2] + (c - groups[-1][0]):
            groups[-1][1] = c
        else:
            groups.append([c, c, g])
    return groups


def build_cmap(pairs):
    pairs = sorted(pairs)
    groups = cmap_groups(pairs)
    # format 12
    f12 = struct.pack('>HHIII', 12, 0, 16 + 12 * len(groups), 0, len(groups)) + b''.join(struct.pack('>III', *g) for g in groups)
    # format 4 for the BMP part
    bmp = [g for g in groups if g[1] <= 0xFFFF]
    segs = [(s, e, (gid - s) & 0xFFFF) for s, e, gid in bmp] + [(0xFFFF, 0xFFFF, 1)]
    n = len(segs)
    sr = 2 * (1 << (n.bit_length() - 1)); es = (n.bit_length() - 1)
    f4 = struct.pack('>HHHHHHH', 4, 16 + 8 * n, 0, 2 * n, sr, es, 2 * n - sr)
    f4 += b''.join(struct.pack('>H', s[1]) for s in segs) + b'\0\0'
    f4 += b''.join(struct.pack('>H', s[0]) for s in segs)
    f4 += b''.join(struct.pack('>H', s[2]) for s in segs)
    f4 += b'\0\0' * n
    tab = struct.pack('>HH', 0, 2)
    off4 = 4 + 16
    off12 = off4 + len(f4)
    tab += struct.pack('>HHI', 3, 1, off4) + struct.pack('>HHI', 3, 10, off12)
    return tab + f4 + f12, len(groups)


def build_cmap13(ranges, gid=1):
    """cmap with one (3,10) format 13 subtable (many to one): every code point of the ranges maps to glyph `gid`."""
    groups = b''.join(struct.pack('>III', a, b, gid) for a, b in ranges)
    sub = struct.pack('>HHIII', 13, 0, 16 + 12 * len(ranges), 0, len(ranges)) + groups
    return struct.pack('>HH', 0, 1) + struct.pack('>HHI', 3, 10, 12) + sub


# ---------------------------------------------------------------- GPOS

def build_gpos(adjust, features=(b'dist', b'kern', b'locl', b'mark', b'rlig'), scripts=(b'DFLT',)):
    """GPOS with one SinglePos format 2 lookup giving XAdvance. adjust: {gid: xadvance}."""
    gids = sorted(adjust)
    # coverage format 2 (ranges)
    rng = []
    for i, g in enumerate(gids):
        if rng and g == rng[-1][1] + 1: rng[-1][1] = g
        else: rng.append([g, g, i])
    cov = struct.pack('>HH', 2, len(rng)) + b''.join(struct.pack('>HHH', *r) for r in rng)
    sub = struct.pack('>HHHH', 2, 8 + 2 * len(gids), 4, len(gids)) + b''.join(struct.pack('>h', adjust[g]) for g in gids) + cov
    lookup = struct.pack('>HHHH', 1, 0, 1, 8) + sub
    lookup_list = struct.pack('>HH', 1, 4) + lookup
    nf = len(features)
    feat = [struct.pack('>HHH', 0, 1, 0) for _ in features]
    fl = struct.pack('>H', nf)
    base = 2 + 6 * nf
    for t, f in zip(features, feat):
        fl += t + struct.pack('>H', base); base += len(f)
    fl += b''.join(feat)
    langsys = struct.pack('>HHH', 0, 0xFFFF, nf) + b''.join(struct.pack('>H', i) for i in range(nf))
    script = struct.pack('>HH', 4, 0) + langsys
    sl = struct.pack('>H', len(scripts))
    for i, tag in enumerate(scripts):
        sl += tag + struct.pack('>H', 2 + 6 * len(scripts) + i * len(script))
    sl += script * len(scripts)
    head = struct.pack('>HHHHH', 1, 0, 10, 10 + len(sl), 10 + len(sl) + len(fl))
    return head + sl + fl + lookup_list


# ---------------------------------------------------------------- tables, sfnt, wrappers

def name_table(family):
    recs = [(1, family), (2, 'Regular'), (3, family + ' generated'), (4, family), (5, 'Version 1.0'), (6, family)]
    strs = b''; out = []
    for nid, s in recs:
        b = s.encode('utf-16-be')
        out.append(struct.pack('>HHHHHH', 3, 1, 0x409, nid, len(b), len(strs))); strs += b
    return struct.pack('>HHH', 0, len(recs), 6 + 12 * len(recs)) + b''.join(out) + strs


def checksum(b):
    b = b + b'\0' * (-len(b) % 4)
    return sum(struct.unpack('>%dI' % (len(b) // 4), b)) & 0xFFFFFFFF


def build_sfnt(tables):
    """tables: {tag: bytes}; head.checkSumAdjustment is filled in."""
    tags = sorted(tables)
    n = len(tags)
    sr = 16 * (1 << (n.bit_length() - 1)); es = n.bit_length() - 1
    off = 12 + 16 * n
    dirent = b''; body = b''
    t = dict(tables)
    for tag in tags:
        b = t[tag]
        dirent += struct.pack('>4sIII', tag.encode(), checksum(b), off + len(body), len(b))
        body += b + b'\0' * (-len(b) % 4)
    hdr = struct.pack('>IHHHH', 0x10000, n, sr, es, 16 * n - sr)
    font = bytearray(hdr + dirent + body)
    # checkSumAdjustment
    ho = 12 + 16 * n
    for i, tag in enumerate(tags):
        if tag == 'head':
            hoff = struct.unpack_from('>I', font, 12 + 16 * i + 8)[0]
    adj = (0xB1B0AFBA - checksum(bytes(font))) & 0xFFFFFFFF
    struct.pack_into('>I', font, hoff + 8, adj)
    return bytes(font)


def read_tables(data):
    n = u16(data, 4)
    out = {}
    for i in range(n):
        tag, _c, off, ln = struct.unpack_from('>4sIII', data, 12 + 16 * i)
        out[tag.decode()] = data[off:off + ln]
    return out


def to_woff1(data, level=9):
    tabs = read_tables(data)
    n = len(tabs)
    off = 44 + 20 * n
    ent = b''; body = b''
    for tag in sorted(tabs):
        raw = tabs[tag]
        z = zlib.compress(raw, level)
        if len(z) >= len(raw): z = raw
        ent += struct.pack('>4sIIII', tag.encode(), off + len(body), len(z), len(raw), checksum(raw))
        body += z + b'\0' * (-len(z) % 4)
    total = 44 + 20 * n + len(body)
    hdr = struct.pack('>4sIIHHIHHIIIII', b'wOFF', 0x10000, total, n, 0, len(data), 1, 0, 0, 0, 0, 0, 0)
    return hdr + ent + body


def _base128(v):
    out = [v & 0x7F]; v >>= 7
    while v:
        out.append(0x80 | (v & 0x7F)); v >>= 7
    return bytes(reversed(out))


def _255u16(v):
    if v < 253: return bytes([v])
    if v < 506: return bytes([255, v - 253])
    if v < 762: return bytes([254, v - 506])
    return bytes([253, v >> 8, v & 255])


def _triplet(dx, dy, on):
    """WOFF2 5.2 triplet encoding of one point: (flag byte, data bytes)."""
    ax, ay = abs(dx), abs(dy)
    sx = 1 if dx >= 0 else 0; sy = 1 if dy >= 0 else 0
    off = 0 if on else 0x80
    if dx == 0 and ay < 1280:
        return off | (((ay >> 8) << 1) + sy), bytes([ay & 255])
    if dy == 0 and ax < 1280:
        return off | (10 + ((ax >> 8) << 1) + sx), bytes([ax & 255])
    if 1 <= ax <= 64 and 1 <= ay <= 64:
        return off | (20 + ((ax - 1) & 0x30) + (((ay - 1) & 0x30) >> 2) + sx + 2 * sy), bytes([(((ax - 1) & 15) << 4) | ((ay - 1) & 15)])
    if 1 <= ax <= 768 and 1 <= ay <= 768:
        return off | (84 + 12 * ((ax - 1) >> 8) + 4 * ((ay - 1) >> 8) + sx + 2 * sy), bytes([(ax - 1) & 255, (ay - 1) & 255])
    if ax < 4096 and ay < 4096:
        return off | (120 + sx + 2 * sy), bytes([ax >> 4, ((ax & 15) << 4) | (ay >> 8), ay & 255])
    return off | (124 + sx + 2 * sy), struct.pack('>HH', ax, ay)


def _parse_glyph_points(d, o, nc):
    ends = [u16(d, o + 10 + 2 * k) for k in range(nc)]
    il = u16(d, o + 10 + 2 * nc)
    instr = d[o + 12 + 2 * nc:o + 12 + 2 * nc + il]
    p = o + 12 + 2 * nc + il
    n = ends[-1] + 1
    flags = []
    while len(flags) < n:
        f = d[p]; p += 1; flags.append(f)
        if f & 8:
            r = d[p]; p += 1; flags += [f] * r
    xs = []; x = 0
    for f in flags:
        if f & 2:
            dx = d[p]; p += 1; x += dx if f & 16 else -dx
        elif not f & 16:
            x += i16(d, p); p += 2
        xs.append(x)
    ys = []; y = 0
    for f in flags:
        if f & 4:
            dy = d[p]; p += 1; y += dy if f & 32 else -dy
        elif not f & 32:
            y += i16(d, p); p += 2
        ys.append(y)
    return ends, instr, [(xs[k], ys[k], bool(flags[k] & 1)) for k in range(n)]


def woff2_transform_glyf(glyf, loca, num_glyphs, index_format):
    """The WOFF2 transformed glyf table (and its xMin list, for the hmtx transform) of a TrueType glyf/loca pair."""
    ncs = bytearray(); npts = bytearray(); flg = bytearray(); gs = bytearray(); cs = bytearray(); bbs = bytearray(); ins = bytearray()
    bitmap = bytearray(4 * ((num_glyphs + 31) >> 5)); xmins = []
    for g in range(num_glyphs):
        a, b = loca[g], loca[g + 1]
        if b == a:
            ncs += struct.pack('>h', 0); xmins.append(0); continue
        nc = i16(glyf, a)
        bb = struct.unpack_from('>hhhh', glyf, a + 2)
        ncs += struct.pack('>h', nc)
        if nc >= 0:
            ends, instr, pts = _parse_glyph_points(glyf, a, nc)
            s0 = 0
            for e in ends:
                npts += _255u16(e + 1 - s0); s0 = e + 1
            px = py = 0
            for x, y, on in pts:
                fl, data = _triplet(x - px, y - py, on); px, py = x, y
                flg.append(fl); gs += data
            gs += _255u16(len(instr)); ins += instr
            xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
            if bb != (min(xs), min(ys), max(xs), max(ys)):
                bitmap[g >> 3] |= 0x80 >> (g & 7); bbs += struct.pack('>hhhh', *bb)
        else:
            bitmap[g >> 3] |= 0x80 >> (g & 7); bbs += struct.pack('>hhhh', *bb)
            p = a + 10; have_instr = False
            while True:
                fl = u16(glyf, p); gi = u16(glyf, p + 2)
                ln = 4 + (4 if fl & 1 else 2) + (2 if fl & 8 else 4 if fl & 0x40 else 8 if fl & 0x80 else 0)
                cs += glyf[p:p + ln]; p += ln
                if fl & 0x100: have_instr = True
                if not fl & 0x20: break
            if have_instr:
                il = u16(glyf, p); gs += _255u16(il); ins += glyf[p + 2:p + 2 + il]
        xmins.append(bb[0])
    hdr = struct.pack('>HHHHIIIIIII', 0, 0, num_glyphs, index_format, len(ncs), len(npts), len(flg), len(gs), len(cs), len(bitmap) + len(bbs), len(ins))
    return hdr + bytes(ncs) + bytes(npts) + bytes(flg) + bytes(gs) + bytes(cs) + bytes(bitmap) + bytes(bbs) + bytes(ins), xmins


def woff2_transform_hmtx(hmtx, num_hmetrics, num_glyphs, xmins):
    """Transform version 1: drop the lsb arrays when they equal the glyphs' xMin; None when they do not."""
    adv = [u16(hmtx, 4 * i) for i in range(num_hmetrics)]
    lsb = [i16(hmtx, 4 * i + 2) for i in range(num_hmetrics)] + [i16(hmtx, 4 * num_hmetrics + 2 * k) for k in range(num_glyphs - num_hmetrics)]
    flags = 0
    if all(lsb[i] == xmins[i] for i in range(num_hmetrics)): flags |= 1
    if all(lsb[i] == xmins[i] for i in range(num_hmetrics, num_glyphs)): flags |= 2
    out = bytes([flags]) + b''.join(struct.pack('>H', a) for a in adv)
    if not flags & 1: out += b''.join(struct.pack('>h', l) for l in lsb[:num_hmetrics])
    if not flags & 2: out += b''.join(struct.pack('>h', l) for l in lsb[num_hmetrics:])
    return out, flags


WOFF2_KNOWN = ['cmap', 'head', 'hhea', 'hmtx', 'maxp', 'name', 'OS/2', 'post', 'cvt ', 'fpgm', 'glyf', 'loca', 'prep',
               'CFF ', 'VORG', 'EBDT', 'EBLC', 'gasp', 'hdmx', 'kern', 'LTSH', 'PCLT', 'VDMX', 'vhea', 'vmtx', 'BASE',
               'GDEF', 'GPOS', 'GSUB', 'EBSC', 'JSTF', 'MATH', 'CBDT', 'CBLC', 'COLR', 'CPAL', 'SVG ', 'sbix', 'acnt',
               'avar', 'bdat', 'bloc', 'bsln', 'cvar', 'fdsc', 'feat', 'fmtx', 'fvar', 'gvar', 'hsty', 'just', 'lcar',
               'mort', 'morx', 'opbd', 'prop', 'trak', 'Zapf', 'Silf', 'Glat', 'Gloc', 'Feat', 'Sill']


def to_woff2(data, brotli_compress, transform=True):
    """WOFF2: table directory with known-tag flags and UIntBase128 lengths, one Brotli stream over the table data.
    transform=False keeps glyf/loca/hmtx as they are (transform version 3 for glyf/loca); True writes the transformed
    glyf (split streams, triplets), an empty loca, and the hmtx without the lsb array when it equals xMin."""
    tabs = read_tables(data)
    order = sorted(tabs)   # tag order, as the reference encoder: a transformed hmtx needs glyf before it
    ng = u16(tabs['maxp'], 4)
    fmt = i16(tabs['head'], 50)
    payload = {}; ent_flags = {}
    for t in order: payload[t] = tabs[t]; ent_flags[t] = (0, None)
    if transform:
        loca = [u32(tabs['loca'], 4 * i) for i in range(ng + 1)] if fmt else [2 * u16(tabs['loca'], 2 * i) for i in range(ng + 1)]
        g, xmins = woff2_transform_glyf(tabs['glyf'], loca, ng, fmt)
        payload['glyf'] = g; ent_flags['glyf'] = (0, len(g))
        payload['loca'] = b''; ent_flags['loca'] = (0, 0)
        if 'hmtx' in tabs:
            nh = u16(tabs['hhea'], 34)
            h, fl = woff2_transform_hmtx(tabs['hmtx'], nh, ng, xmins)
            if fl == 3:
                payload['hmtx'] = h; ent_flags['hmtx'] = (1, len(h))
    else:
        ent_flags['glyf'] = (3, None); ent_flags['loca'] = (3, None)
    ent = b''; stream = b''
    for tag in order:
        raw = tabs[tag]
        idx = WOFF2_KNOWN.index(tag) if tag in WOFF2_KNOWN else 63
        ver, tlen = ent_flags[tag]
        ent += bytes([idx | (ver << 6)]) + (b'' if idx != 63 else tag.encode()) + _base128(len(raw))
        if tlen is not None: ent += _base128(tlen)
        stream += payload[tag]
    comp = brotli_compress(stream)
    total_sfnt = 12 + 16 * len(tabs) + sum(len(t) + (-len(t) % 4) for t in tabs.values())
    body = ent + comp
    length = 48 + len(body)
    pad = -length % 4
    hdr = struct.pack('>4sIIHHIIHHIIIII', b'wOF2', 0x10000, length + pad, len(tabs), 0, total_sfnt, len(comp), 1, 0, 0, 0, 0, 0, 0)
    return hdr + body + b'\0' * pad
