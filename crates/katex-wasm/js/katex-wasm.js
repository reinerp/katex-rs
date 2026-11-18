// Minimal hand-written glue for katex_wasm.wasm (no wasm-bindgen): works in a browser (instance given) and in node.
// mode: 0 legacy, 1 html, 2 flat, 3 font, 4 font + compact_line, 5 flat + compact_line
class KatexWasm {
  constructor(instance) {
    this.x = instance.exports; this.enc = new TextEncoder(); this.dec = new TextDecoder();
    this.cap = 1 << 16; this.inp = this.x.alloc(this.cap);
    this.stat = { encode: 0, decode: 0, call: 0 };
  }
  mem() { return new Uint8Array(this.x.memory.buffer); }
  ensure(n) { if (n > this.cap) { this.x.dealloc(this.inp, this.cap); this.cap = Math.max(n, this.cap * 2); this.inp = this.x.alloc(this.cap); } }
  // one formula: copy the TeX into wasm memory, call, decode the HTML
  render(tex, display, mode) {
    this.ensure(tex.length * 3 + 4);
    const n = this.enc.encodeInto(tex, this.mem().subarray(this.inp, this.inp + this.cap)).written;
    const p = this.x.render(this.inp, n, (mode << 1) | (display ? 1 : 0));
    return this.dec.decode(this.mem().subarray(p, p + this.x.out_len()));
  }
  // many formulas in one call: texs[i], displays[i]; returns the HTML strings. t (optional) gets the times of the three stages
  renderBatch(texs, displays, mode, t) {
    const T = t ? () => performance.now() : null; let t0 = T && T();
    let total = 0; for (const s of texs) total += s.length * 3 + 5;
    this.ensure(total);
    const mem = this.mem(), dv = new DataView(this.x.memory.buffer);
    let o = this.inp;
    for (let i = 0; i < texs.length; i++) {
      mem[o] = displays[i] ? 1 : 0;
      const n = this.enc.encodeInto(texs[i], mem.subarray(o + 5, this.inp + this.cap)).written;
      dv.setUint32(o + 1, n, true); o += 5 + n;
    }
    let t1 = T && T();
    const p = this.x.render_batch(this.inp, o - this.inp, mode << 1);
    let t2 = T && T();
    const len = this.x.out_len(), mem2 = this.mem(), dv2 = new DataView(this.x.memory.buffer), res = new Array(texs.length);
    let q = p;
    for (let i = 0; i < texs.length; i++) { const l = dv2.getUint32(q, true); res[i] = this.dec.decode(mem2.subarray(q + 4, q + 4 + l)); q += 4 + l; }
    if (t) { const t3 = T(); t.encode = t1 - t0; t.call = t2 - t1; t.decode = t3 - t2; }
    return res;
  }
}
if (typeof module !== 'undefined') module.exports = { KatexWasm };
