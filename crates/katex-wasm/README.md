# katex-wasm

katex-rs as a WebAssembly module with a plain C ABI (no wasm-bindgen, no dependencies besides katex-rs), and a small hand-written JS glue
(`js/katex-wasm.js`). The default build is the **hybrid output only** (`levels::ALL | levels::FONT_RUN`, see `docs/FONT.md`, with the options
`compact_line`, `compact_wrap_min` and the `aria-label`) and is size-optimised.

```js
const katex = await KatexWasm.load('/katex.wasm');   // WebAssembly.instantiateStreaming: start it from the <head> to compile while the page parses
katex.init();                                        // optional: build the context now (about 3 ms) instead of in the first render (about 15 ms with the module's lazy compilation)
const html = katex.render('a+b+c=d', /* display */ false, /* mode */ 3);   // mode 4: with the host line extents (katex.setLine(0.8815, 0.3581))
```

C ABI (`src/lib.rs`): `alloc`, `dealloc`, `init(mode)`, `render(ptr, len, flags) -> ptr` (HTML at `ptr`, `out_len()` bytes, valid until the next call; `out_status()` is 1 for an error text),
`render_batch(ptr, len, flags)` (many formulas in one call, not faster than one call each), `set_label`, `set_wrap_min`, `set_line`.
Every render starts from the settings the context made: what `\gdef` and `\global\def` define is gone after the render.

## Build

`build.sh` (see its header): the nightly toolchain with `-Zbuild-std` (`panic=immediate-abort`, std's `optimize_for_size`), `opt-level = "z"`, an LLVM inline
threshold of 25, fat LTO, and optionally `wasm-opt --reorder-locals`. `FULL=1` builds the module with every output mode for comparison, `STABLE=1` uses the stable toolchain.

## What is left out of the default bundle

Cargo features of katex-rs that are off (they are all on by default, so the native build and its output are unchanged): `mathml` (MathML builders and the `Mathml` /
`HtmlAndMathml` outputs), `sizes` (`compact_sizes`, `compact::take_sizes`), `diagnostics` (`Debug` dumps in error messages, `\show`), `std-float` (core's float parser and
formatter and their 10 KB table: `numfmt.rs` has exact replacements for what the renderer reads and writes; `compact_precision` is ignored). Features that are on:
`fast-hash` (a multiply-and-rotate hash instead of SipHash for the tables), `fixed-levels` (the compact output always has every step and the font runs),
`packed-metrics` (the character metrics as columns of LEB128 numbers, unpacked when the context is made). The allocator is a bump allocator (`src/bump.rs`) whose
scratch region is emptied after every render.

## Size and speed

The fixture of 4,800 formulas in headless Chrome, all builds interleaved in every round (14 rounds, median over the rounds of the ratio to the baseline of the same round; two runs, at a load of
10 to 13 because other jobs were running), against the speed build of the first WASM measurement (opt-level 3, dlmalloc, nothing left out: 1,462,440 B raw, 342,393 B brotli, the font before
`bb64ff11`). `OPT_LEVEL` is z, `INLINE` the LLVM inline threshold:

| build | raw | brotli -11 | render time | settle time |
| --- | --- | --- | --- | --- |
| **`INLINE=25` (default)** | **573,669** | **181,542** | **0.91 / 1.03** | **0.97 / 0.97** |
| `INLINE=10` | 537,886 | 173,326 | 1.00 / 1.15 | 0.97 / 1.05 |
| `INLINE=0` | 523,559 | 168,870 | - / 1.10 | - / 0.98 |

The same flags before the font with the implicit layout and the packed symbols (681,472 B raw, 196,951 B brotli): render 0.90 to 0.97, settle 0.99 to 1.01 in three runs.
