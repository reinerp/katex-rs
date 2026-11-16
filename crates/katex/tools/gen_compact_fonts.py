#!/usr/bin/env python3
"""Write the fonts of the compact output that KaTeX does not have: a TrueType font with empty glyphs
of exact advances for some Unicode space code points (the spaces between atoms).

usage: gen_compact_fonts.py OUTDIR
Standard library only.
"""
import struct, sys, time

def checksum(b):
    b += b'\0' * (-len(b) % 4)
    return sum(struct.unpack('>%dI' % (len(b) // 4), b)) & 0xffffffff

def build(spaces, upem=10000, name='KaTeX_Space'):
    cps = sorted(spaces)
    n = len(cps) + 1                       # glyph 0 is .notdef
    adv = [0] + [spaces[c] for c in cps]
    t = {}
    t['head'] = struct.pack('>HHHHIIHHqqhhhhHHhhh', 1, 0, 0, 0, 0, 0x5F0F3CF5, 0x000b, upem, 0, 0, 0, 0, 0, 0, 0, 8, 2, 0, 0)
    # ^ checkSumAdjustment patched below
    t['hhea'] = struct.pack('>HHhhhHhhhhhhhhhhhH', 1, 0, 800, -200, 0, max(adv), 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, n)
    t['hmtx'] = b''.join(struct.pack('>Hh', a, 0) for a in adv)
    t['maxp'] = struct.pack('>IHHHHHHHHHHHHHH', 0x00010000, n, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0)
    t['loca'] = struct.pack('>%dH' % (n + 1), *([0] * (n + 1)))
    t['glyf'] = b'\0\0\0\0'
    # cmap format 4, one segment per code point
    segs = [(c, c, (i + 1 - c) & 0xffff) for i, c in enumerate(cps)] + [(0xffff, 0xffff, 1)]
    sc = len(segs)
    sr = 2 * (1 << (sc.bit_length() - 1)); es = (sc.bit_length() - 1); rs = 2 * sc - sr
    f4 = struct.pack('>HHHHHHH', 4, 16 + 8 * sc, 0, 2 * sc, sr, es, rs)
    f4 += b''.join(struct.pack('>H', e) for _, e, _ in segs) + b'\0\0'
    f4 += b''.join(struct.pack('>H', s) for s, _, _ in segs)
    f4 += b''.join(struct.pack('>H', d) for _, _, d in segs)
    f4 += b'\0\0' * sc
    t['cmap'] = struct.pack('>HH', 0, 1) + struct.pack('>HHI', 3, 1, 12) + f4
    nm = name.encode('utf-16-be')
    recs = [(3, 1, 0x409, 1, nm), (3, 1, 0x409, 2, 'Regular'.encode('utf-16-be')), (3, 1, 0x409, 4, nm), (3, 1, 0x409, 6, nm)]
    off = 0; body = b''; head = b''
    for p, e, l, i, s in recs:
        head += struct.pack('>HHHHHH', p, e, l, i, len(s), off); body += s; off += len(s)
    t['name'] = struct.pack('>HHH', 0, len(recs), 6 + 12 * len(recs)) + head + body
    # OS/2 version 3 (96 bytes), written field by field
    o = struct.pack('>HhHHHhhhhhhhhhhhh', 3, 0, 400, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
    o += b'\0' * 10                            # panose
    o += struct.pack('>IIII', 0, 0, 0, 0)       # unicode ranges
    o += b'    '                                # vendor
    o += struct.pack('>HHHhhhHH', 0x40, 0x20, 0x2060, 800, -200, 0, 800, 200)
    o += struct.pack('>II', 1, 0)               # code page ranges
    o += struct.pack('>hhHHH', 0, 0, 0, 0, 0xffff)
    t['OS/2'] = o
    t['post'] = struct.pack('>IIhhIIIII', 0x00030000, 0, -100, 50, 0, 0, 0, 0, 0)
    order = sorted(t)
    out = struct.pack('>IHHHH', 0x00010000, len(order), *(lambda e: (16 * (1 << e), e, 16 * len(order) - 16 * (1 << e)))(len(order).bit_length() - 1))
    offset = 12 + 16 * len(order)
    dirs = b''; blobs = b''
    for tag in order:
        d = t[tag]
        dirs += struct.pack('>4sIII', tag.encode(), checksum(d), offset + len(blobs), len(d))
        blobs += d + b'\0' * (-len(d) % 4)
    font = bytearray(out + dirs + blobs)
    adj = (0xB1B0AFBA - checksum(bytes(font))) & 0xffffffff
    # checkSumAdjustment is 4 bytes before the magic
    m = font.index(struct.pack('>I', 0x5F0F3CF5))
    font[m - 4:m] = struct.pack('>I', adj)
    return bytes(font)

SPACES = {
    0x2002: 5000,    # half an em
    0x2003: 10000,   # \quad
    0x2004: 2778,    # thick space, 5mu
    0x205f: 2222,    # medium space, 4mu
    0x2006: 1667,    # thin space, 3mu
    0x200a: 556,     # 1mu
}

if __name__ == '__main__':
    import os
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    open(os.path.join(out, 'KaTeX_Compact-Space.ttf'), 'wb').write(build(SPACES))
