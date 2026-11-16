// node cmp.mjs K.png F.png OUT.png dpr rowh rows(comma) x0 w [scale] [yfrac0 yfrac1] : stacked crops of KaTeX, font, and diff (red) per row
import fs from 'node:fs'; import zlib from 'node:zlib';
function png(path) {
  const d = fs.readFileSync(path); let i = 8, w, h, ct; const idat = [];
  while (i < d.length) { const n = d.readUInt32BE(i), t = d.toString('latin1', i + 4, i + 8);
    if (t === 'IHDR') { w = d.readUInt32BE(i + 8); h = d.readUInt32BE(i + 12); ct = d[i + 17]; }
    if (t === 'IDAT') idat.push(d.subarray(i + 8, i + 8 + n)); i += 12 + n; }
  const raw = zlib.inflateSync(Buffer.concat(idat)); const bpp = ct === 6 ? 4 : 3, stride = w * bpp;
  const img = new Uint8Array(h * stride); let pos = 0;
  for (let y = 0; y < h; y++) { const f = raw[pos++]; const o = y * stride, p = o - stride;
    for (let k = 0; k < stride; k++) { const x = raw[pos + k]; const a = k >= bpp ? img[o + k - bpp] : 0, b = y ? img[p + k] : 0, c = (k >= bpp && y) ? img[p + k - bpp] : 0; let v;
      switch (f) { case 0: v = x; break; case 1: v = x + a; break; case 2: v = x + b; break; case 3: v = x + ((a + b) >> 1); break;
        default: { const pp = a + b - c, pa = Math.abs(pp - a), pb = Math.abs(pp - b), pc = Math.abs(pp - c); v = x + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c); } }
      img[o + k] = v & 255; } pos += stride; }
  return { w, h, bpp, img };
}
function crc(buf) { let c, crc = 0xffffffff; for (let n = 0; n < buf.length; n++) { c = (crc ^ buf[n]) & 0xff; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; crc = (crc >>> 8) ^ c; } return (crc ^ 0xffffffff) >>> 0; }
function writePng(path, w, h, rgb) {
  const raw = Buffer.alloc(h * (w * 3 + 1)); for (let y = 0; y < h; y++) { raw[y * (w * 3 + 1)] = 0; Buffer.from(rgb.buffer, y * w * 3, w * 3).copy(raw, y * (w * 3 + 1) + 1); }
  const ch = (t, d) => { const b = Buffer.concat([Buffer.from(t), d]); const o = Buffer.alloc(4); o.writeUInt32BE(b.length - 4); const c = Buffer.alloc(4); c.writeUInt32BE(crc(b)); return Buffer.concat([o, b, c]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  fs.writeFileSync(path, Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), ch('IHDR', ihdr), ch('IDAT', zlib.deflateSync(raw)), ch('IEND', Buffer.alloc(0))]));
}
const [kp, fp, out, dprS, rowhS, rowsS, x0S, wS, scS, y0S, y1S] = process.argv.slice(2);
const dpr = +dprS, rowh = +rowhS, rows = rowsS.split(',').map(Number), x0 = +x0S, w = +wS, sc = +(scS || 2), yf0 = +(y0S || 0.25), yf1 = +(y1S || 0.75);
const FOFF = +(process.env.FOFF || 0); const K = png(kp), F = png(fp); const rh = Math.round(rowh * dpr * (yf1 - yf0)), W = w * dpr * sc, H = rows.length * 3 * rh * sc;
const rgb = new Uint8Array(W * H * 3).fill(255); let oy = 0;
const px = (I, x, y) => { const o = (y * I.w + x) * I.bpp; return [I.img[o], I.img[o + 1], I.img[o + 2]]; };
for (const r of rows) for (const which of [0, 1, 2]) {
  for (let y = 0; y < rh; y++) for (let x = 0; x < w * dpr; x++) {
    const sy = Math.round(r * rowh * dpr + rowh * dpr * yf0) + y, sx = x0 * dpr + x; const a = px(K, sx, sy), b = px(F, sx, sy + FOFF * rowh * dpr);
    const col = which === 0 ? a : which === 1 ? b : (a[0] !== b[0] || a[1] !== b[1] || a[2] !== b[2]) ? [255, 0, 0] : [255, 255, 255];
    for (let j = 0; j < sc; j++) for (let i = 0; i < sc; i++) { const o = ((oy + y * sc + j) * W + x * sc + i) * 3; rgb[o] = col[0]; rgb[o + 1] = col[1]; rgb[o + 2] = col[2]; }
  }
  oy += rh * sc;
}
writePng(out, W, H, rgb);
