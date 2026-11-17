# Measurements behind docs/FONT.md

Measured 2026-11-15 on the shared machine `agenthost` (load average 4 to 28 from other agents, several of them running Chrome
benchmarks), headless Chrome 156 over CDP. The reference is KaTeX JS 0.18.5 and the 1,128-formula corpus of the compact work
(`~/scratch/katex-harden/data/formulas.jsonl`, `formulas.bin` for the Rust harness).

`tools/` has the scripts (stdlib Python and a little Node; the Chrome-driving ones import `cdp.py`, `cu.py` and agentserver's
`headless_chrome.py`, and were run in the `sbx --no-net` wrapper `sxc` of `~/scratch/katex-font`; paths in them are those of that
machine); `regen.sh` is the whole sequence and stops with a message if `BROTLI` is not set. `results/` has the numbers: the files of
stage 1 (the prototype: whole formulas as one text node, measured with the Python encoder) and `results/hybrid/` (the Rust
`FONT_RUN` output, in which text nodes and elements alternate).

## Tools

| file | what |
| --- | --- |
| `tools/mkfont.sh` | the spec from the corpus, the font, the Rust table, rounds of "pairs the encoder needed"; `all 2` is the committed font, `even 2` the held-out one |
| `tools/mfextract.py`, `mfanalyze.py`, `mfspec.py`, `mfaccents.py`, `mfpairs.py` | per-glyph geometry of the corpus from KaTeX JS, coverage statistics, the spec, accent overlays, the missing pairs |
| `tools/ch_font.rs`, `ch_font-Cargo.toml` | the harness crate that renders the corpus with the Rust crate (`render formulas.bin out.jsonl MODE REPS inline\|display`; `MODE` `compact` is the flat design, `font` the step on, `font:LEVELS`; `css URL` prints the stylesheet; `MATHRUN_MISSING=1` prints the pairs the font lacks) |
| `tools/mfhyb.py`, `mfhpix.py`, `mfshift.py`, `mfcent.py`, `mfcat.py`, `pngdiff.mjs`, `pngshift.mjs`, `pngcent.mjs`, `cmp.mjs` | the Rust output against KaTeX JS: widths, glyph x, pixels (scale 1 and 2), 1-pixel shifts, ink centroids, by kind of formula; crops (rows: KaTeX, font, difference in red) |
| `tools/mfwrap.py` | line starts and paragraph heights at a given width (`custom=FILE` for formulas of your own, `owa` for a host with `overflow-wrap: anywhere`, `oldfont` for the font of the commit before) |
| `tools/targeted.tex` | 34 formulas with a stack, `\text` with spaces and hyphens, brackets, slashes, bars, commas ... for the line break check at 60 to 250 px |
| `tools/mfgaps.py`, `mfclasses.py`, `mfscriptspace.py`, `mfnamed.py` | the gaps between the glyphs of the runs the encoder wrote (`MATHRUN_RUNS=1` of the harness), by class of the atom; the script pairs that the script space follows; the named spacers (`MATHRUN_SPACES=1`) |
| `tools/mfpos.py`, `mfunits.py`, `mferr.py`, `mfold.py`, `pngside.mjs` | two versions of the encoder and the font against each other: glyph positions in Chrome and in font units, errors against the ideal positions, pixels, side-by-side crops with the difference |
| `tools/mfchars.py`, `mfbench4.py`, `evalb4.sh`, `mfsrv2.sh` | characters, bytes and elements per formula; the micro and fixture pages of floor / flat / old / new; the server render time old against new |
| `tools/exp_wrap2.py`, `exp_pua.py`, `exp_strut.py`, `exp_w.py` | where Chrome breaks (every pair of ASCII punctuation marks, a break before an inline block with each candidate character), the private-use code points, the strut of a formula that wraps, the width of a formula with `<wbr>` and with U+200B |
| `tools/mfshare.py`, `mfstock.py` | share of glyphs in text nodes and what is outside, bytes and elements |
| `tools/rsrun.sh`, `shiftfind.mjs` | render given formulas with the Rust harness and print the markup; the best whole-pixel shift of a row against KaTeX's |
| `tools/mfshow.py` | the side-by-side screenshot pages (`results/crops/hybrid_p*_dpr*.png`) |
| `tools/stacks.tex` | 24 formulas with stacks, for `mfhyb.py --custom` |
| `tools/mfbench2.py` | the controlled variants of the micro page ("where the time of a font formula goes") |
| `tools/mfbench3.py`, `measure3.py`, `evalb.sh`, `mfsrv.sh` | the micro pages and the fixture for floor / flat / hybrid, the interleaved runs, the server render time |
| `tools/evalall.sh`, `evalw.sh` | the geometry and line-break runs in one go |
| `tools/neg_spacer_across_elements.py`, `neg_spacer_runs.py`, `neg_spacer_sizes.py` | negative spacers: next to elements, runs, sizes |
| `tools/mfsizes.py`, `mftest.py`, `mfbench.py`, `mfhybrid.py`, `mfctx.py`, `mfshape.py`, `fmtcheck.py`, `woff2dec.py` | stage 1: sizes per repertoire, the prototype's fidelity, micro pages, line context, failure behaviour, the four containers |

## Results

| file | what |
| --- | --- |
| `results/hybrid/geometry_inline.txt`, `geometry_display.txt` | Rust output against KaTeX JS: glyph x, widths, pixels, 1-pixel shifts and ink centroids by kind of formula |
| `results/hybrid/metrics_inline.json`, `metrics_display.json` | the same per formula (`wk`, `wf` widths, `dx`, `dtop`, element counts, bytes, differing pixels `px1`, `px2`, centroid shifts `cx1`, ...) |
| `results/hybrid/wrap.txt` | line starts and paragraph heights, against KaTeX in `geometricPrecision`, in `auto`, and the flat design |
| `results/hybrid/share.txt`, `heldout.txt` | glyphs in text nodes and what is left; the held-out check |
| `results/hybrid/stacks.txt` | the stack formulas at four line heights and a serif page font |
| `results/hybrid/h_*` | the second hybrid stage (spacing in the glyphs, line breaks as characters; FONT.md "h"): characters, bytes, elements (`h_chars.txt`), the gaps of the corpus (`h_spacing_data.txt`), positions and pixels old against new (`h_positions_pixels.txt`, `h_positions_*.json`, `h_pixels_*.json`), against KaTeX JS (`h_katex_geometry.txt`), line breaks (`h_wrap.txt`), flag off identical to the commit before (`h_identity.txt`), timings (`h_bench_*`) |
| `results/crops/old_new_p*_dpr*.png` | 45 formulas, old / new / difference in red, scale 1 and 2 |
| `results/hybrid/bench_micro.jsonl`, `bench_fixture.jsonl`, `bench_server.txt` | settle and CPU times, server render time |
| `results/hybrid/bench_step1_variants.jsonl`, `bench_step1_variants2.jsonl` | the controlled variants of the micro page |
| `results/hybrid/neg_spacer_across_elements.txt` | the run clamp across elements |
| `results/crops/hybrid_p0..p5_dpr1\|2.png` | 74 formulas of every kind, KaTeX JS left and the font right, scale 1 and 2 |
| `results/crops/hybrid_diff_x3_dpr2.png` | KaTeX, font and difference (red) for `255^k` (a one-pixel baseline step) and the start of a formula with scripts (quarter-pixel steps), x3 |
| `results/sizes.json`, `sizes.txt`, `coverage_corpus.txt`, `gap_clusters.json`, `accent_overlays.json` | stage 1: sizes, pair coverage, spacer clusters, accent offsets |
| `results/fidelity_metrics.json`, `fidelity_run.txt`, `format_check.txt`, `line_context.json`, `shaping.json`, `shape/`, `crops/` | stage 1: prototype fidelity, containers, line context, shaping and failure screenshots |
| `results/micro_settle.jsonl`, `fixture_settle.jsonl`, `stock_katex_auto_dpr1.txt`, `both_auto_default.txt` | stage 1: settle times; stock KaTeX and both in `text-rendering: auto` |
