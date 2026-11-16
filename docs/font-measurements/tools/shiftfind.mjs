// node shiftfind.mjs K.png F.png dpr rowh row x0 w : best integer device-pixel shift of the hybrid row against KaTeX's (sum of abs diff)
import fs from 'node:fs'; import zlib from 'node:zlib';
function png(path) { const d = fs.readFileSync(path); let i = 8, w, h, ct; const idat = [];
  while (i < d.length) { const n = d.readUInt32BE(i), t = d.toString('latin1', i + 4, i + 8); if (t === 'IHDR') { w = d.readUInt32BE(i + 8); h = d.readUInt32BE(i + 12); ct = d[i + 17]; } if (t === 'IDAT') idat.push(d.subarray(i + 8, i + 8 + n)); i += 12 + n; }
  const raw = zlib.inflateSync(Buffer.concat(idat)); const bpp = ct === 6 ? 4 : 3, stride = w * bpp; const img = new Uint8Array(h * stride); let pos = 0;
  for (let y = 0; y < h; y++) { const f = raw[pos++]; const o = y * stride, p = o - stride; for (let k = 0; k < stride; k++) { const x = raw[pos + k]; const a = k >= bpp ? img[o + k - bpp] : 0, b = y ? img[p + k] : 0, c = (k >= bpp && y) ? img[p + k - bpp] : 0; let v;
    switch (f) { case 0: v = x; break; case 1: v = x + a; break; case 2: v = x + b; break; case 3: v = x + ((a + b) >> 1); break; default: { const pp = a + b - c, pa = Math.abs(pp - a), pb = Math.abs(pp - b), pc = Math.abs(pp - c); v = x + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c); } } img[o + k] = v & 255; } pos += stride; }
  return { w, h, bpp, img }; }
const [kp, fp, dprS, rowhS, rowS, x0S, wS] = process.argv.slice(2); const dpr = +dprS, rh = +rowhS * dpr, row = +rowS, x0 = +x0S * dpr, w = +wS * dpr;
const K = png(kp), F = png(fp); const g = (I, x, y) => I.img[(y * I.w + x) * I.bpp];
let best = null;
for (let dy = -4; dy <= 4; dy++) for (let dx = -4; dx <= 4; dx++) { let s = 0;
  for (let y = row * rh + 4; y < (row + 1) * rh - 4; y++) for (let x = x0 + 4; x < x0 + w - 4; x++) s += Math.abs(g(K, x, y) - g(F, x + dx, y + dy));
  if (!best || s < best.s) best = { s, dx, dy }; if (dx === 0 && dy === 0) console.log('no shift: ' + s); }
console.log('best shift (device px) of hybrid: dx', best.dx, 'dy', best.dy, 'sum', best.s);
