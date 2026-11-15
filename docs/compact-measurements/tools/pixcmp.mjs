// node pixcmp.mjs OUTDIR REF NAME DPR : per-formula differing pixel counts between two sets of grid screenshots
import fs from 'node:fs'; import zlib from 'node:zlib';
const [out, ref, name, dprS] = process.argv.slice(2); const dpr = +dprS;
const F = fs.readFileSync('/home/agents/scratch/katex-harden/data/formulas.jsonl', 'utf8').trim().split('\n').map(JSON.parse);
const ROW = 200, PER = 25;
function png(path) {
  const d = fs.readFileSync(path); let i = 8, w, h, ct; const idat = [];
  while (i < d.length) {
    const n = d.readUInt32BE(i), t = d.toString('latin1', i + 4, i + 8);
    if (t === 'IHDR') { w = d.readUInt32BE(i + 8); h = d.readUInt32BE(i + 12); ct = d[i + 17]; }
    if (t === 'IDAT') idat.push(d.subarray(i + 8, i + 8 + n));
    i += 12 + n;
  }
  const raw = zlib.inflateSync(Buffer.concat(idat)); const bpp = ct === 6 ? 4 : 3, stride = w * bpp;
  const img = new Uint8Array(h * stride); let pos = 0;
  for (let y = 0; y < h; y++) {
    const f = raw[pos++]; const o = y * stride, p = o - stride;
    for (let k = 0; k < stride; k++) {
      const x = raw[pos + k]; const a = k >= bpp ? img[o + k - bpp] : 0, b = y ? img[p + k] : 0, c = (k >= bpp && y) ? img[p + k - bpp] : 0;
      let v;
      switch (f) { case 0: v = x; break; case 1: v = x + a; break; case 2: v = x + b; break; case 3: v = x + ((a + b) >> 1); break;
        default: { const pp = a + b - c, pa = Math.abs(pp - a), pb = Math.abs(pp - b), pc = Math.abs(pp - c); v = x + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c); } }
      img[o + k] = v & 255;
    }
    pos += stride;
  }
  return { w, h, bpp, img };
}
const res = {}; let bad = 0, total = 0;
const MAXP = +(process.env.MAXP || 999);
for (let p = 0; p * PER < F.length && p < MAXP; p++) {
  const pad = String(p).padStart(3, '0');
  const A = png(`${out}/${ref}_dpr${dpr}_${pad}.png`), B = png(`${out}/${name}_dpr${dpr}_${pad}.png`);
  if (A.w !== B.w || A.h !== B.h) { for (const f of F.slice(p * PER, (p + 1) * PER)) res[f.id] = -1; continue; }
  F.slice(p * PER, (p + 1) * PER).forEach((f, k) => {
    let n = 0; const s = A.w * A.bpp; const y0 = k * ROW * dpr, y1 = Math.min((k + 1) * ROW * dpr, A.h);
    for (let y = y0; y < y1; y++) for (let x = 0; x < A.w; x++) { const o = y * s + x * A.bpp; if (A.img[o] !== B.img[o] || A.img[o + 1] !== B.img[o + 1] || A.img[o + 2] !== B.img[o + 2]) n++; }
    res[f.id] = n;
  });
}
const ids = Object.keys(res).filter((i) => res[i]);
console.log(`${name} vs ${ref} dpr ${dpr}: differing formulas ${ids.length} / ${Object.keys(res).length}, differing pixels ${Object.values(res).reduce((a, b) => a + Math.max(b, 0), 0)}`);
if (ids.length) console.log('ids', ids.slice(0, 40).join(' '));
fs.writeFileSync(`${out}/diff_${name}_${ref}_dpr${dpr}.json`, JSON.stringify(res));
