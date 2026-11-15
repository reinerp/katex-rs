// node crop.mjs OUT.png DPR SCALE FORMULA_ID NAME... : stack the formula's row from each NAME's grid screenshot (png/NAME_dprD_PPP.png), plus a red diff of the last vs first
import fs from 'node:fs'; import zlib from 'node:zlib';
const [outp, dprS, scaleS, idS, ...names] = process.argv.slice(2); const dpr = +dprS, scale = +scaleS, id = +idS;
const F = fs.readFileSync('/home/agents/scratch/katex-harden/data/formulas.jsonl', 'utf8').trim().split('\n').map(JSON.parse);
const idx = F.findIndex((f) => f.id === id); const page = Math.floor(idx / 25), k = idx % 25;
function png(path) {
  const d = fs.readFileSync(path); let i = 8, w, h, ct; const idat = [];
  while (i < d.length) { const n = d.readUInt32BE(i), t = d.toString('latin1', i + 4, i + 8);
    if (t === 'IHDR') { w = d.readUInt32BE(i + 8); h = d.readUInt32BE(i + 12); ct = d[i + 17]; } if (t === 'IDAT') idat.push(d.subarray(i + 8, i + 8 + n)); i += 12 + n; }
  const raw = zlib.inflateSync(Buffer.concat(idat)); const bpp = ct === 6 ? 4 : 3, stride = w * bpp; const img = new Uint8Array(h * stride); let pos = 0;
  for (let y = 0; y < h; y++) { const f = raw[pos++]; const o = y * stride, p = o - stride;
    for (let q = 0; q < stride; q++) { const x = raw[pos + q]; const a = q >= bpp ? img[o + q - bpp] : 0, b = y ? img[p + q] : 0, c = (q >= bpp && y) ? img[p + q - bpp] : 0; let v;
      switch (f) { case 0: v = x; break; case 1: v = x + a; break; case 2: v = x + b; break; case 3: v = x + ((a + b) >> 1); break;
        default: { const pp = a + b - c, pa = Math.abs(pp - a), pb = Math.abs(pp - b), pc = Math.abs(pp - c); v = x + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c); } } img[o + q] = v & 255; } pos += stride; }
  return { w, h, bpp, img }; }
const rows = []; const ROW = 200 * dpr; let W = 0, minx = 1e9, maxx = 0, miny = 1e9, maxy = 0;
const imgs = names.map((n) => png(`png/${n}_dpr${dpr}_${String(page).padStart(3, '0')}.png`));
for (const im of imgs) { const r = []; for (let y = k * ROW; y < (k + 1) * ROW; y++) r.push(im.img.subarray(y * im.w * im.bpp, (y + 1) * im.w * im.bpp)); rows.push(r); W = im.w; }
// crop to ink bounding box over all
for (const im of imgs) for (let y = k * ROW; y < (k + 1) * ROW; y++) for (let x = 0; x < W; x++) { const o = (y * W + x) * im.bpp; if (im.img[o] < 250 || im.img[o + 1] < 250 || im.img[o + 2] < 250) { minx = Math.min(minx, x); maxx = Math.max(maxx, x); miny = Math.min(miny, y - k * ROW); maxy = Math.max(maxy, y - k * ROW); } }
minx = Math.max(0, minx - 8); maxx = Math.min(W - 1, maxx + 8); const cw = Math.min(maxx - minx + 1, 1400 / scale);
miny = Math.max(0, miny - 6); maxy = Math.min(ROW - 1, maxy + 6); const RH = maxy - miny + 1;
const outW = Math.floor(cw * scale), blocks = names.length + 1, outH = blocks * (RH * scale) + (blocks - 1) * 4;
const out = Buffer.alloc(outH * outW * 3, 128);
function put(by, fn) { for (let y = 0; y < RH * scale; y++) for (let x = 0; x < outW; x++) { const sy = miny + Math.floor(y / scale), sx = Math.floor(x / scale) + minx; const [r, g, b] = fn(sy, sx); const o = ((by + y) * outW + x) * 3; out[o] = r; out[o + 1] = g; out[o + 2] = b; } }
names.forEach((n, i) => put(i * (RH * scale + 4), (sy, sx) => { const o = sx * imgs[i].bpp; const r = rows[i][sy]; return [r[o], r[o + 1], r[o + 2]]; }));
put(names.length * (RH * scale + 4), (sy, sx) => { const a = rows[0][sy], b = rows[rows.length - 1][sy]; const o = sx * imgs[0].bpp; const d = a[o] !== b[o] || a[o + 1] !== b[o + 1] || a[o + 2] !== b[o + 2]; return d ? [255, 0, 0] : [255 - (255 - a[o]) / 3 | 0, 255 - (255 - a[o + 1]) / 3 | 0, 255 - (255 - a[o + 2]) / 3 | 0]; });
// write png
const crc = (() => { const t = new Uint32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; }); return (b) => { let c = ~0; for (const x of b) c = t[(c ^ x) & 255] ^ (c >>> 8); return ~c >>> 0; }; })();
const chunk = (t, d) => { const b = Buffer.alloc(12 + d.length); b.writeUInt32BE(d.length, 0); b.write(t, 4, 'latin1'); d.copy(b, 8); b.writeUInt32BE(crc(b.subarray(4, 8 + d.length)), 8 + d.length); return b; };
const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(outW, 0); ihdr.writeUInt32BE(outH, 4); ihdr[8] = 8; ihdr[9] = 2;
const raw = Buffer.alloc(outH * (outW * 3 + 1)); for (let y = 0; y < outH; y++) out.copy(raw, y * (outW * 3 + 1) + 1, y * outW * 3, (y + 1) * outW * 3);
fs.writeFileSync(outp, Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]));
