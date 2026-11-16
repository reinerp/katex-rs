# woff2dec.py FILE.woff2 [REF.ttf] : decode a WOFF2 (brotli via the brotli CLI), parse the transformed glyf and compare outlines with REF
import sys, struct, subprocess, os
sys.path.insert(0, '/home/agents/workspaces/katex-rs-font/crates/katex/tools')
import sfnt
def base128(d, p):
    v = 0
    for i in range(5):
        b = d[p]; p += 1; v = (v << 7) | (b & 0x7f)
        if not b & 0x80: return v, p
def r255(d, p):
    c = d[p]; p += 1
    if c == 253: return (d[p] << 8) | d[p + 1], p + 2
    if c == 255: return 253 + d[p], p + 1
    if c == 254: return 506 + d[p], p + 1
    return c, p
def sgn(flag, v): return v if flag & 1 else -v
def triplet(flag, d, p):
    if flag < 10: return 0, sgn(flag, ((flag & 14) << 7) + d[p]), p + 1
    if flag < 20: return sgn(flag, (((flag - 10) & 14) << 7) + d[p]), 0, p + 1
    if flag < 84:
        b0 = flag - 20; b1 = d[p]
        return sgn(flag, 1 + (b0 & 0x30) + (b1 >> 4)), sgn(flag >> 1, 1 + ((b0 & 0x0c) << 2) + (b1 & 0x0f)), p + 1
    if flag < 120:
        b0 = flag - 84
        return sgn(flag, 1 + ((b0 // 12) << 8) + d[p]), sgn(flag >> 1, 1 + (((b0 % 12) >> 2) << 8) + d[p + 1]), p + 2
    if flag < 124:
        b2 = d[p + 1]
        return sgn(flag, (d[p] << 4) + (b2 >> 4)), sgn(flag >> 1, ((b2 & 0x0f) << 8) + d[p + 2]), p + 3
    return sgn(flag, (d[p] << 8) + d[p + 1]), sgn(flag >> 1, (d[p + 2] << 8) + d[p + 3]), p + 4
def decode(path):
    d = open(path, 'rb').read()
    sig, flavor, length, nt, _r, tsz, csz = struct.unpack_from('>4sIIHHII', d, 0)
    p = 48; ents = []
    for i in range(nt):
        fl = d[p]; p += 1
        idx = fl & 63; ver = fl >> 6
        if idx == 63: tag = d[p:p + 4].decode(); p += 4
        else: tag = sfnt.WOFF2_KNOWN[idx]
        ol, p = base128(d, p)
        tl = None
        if (tag in ('glyf', 'loca') and ver == 0) or (tag == 'hmtx' and ver == 1): tl, p = base128(d, p)
        ents.append((tag, ver, ol, tl))
    comp = d[p:p + csz]
    raw = subprocess.run(['/home/agents/scratch/katex-font/brotli', '-d', '-c'], input=comp, stdout=subprocess.PIPE, check=True).stdout
    tabs = {}; q = 0
    for tag, ver, ol, tl in ents:
        n = tl if tl is not None else ol
        tabs[tag] = (ver, ol, tl, raw[q:q + n]); q += n
    return d, ents, tabs, (tsz, length)
def glyphs(g):
    _, _, ng, fmt, n1, n2, n3, n4, n5, n6, n7 = struct.unpack_from('>HHHHIIIIIII', g, 0)
    o = 36; ncs = g[o:o + n1]; o += n1; npts = g[o:o + n2]; o += n2; flg = g[o:o + n3]; o += n3; gs = g[o:o + n4]; o += n4; cs = g[o:o + n5]; o += n5
    bb = g[o:o + n6]; o += n6; ins = g[o:o + n7]
    bm = 4 * ((ng + 31) >> 5); bitmap = bb[:bm]; bp = bm
    out = []; pn = fp = gp = cp = ip = 0
    for i in range(ng):
        nc = struct.unpack_from('>h', ncs, 2 * i)[0]
        if nc == 0: out.append([]); continue
        if nc > 0:
            counts = []
            for _ in range(nc):
                c, pn = r255(npts, pn); counts.append(c)
            pts = []; x = y = 0
            for c in counts:
                cont = []
                for _ in range(c):
                    f = flg[fp]; fp += 1
                    dx, dy, gp = triplet(f & 0x7f, gs, gp); x += dx; y += dy
                    cont.append((x, y, not f & 0x80))
                pts.append(cont)
            il, gp = r255(gs, gp); ip += il
            if bitmap[i >> 3] & (0x80 >> (i & 7)): bp += 8
            out.append(pts)
        else:
            out.append('composite')
            bp += 8
            while True:
                fl = struct.unpack_from('>H', cs, cp)[0]
                ln = 4 + (4 if fl & 1 else 2) + (2 if fl & 8 else 4 if fl & 0x40 else 8 if fl & 0x80 else 0); cp += ln
                if not fl & 0x20: break
    assert pn == len(npts) and fp == len(flg) and gp == len(gs) and cp == len(cs) and bp == len(bb) and ip == len(ins), (pn, len(npts), fp, len(flg), gp, len(gs), cp, len(cs), bp, len(bb), ip, len(ins))
    return out
if __name__ == '__main__':
    d, ents, tabs, (tsz, ln) = decode(sys.argv[1])
    print('tables', [(t, v, ol, tl) for t, v, ol, tl in ents], 'totalSfnt', struct.unpack_from('>I', d, 16)[0], 'file', ln)
    g = glyphs(tabs['glyf'][3])
    print('glyphs parsed', len(g))
    if len(sys.argv) > 2:
        f = sfnt.SourceFont(sys.argv[2]); bad = 0
        for i, pts in enumerate(g):
            if pts == 'composite': continue
            ref = f.contours(i)
            if [list(c) for c in pts] != [list(c) for c in ref]: bad += 1
        print('outline mismatches vs', sys.argv[2], bad)
