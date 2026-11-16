// node pngdiff.mjs A.png B.png ROWH [dpr] : differing pixels per band of ROWH css px, JSON on stdout
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
// node pngshift.mjs K.png F.png ROWH DPR : per row band, the differing pixels at vertical offsets -2..2 device pixels of the second image: [[n(-2)..n(+2)]...]
const [pa, pb, rowh, dprS] = process.argv.slice(2); const dpr = +(dprS || 1); const A = png(pa), B = png(pb);
const s = A.w * A.bpp; const RH = +rowh * dpr; const bands = [];
for (let y0 = 0; y0 < A.h; y0 += RH) { const y1 = Math.min(y0 + RH, A.h); const res = [];
  for (let dy = -2; dy <= 2; dy++) { let n = 0;
    for (let y = y0 + 2; y < y1 - 2; y++) for (let x = 0; x < A.w; x++) { const o = y * s + x * A.bpp, p = (y + dy) * s + x * A.bpp;
      if (Math.max(Math.abs(A.img[o] - B.img[p]), Math.abs(A.img[o+1] - B.img[p+1]), Math.abs(A.img[o+2] - B.img[p+2])) > 0) n++; }
    res.push(n); }
  bands.push(res); }
console.log(JSON.stringify(bands));
