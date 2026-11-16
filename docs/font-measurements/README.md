# Measurements behind docs/FONT.md

Measured 2026-11-15 on the shared machine `agenthost` (load average 4 to 9 from other agents, one of them running Chrome
benchmarks), headless Chrome 156 over CDP. The reference is KaTeX JS 0.18.5 and the 1,128-formula corpus of the compact work.

`tools/` has the scripts (stdlib Python; Chrome-driving ones import `cdp.py`, `cu.py` and agentserver's `headless_chrome.py`,
and were run in the `sbx --no-net` wrapper `sxc` of `~/scratch/katex-font`); `regen.sh` is the sequence. `results/` has the numbers.

| file | what |
| --- | --- |
| `tools/mfextract.py` | per-glyph geometry of the corpus from KaTeX JS (font, char, size, x, baseline, advance, accent hints, rules) |
| `tools/mfanalyze.py`, `mfspec.py` | pair/glyph/gap statistics, and the font spec from them |
| `tools/mfaccents.py` | accent x letter overlay specs |
| `tools/mftest.py`, `mfpix.py`, `pngdiff.mjs`, `cmp.mjs` | encode formulas, compare widths, glyph x, pixels with KaTeX JS; crops |
| `tools/mfsizes.py` | sizes and coverage per repertoire |
| `tools/mfbench.py`, `mfhybrid.py` | micro pages and the fixture with font spans (page building: `~/scratch/katex-compact/tools/mkpage.py`; timing: `measure3.py` there) |
| `tools/mfshape.py`, `mfctx.py` | shaping/selection/failure behaviour; the line around a formula |
| `tools/fmtcheck.py`, `woff2dec.py` | all four containers render identically; an independent WOFF 2 glyf decoder |
| `results/sizes.json`, `sizes.txt` | the sizes table |
| `results/coverage_corpus.txt`, `gap_clusters.json`, `accent_overlays.json` | pair coverage, spacer clusters, accent offsets |
| `results/fidelity_metrics.json`, `fidelity_run.txt`, `format_check.txt` | per-formula widths, glyph x deviation, differing pixels (scale 1, 2) |
| `results/micro_settle.jsonl`, `fixture_settle.jsonl` | settle times |
| `results/line_context.json` | paragraph height and baseline around each formula, KaTeX vs the font variants |
| `results/shaping.json`, `shape/`, `crops/` | shaping tests and screenshots |
| `results/stock_katex_auto_dpr1.txt`, `both_auto_default.txt` | stock KaTeX / both in `text-rendering: auto` |
