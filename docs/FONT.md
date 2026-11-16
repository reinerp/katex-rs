# Maths layout in a generated font (feasibility prototype)

A typeset formula as one element with one text node: the layout KaTeX does with elements (font switches, spacing
spans, italic-correction margins, size and vertical-shift spans) is baked into one generated font, and the formula is
a string of code points in it. Nothing here is wired into the Rust crate; this is the font compiler, an encoder
prototype, the measurements, and a recommendation. The numbers are in "Results" below, the raw files in
`docs/font-measurements/`.

```html
<span class=mf style="--h:.88em;--d:.31em">𝑥&#xE251;&#xE016;&#xE015;…</span>   <!-- x_i^2 + a = b, about 35 bytes -->
```

## Files

| path | what |
| --- | --- |
| `crates/katex/tools/sfnt.py` | stdlib sfnt reader/writer: glyf outlines, cmap 4/12, GPOS, WOFF 1, WOFF 2 (untransformed and transformed glyf/loca/hmtx) |
| `crates/katex/tools/gen_math_font.py` | the font compiler: `--fonts DIR --spec SPEC.json --out OUT --name NAME` |
| `crates/katex/tools/mathfont_encode.py` | the encoder prototype (glyph list -> code points), what the Rust encoder has to do |
| `crates/katex/data/math-font/` | the recommended build: `.woff2` (ship), `.map.json` (encoder input), `.css`, `spec.json` (inputs), `.ttf`/`.woff` are debugging outputs and not checked in |
| `docs/font-measurements/` | measurement scripts (`tools/`), results and crops (`results/`) |

`gen_math_font.py` needs `brotli` (the Ubuntu 24.04 package `brotli` 1.1.0-2ubuntu0.1, Google's reference CLI, used
as a build-time tool only; `BROTLI=/path/to/brotli` if it is not on `PATH`). The generated `.woff2` is checked in,
so consumers and the Rust build never need it. Everything else is the Python standard library.

## Design

### The font

One TrueType font, `unitsPerEm` 4000 (see "Precision"). Glyph ids are assigned in code point order, so each plane is
one cmap group.

1. **Base planes**: the outlines of KaTeX's own TTFs (Main-Regular, Math-Italic, AMS-Regular, Size1-4, ...), one glyph
   per (source font, character). Hinting is dropped (the source glyphs carry little: 2-5% of `glyf`, and an `fpgm`,
   `prep`, `cvt`, `gasp` of 3.8 KB per font). Coordinates are scaled by `upem/1000`.
2. **Italic correction**: for Math-Italic the advance includes KaTeX's `margin-right: <italic correction>`. A
   `noic` twin (same outline, one-component composite, advance without the correction) is used when a subscript
   follows, because KaTeX tucks it with `margin-left: -ic`.
3. **Script variants** are TrueType composite glyphs: one component (the base glyph), a uniform F2Dot14 scale (0.7 or
   0.5) and a y offset in font units; the advance in `hmtx` is the scaled advance. One per (size, vertical shift, glyph).
4. **Spacers**: empty glyphs with exact advances: named ones (the gaps that occur in the corpus: 3mu, 4mu, 5mu, quad,
   0.05em script space, ...) and a binary set of +-2^k units (k = 0..14), so any kern is a few characters. A negative
   spacer has advance 0 in `hmtx` and a negative `XAdvance` from one GPOS single-adjustment lookup (format 2, the
   only lookup in the font, registered under `kern` and `dist`).
5. **Accent overlays** (composites of two components: the letter and the accent at KaTeX's offset, skew included),
   **rule glyphs** (one 2 em unit rectangle, scaled in x and y by an x/y-scale composite, 2^k/512 em wide, at a
   thickness and height), and the **private-use twins of the real letters** (see "Risks", run clamp).

Vertical metrics: `hhea`/`OS/2` ascent = descent = 0.6 em, and the formula span has `line-height: 0`. With
ascent = descent the zero-height inline box sits on the baseline, so the formula never changes the height of the
line it is in.

### Code points

* Main-Regular characters that are not letters keep their real code point (ASCII digits and punctuation, U+2212, ...):
  copying a formula gives readable text for them, and a failed font load degrades to a system-font rendering instead
  of boxes.
* Math-Italic letters use the Mathematical Italic block (U+1D44E, ...), also real, so `x_i` copies as `𝑥`.
* Main-Regular letters are real ASCII/Greek/Latin letters too, with private-use twins (below).
* Everything else is private use, BMP first (U+E000..U+F8FF, 3 bytes in UTF-8, 6,400 code points), then plane 15.
  In the recommended build everything fits in the BMP block. Order: spacers, noic, alt letters, base planes,
  script variants by frequency of their (size, shift), rules, overlays, negative spacers.
* No line break inside a run (the span is `white-space: nowrap`); an allowed break is an explicit U+200B, which breaks
  only when the span is not `nowrap` (checked).

### Encoder

The encoder keeps an exact cursor in font units. For each positioned glyph (source font, character, size relative to the
formula, x, baseline height) it emits spacers to reach x if the cursor is not there (named spacer first, then the
binary ones, negative ones moving back), then the glyph's code point, advancing the cursor by that glyph's advance. A
glyph at a size/shift that is not in the font cannot be encoded: the formula falls back to markup. See
`mathfont_encode.py`.

### Precision

`unitsPerEm` 4000. Each glyph's advance is rounded to a font unit, a script variant's to a font unit of 0.7 or 0.5
times the base advance, and a spacer is exact to a unit (0.0025 px at 20 px of font size). Blink lays out in 1/64 px
(0.0156 px), so a measured glyph position has a resolution of 0.0156 px and the figures below cannot show a
difference below that. Max deviation of a glyph's left edge from KaTeX JS 0.18.5 over the corpus's font-only inline formulas with upem 1000 / 4000 /
8000: 0.078 / 0.047 / 0.047 px (p95 0.047 / 0.031 / 0.031); the recommended 4000 build has one formula of 264 glyphs at 0.078. The font is 14% smaller
at 1000 than at 8000 because the outline coordinates need fewer bytes (WOFF 2 of the same build: 65.5 / 71.0 / 73.2 /
76.6 KB at 1000 / 2000 / 4000 / 8000), so 4000 is the compromise; 1000 is the choice if 0.08 px is acceptable.

### text-rendering

The span needs `text-rendering: geometricPrecision`. Without it, headless Chrome on Linux (default text rendering,
device scale 1 *and* 2) rounds every glyph's advance to a whole pixel, spacers and script variants included, and a formula
comes out up to 40 px too wide or narrow (both KaTeX and the font in `auto`: max glyph deviation 23.8 px, formula width
39 px, `results/both_auto_default.txt`). With `geometricPrecision` the advances are exact at every size from 6 to
100 px (`widths 6..100 px: error <= 0.011 px`, a back-and-forth spacer test). Stock KaTeX sets `text-rendering: auto`,
so on such a platform KaTeX's own layout is the rounded one: against it the font differs by 0.83 px median and 16.8 px
max in formula width and nearly every pixel row (`results/stock_katex_auto_dpr1.txt`). All fidelity numbers below
compare against KaTeX JS forced to `geometricPrecision` too, i.e. against its ideal layout; on macOS, Windows and
other platforms that always position text at sub-pixel precision stock KaTeX is already that.

### Coverage of (size, shift) pairs

`mfanalyze.py` and `results/coverage_corpus.txt`. Over the corpus (2,254 renderings: inline and display) there are 109 distinct
(size, shift) pairs among the 6,583 glyphs that are script size (0.7 or 0.5 of the formula's); 1,236 renderings have scripts, 219 of
them both raised and lowered ones. The two shifts of a lone subscript (0.7, -0.15) and superscript (0.7, 0.363) are
44% and 18% of the script glyphs. Glyphs of base size at shift 0 are 68.7% of all.

| pairs in the font | script glyphs covered | renderings with scripts fully covered |
| --- | --- | --- |
| 1 | 43.7% | 48.4% |
| 2 | 61.6% | 74.9% |
| 4 | 71.7% | 80.3% |
| 8 | 79.3% | 84.5% |
| 16 | 86.1% | 88.7% |
| 32 | 93.1% | 90.9% |
| 64 | 98.6% | 96.5% |
| 109 (all) | 100% | 100% |

The tail does not shrink by rounding the shifts (to 0.005 em: 133 pairs, to 0.02 em: 113): the large shifts are stacked-script and
big-operator positions, one per kind of base. Named spacers: the 16 commonest gaps are 5mu (1,167 occurrences), 4mu (847), the
0.05 em script space (680), 3mu (525), then 0.327, 0.2715 em, 0.25 em, 2.17 em, ... (`results/gap_clusters.json`); the top 8 are 78%
of the gaps of 100 units or more, the top 16 83%, the rest are written with the binary spacers. Accents are rare in this corpus
(3 occurrences, `\hat` and similar in none with a single-glyph base), so the 468 accent composites are for completeness.

## Results

All measured on the shared machine, headless Chrome 156 on Linux over CDP, KaTeX JS 0.18.5, the 1,128-formula corpus
of the compact work (`~/scratch/katex-harden/data/formulas.jsonl`), 16 px of body text so the formula is 19.36 px.
Other agents were running Chrome at the same time (load average 4 to 9): timings are medians of 7 interleaved runs
with the minimum next to them, and a difference under 15% is noise.

### a. Does Chrome accept it, and how big is it

TTF, WOFF 1, WOFF 2 with untransformed glyf/loca, and WOFF 2 with transformed glyf/loca/hmtx all load (no OTS error,
`document.fonts` status `loaded`) and render all 3,633 glyphs of the recommended build with **0 differing pixels** across
the four (`tools/fmtcheck.py`). The transformed WOFF 2 is also decoded by an independent stdlib decoder
(`tools/woff2dec.py`, which first agrees with Google's encoder's output for `KaTeX_Main-Regular.woff2`) with 0 outline
differences; FreeType (via `fc-scan`) parses the TTF's cmap. Two things OTS insisted on: a non-empty `glyf` (also for the
blank fallback face), and a transformed `hmtx` only after `glyf` in the table directory (tag order, as Google's encoder).
No other validator is installed (no `ots-sanitize`, `fonttools`, `woff2_*`, Firefox).

Source `glyf` instructions: 2,106 of 138,700 bytes (1.5%) in the seven main fonts, plus 26,675 bytes of `fpgm`, `prep`,
`cvt `, `gasp`: all dropped (they cannot survive the unitsPerEm change anyway). Hinted Windows rendering of KaTeX's fonts may
differ slightly from the unhinted outlines here; only Linux Chrome was tested.

Sizes in bytes. TTF raw; WOFF 1 (zlib 9); WOFF 2 untransformed and transformed (Brotli 11, window 24, via the `brotli` CLI);
zstd -19 and brotli -11 of the raw TTF (what the server may do itself). Coverage: share of the 2,254 corpus formulas
(each of the 1,127 renderable corpus formulas in inline and in display style) that are fully font-expressible, and of all their glyphs that have a code point.
For scale: KaTeX's own WOFF 2 are 76,256 for Main-Regular + Math-Italic + AMS + Size1, 90,016 with Size2-4, 259,792 for all 20.

| build | glyphs | variants | TTF | WOFF 1 | WOFF 2 untransf. | **WOFF 2** | zstd -19 | brotli -11 | formulas | glyphs |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| M3 base only (Main, Math, Size1) | 622 | 0 | 87,568 | 52,532 | 48,804 | **43,068** | 51,129 | 48,942 | 43.1% | 67.3% |
| M3, 2 pairs (minimal useful) | 1,100 | 478 | 99,996 | 58,524 | 52,836 | **45,960** | 56,344 | 52,904 | 82.2% | 82.4% |
| M3, 8 pairs | 2,087 | 1,465 | 125,656 | 68,840 | 58,160 | **50,684** | 64,182 | 58,302 | 85.2% | 87.0% |
| M3, 16 pairs | 2,698 | 2,076 | 141,424 | 75,416 | 63,060 | **54,356** | 69,864 | 63,327 | 86.0% | 89.9% |
| M3 + AMS subset, 16 pairs | 2,745 | 2,076 | 161,192 | 85,720 | 70,488 | **61,680** | 76,476 | 70,514 | 86.3% | 89.9% |
| + Size2-4, 16 pairs | 2,851 | 2,076 | 174,512 | 93,424 | 77,464 | **67,376** | 84,387 | 77,567 | 86.4% | 90.2% |
| + accents + rules, 16 pairs | 3,331 | 2,076 | 189,872 | 97,732 | 80,264 | **69,432** | 86,599 | 80,231 | 86.4% | 90.3% |
| **recommended**: same, 64 pairs (the committed build) | 3,763 | 2,508 | 202,076 | 102,864 | 84,060 | **72,740** | 90,418 | 84,060 | 88.7% | 97.4% |
| M4 (Main, Math, full AMS, Size1) base only | 879 | 0 | 148,708 | 85,328 | 76,884 | 67,532 | 80,912 | 76,858 | 43.2% | 67.3% |
| M4, 8 pairs, closure for 2 | 1,553 | 674 | 167,580 | 93,740 | 82,256 | 71,896 | 87,714 | 82,443 | 85.5% | 87.1% |
| M4, 8 pairs, closure for all 8 | 2,344 | 1,465 | 189,728 | 101,876 | 86,024 | 75,412 | 92,663 | 86,086 | 85.5% | 87.1% |
| M4, 32 pairs, closure for 2 | 1,919 | 1,040 | 177,708 | 98,472 | 85,360 | 74,748 | 91,381 | 85,579 | 87.2% | 93.8% |
| M7 (+Size2-4), 8 pairs | 1,659 | 674 | 180,900 | 101,420 | 89,256 | 77,640 | 95,037 | 89,214 | 85.6% | 87.4% |
| all 20 fonts, base only | 2,382 | 0 | 420,552 | 256,384 | 232,596 | 204,260 | 245,078 | 232,410 | 44.0% | 68.7% |
| all 20 fonts, 8 pairs | 3,059 | 677 | 439,508 | 265,388 | 237,756 | 208,880 | 252,843 | 237,853 | 87.5% | 88.5% |
| all 20 fonts, 64 pairs, accents, rules | 4,541 | 1,679 | 482,804 | 281,592 | 248,060 | 217,040 | 263,631 | 247,784 | 90.8% | 98.5% |

"closure" is the script glyph set (all Math-Italic, ASCII and common symbols of Main-Regular) given to a pair; a pair
beyond the closure count gets only the glyphs seen there. A script variant costs about 5 compressed bytes, so giving every
pair the closure (M4, 8 pairs: 75.4 against 71.9 KB) is cheap; the base outlines are the cost (AMS alone is 28 KB in
KaTeX's WOFF 2 and used by 10 glyphs of the corpus, which is why the recommendation carries a 70-glyph AMS subset:
blackboard capitals, what the corpus uses, a few more). How the bytes come about, for M4 with 16 pairs (80,008 WOFF 2):

| step | bytes |
| --- | --- |
| KaTeX's four source TTFs, hinted | 160,748 |
| their WOFF 2 | 76,256 |
| our base outlines only, unhinted, upem 4000, TTF / WOFF 2 | 148,708 / 67,532 |
| + 2,100 script variants as one-component composites, TTF / WOFF 1 / WOFF 2 untransformed / WOFF 2 | 213,352 / 111,464 / 91,828 / 80,008 |
| the same variants as flattened simple outlines instead | 623,628 / 386,404 / 148,276 / 125,284 |
| without the private-use twins of the letters (they cost 82 glyphs) | 211,384 / 110,768 / 91,464 / 79,648 |
| without sharing identical outlines between fonts (43 in all 20 fonts, 11 in M4: composite reference instead) | 214,592 / 111,828 / 91,796 / 79,868 |
| unitsPerEm 1000 / 2000 / 4000 / 8000 (M4, 8 pairs, WOFF 2) | 65,540 / 70,984 / 73,164 / 76,580 |

So composites are worth 45 KB of WOFF 2 (36%) for the variants; transforming glyf/loca/hmtx is worth 12 KB over
untransformed (13%) and 31 KB over WOFF 1; identical-outline sharing and the twins are noise after compression.
Our WOFF 2 encoder on `KaTeX_Main-Regular.ttf` gives 26,008 bytes against Google's 26,272.

Table hygiene of the committed build (TTF 202,076 bytes): `head` 54, `hhea` 36, `maxp` 32, `OS/2` 96, `post` format 3 (32, no glyph
names), `name` 230 (family, style, unique, full, version, PostScript name), `GPOS` 172 (one single-adjustment lookup for the
negative spacers, no `kern`, `GDEF`, `GSUB`), `cmap` 2,232 (formats 4 and 12, 115 groups: glyph ids run in code point order so
each plane and each run of real code points is one group; the negative spacers are last in glyph order, one group, so that
`numberOfHMetrics` leaves out their `hmtx` entries), `hmtx` 15,012, `loca` 15,056 (long format, because `glyf` is 168,932
bytes, above the 131,070 of the short one), `glyf` 168,932 (simple glyphs without instructions, repeat flags and short
vectors; a composite is 10 header bytes plus 4 + 4 (byte offsets when both fit in 8 bits, else words) + 2 (scale) or 4 (x/y
scale), 20 bytes padded to 4). In the WOFF 2 the transformed `glyf` is 146,441 bytes, `loca` is 0 (rebuilt), `hmtx` is 7,527 (the
lsb arrays are dropped because lsb = xMin for every glyph).

### b. Fidelity against KaTeX JS 0.18.5

Recommended build, inline formulas, KaTeX JS in `geometricPrecision` (see above), glyph rectangles from Range rects:

* 1,051 of 1,127 formulas are fully encoded; 1,002 of those are glyph-only (no rule, no SVG); the rest are fractions with
  bars, roots, stretchy arrows, boxes, a few fonts not in the build.
* glyph left edge: median 0.0157 px (one layout unit), p95 0.031, **max 0.078 px (one formula of 264 glyphs; 1 of 1,002
  above 0.05)**; formula width: median 0, max 0.047 px (`results/fid_rec`, summary in `results/`).
* pixels, `a+b+c=d` style formulas with only base glyphs (487): device scale 1: 238 identical, 94% with no pixel more than
  64/255 different, median 1 differing pixel; scale 2: 20% identical, 63% with none above 64, median 13 pixels.
  Formulas with scripts (515): scale 1: 0 identical but 72% with none above 64 (median 67 faint pixels, p90 269); scale 2:
  15% with none above 64, median 239 differing pixels, 13 above 64. `crops/inline_dpr2_katex_font_diff.png`, `inline_dpr1_x4_...`
  (rows: KaTeX, font, difference in red), `sup2_dpr2_x14.png`.
* Cause of the pixel differences: a glyph is drawn at the sub-pixel position the font gives it, KaTeX's at a position
  rounded to 1/64 px per element, and a script's baseline offset is inside the outline (drawn at a fractional y) where
  KaTeX's script span is positioned and snapped as a box. The scale-0.7 composite itself is the same as the real font at the
  smaller size: a lone `28` at 13.552 px against the 0.7-scaled composite of the same glyphs differs in 81 of 270 ink
  pixels (all faint edges, `crops/composite07_vs_native_dpr2.png`), and a one-component composite against a flattened simple
  outline (scaled in the compiler) is the same to within 3 pixels. Shifting the composite by up to 0.03 em in 0.005 steps never
  gets below that (45 pixels at best), and neither do x offsets of +-0.03 em: the residue is rasterisation noise, not a
  placement error. No hinting differences appear because `geometricPrecision` turns hinting off on both sides.
* Accents: 468 `\hat` ... `\breve` x 52 letters (`results/accent_overlays.json`) all encode as one code point; scale 1:
  78% with no pixel above 64, 46 of 468 within 10 pixels; `\vec` is an SVG and not expressible.
* Fractions: a `\frac` of plain glyphs needs 15 to 20 code points (numerator and denominator at shifted pairs, the bar as
  rule glyphs of widths 2^k/512 em, negative spacers to overlap); the bar is a 0.04 em rectangle, not a snapped 1 px border,
  and the numerator and denominator differ from KaTeX by a vertical pixel snap (`crops/frac_dpr2.png`). Feasible, small
  corpus share (36 of the 1,127 formulas have a fraction bar), not built beyond the test.
* Tall delimiters from pieces: no formula of the corpus has a stacked-piece delimiter (46 have Size glyphs, all single glyphs).
  A multi-component composite (top, n repeats, bottom, each component a y offset) would do for a quantised set of heights;
  not built.
* Surrounding line (`tools/mfctx.py`, 830 inline formulas under 200 px, paragraph of 16 px text with line-height 1.5, 2 lines):
  with ascent = descent = 0.6 em and `line-height: 0` on the span the paragraph is exactly as tall as the same paragraph
  without a formula (0 px difference in all 830) and the baseline marker is where KaTeX puts it (max 0.19 px, 6 formulas);
  KaTeX itself makes the paragraph 0.22 or 1.22 px taller (its struts). With ascent 0.903 / descent 0.272 (Main-Regular's)
  and `line-height: normal` the result is the same as `line-height: 0`; with ascent = descent = 0.6 and normal line height
  every paragraph grows by 5 px. A pseudo-element strut from `--h`/`--d` changes nothing for these formulas (none is taller
  than the line) and exists for formulas that are. Formulas wider than the paragraph do not wrap inside the span (one
  nowrap run; KaTeX wraps between its `.base` boxes): the encoder would emit U+200B at TeX break points for long ones.

### c. Speed

Micro benchmark: 3,000 paragraphs "Step i: we have FORMULA here." in 760 px, settle time (load, fonts, two animation
frames) in ms, median / minimum of 7 interleaved runs, load average 6 to 7, all four variants in the same session
(`results/micro_settle.jsonl`). "Plain text" is the formula's characters as ordinary text (the floor), "current" is the fork's
HTML + MathML output, "compact" is `levels::ALL` as in `COMPACT.md`.

| formula | plain text | current | compact ALL | font (one span) | font / compact |
| --- | --- | --- | --- | --- | --- |
| `a+b+c=d` | 143 / 120 | 2,758 / 2,423 | 1,145 / 1,034 | 380 / 313 | 0.33 |
| `x_i` | 148 / 117 | 2,238 / 2,082 | 695 / 575 | 375 / 297 | 0.54 |
| `x^2` | 158 / 121 | 1,950 / 1,681 | 671 / 583 | 327 / 266 | 0.49 |
| `x_i^2` | 135 / 117 | 2,540 / 2,416 | 903 / 844 | 325 / 278 | 0.36 |

The font variants take 2 to 3 times the plain-text floor, mostly layout of 3,000 shaped runs with 3 to 13 glyphs (CPU
time of Layout 128 to 165 ms against 42 to 52 for plain text, style 21 to 26 against 7 to 10).

Fixture (400 messages of 12 corpus formulas, 4,800 occurrences, `content-visibility` off, 7 interleaved runs, load 4.5):

| page | bytes | settle ms median / min |
| --- | --- | --- |
| current (HTML + MathML) | 10,245,483 | 7,535 / 6,908 |
| compact ALL | 2,283,206 | 2,439 / 2,267 |
| compact ALL, font span where the formula is font-only (84.7% of occurrences, built with a 16-pair font) | 1,412,722 | 1,430 / 1,267 |

With the recommended 64-pair build 87.8% of the 4,796 valid occurrences are font-only (they hold 74.0% of the glyphs and
71.9% of KaTeX's HTML-branch elements, 135,372 of 188,277, which become 4,211 elements); the glyphs of the other formulas
that could be font runs inside the stack markup are another 23.4% of all glyphs, so 97.4% of glyphs could be in a font run.
The formulas that fall back are fractions (bars), roots and stretchy arrows (SVG), boxes, `\overset`-style stacks, and fonts or
(size, shift) pairs outside the build. Display formulas in the hybrid page are `.katex-display > .mf`.

### d. Shaping, selection, copy, find

* **Run clamp, the one surprise.** Chrome splits text into runs by script (real Latin, Greek and Cyrillic letters, private
  use, and Common characters such as digits, punctuation and the Mathematical Italic block are different scripts) and a run's
  total advance is clamped at zero. `a`, a spacer of the advance of `a`, `b` (real letters) is 42 px wide at 40 px, the
  spacer lost, and the spacer alone is 0; with `1` or `=` or a private-use glyph in place of one of the letters it works,
  because the spacers share a run with a character that is not a script letter, whose total is not negative
  (`results/shaping.json`, 30 cases). The GPOS adjustment itself is applied. The encoder therefore uses private-use twins of
  the real Latin/Greek letters in any formula that has a negative spacer (82 twins, 360 bytes of WOFF 2): `A`, back, `B`
  as twins is 22 px, 22.25 px with `geometricPrecision`; as real letters it is 42. Adding `latn`/`grek` script records to
  the GPOS table made no difference. In the corpus the case is rare (stacked scripts next to upright letters); the test
  strings with upright letters next to a stacked sub/superscript are the ones affected.
* Negative spacers (and everything else) are exact from 6 to 100 px of font size, at device scale 1 and 2
  (`widths 6..100 px: error <= 0.011 px` with `geometricPrecision`; with `auto` every glyph advance is rounded to a pixel,
  see above). Twin-encoded `A`, back, `B` is 22 px under default settings, `font-kerning: none`, `text-rendering:
  optimizeSpeed`, `font-variant-ligatures: none`, `font-feature-settings: "kern" 0` and `"kern" 0, "dist" 0`, `direction:
  rtl`, `zoom: 1.5`. It was 42 (lost) under `font-feature-settings: "kern" 0, "dist" 0` while the lookup was registered
  only under those two tags; it is now registered under `dist`, `kern`, `locl`, `mark`, `rlig` (a page cannot reasonably
  switch all of them off, a first build with two tags found out). `letter-spacing` is added to every glyph including the
  spacers (`letter-spacing: 1px` gives 30 px for the 22 px case), so the CSS resets it: `.mf { letter-spacing: normal }`.
  `word-spacing` does not matter (the private-use code points are not spaces). `writing-mode: vertical-*` is not supported.
* Line breaking: no break inside the run with `white-space: nowrap` (the span has it); without nowrap, ASCII punctuation
  breaks (`/`, `,`, ...), private-use letters do not break with `normal` but do with `overflow-wrap: anywhere` and `word-break:
  break-all`; a U+200B between code points gives an allowed break when not nowrap and none with nowrap, as intended.
* Selection: one highlight rectangle for the whole formula (ascent + descent = 1.2 em high), a drag selects code points,
  so a partial selection of a stacked script has an overlapping highlight; `crops/selection_dpr2.png`. KaTeX's selection is a
  ragged set of boxes of different heights.
* Copy: `a`, spacers, `𝑥`, `a`, `b` copies as `a U+EB05 U+EB04 U+EB01 U+E012 U+E00F U+E00D U+E00C 𝑥ab` (a code point list):
  letters and digits are readable, Math-Italic letters come out as Mathematical Italic (`𝑥`), operators as Unicode, but every spacer
  is a private-use character (about 4.9 per formula in the corpus), so pasted text has invisible junk in it. A `copy` handler or
  an `aria-label` with the TeX (as the compact output has) is needed to get TeX on the clipboard.
* Find in page: `window.find` finds ASCII characters of the formula; Math-Italic letters are U+1D4xx code points and are not
  matched by their ASCII base letter there (Ctrl+F's own folding was not tested).

### e. Risks

* **Loading and failure** (`mfshape.py`, `results/shape/fail_*.png`): `font-display: swap` shows the real characters
  of the fallback font immediately, with unspaced letters and one missing-glyph box per spacer (4.9 per formula on average): ugly (`results/shape/fail_*.png`).
  Fixed by the 344-byte blank face in the CSS (inline `data:` URI, `unicode-range` U+E000-F8FF): the spacers draw nothing, and
  while the font loads or when it fails the formula reads `a+b𝑥a` in system fonts (`crops` not kept; checked by screenshot).
  The page text reflows when the real font arrives (the formula span is inline, so it changes width). `font-display: block`
  hides the text for up to 3 s instead. A truncated or missing file is `status: error`, same as above.
* **Other browsers**: Firefox and Safari were not tested (none is installed). Things to check: the GPOS single adjustment
  with negative XAdvance, Common/Unknown-script run splitting (the clamp is Blink's; HarfBuzz is shared with Firefox, which
  shapes per script run as well), `text-rendering: geometricPrecision` meaning sub-pixel positioning, composite glyphs with
  F2Dot14 scale and big offsets in Core Text and DirectWrite, WOFF 2 transformed glyf. Windows with hinted KaTeX
  originals will look slightly different from the unhinted outlines here.
* **No MathML, no per-glyph styling**: a formula is a string, so it cannot be restyled per atom; accessibility needs an
  `aria-label` as the compact output has; text search and copy as above.
* **Print and zoom**: sizes are in em, so zoom is exact (checked at `zoom:1.5`); print was not compared. Forced-colours and
  `-webkit-text-stroke` apply to the whole span.
* **Fonts outside the build** and sizes outside the pair table cannot be encoded: the formula falls back to markup, so
  the stylesheet and the KaTeX fonts still ship.
* The font is not a font a person can type with: private-use code points and a sequence of spacer glyphs per formula; a
  change to the glyph order in a new font version changes every stored formula, so stored HTML must carry a font version (the
  CSS family name `KaTeXMathRun` + version, as the file name).

## Recommendation

* **Planes**: real code points where a neutral one exists (Main-Regular digits, punctuation and symbols; Math-Italic letters in
  the Mathematical Italic block), private use for everything else, BMP block first: all 3,763 glyphs of the recommended
  build fit in U+E000..U+F8FF with room (3,400 of the 6,400 used), so plane 15 is not needed; keep plane 15 for
  an all-20-fonts build (4,541 glyphs). Order inside the private-use block: named and binary spacers (the
  most used and shortest UTF-8 only matters for 3 bytes at most), noic twins, letter twins, base glyphs by font, script variants by
  pair frequency, rules, overlays, negative spacers last (they share advance 0, which `numberOfHMetrics` trims).
* **Repertoire**: Main-Regular, Math-Italic, Size1-4, an AMS subset, 64 (size, shift) pairs with the closure for the top 16,
  accent overlays for Math-Italic letters, and one rule family: 72.7 KB of WOFF 2 (the seven KaTeX fonts of the same repertoire are
  90 KB), and 88.7% of the corpus's formulas (97.4% of glyphs, 87.8% of the fixture's occurrences) as one
  element. The minimal useful set is Main-Regular + Math-Italic + Size1 with 2 pairs: 46 KB, 82% of formulas, because lone
  subscripts (0.7, -0.15) and superscripts (0.7, 0.363) are 62% of all script glyphs and the two together decide 75% of the formulas that have scripts.
  The stylesheet and the KaTeX fonts remain for the fallback formulas (roots and arrows are SVG, so that can't go away); a browser
  loads a KaTeX font only when a fallback formula on the page uses it.
* **Bold, Typewriter, Fraktur, SansSerif, Caligraphic, Script**: leave them to markup (together 5% of the corpus's formulas).

## What a Rust encoder has to do

1. Generate a Rust table from `KaTeXMathRun.map.json` (like `gen_tables.py`: (font, char) -> code point, advance, italic
   correction, noic and letter twin; (size, shift) -> glyph code points; spacers; rules; overlays).
2. Lay the formula's box tree out in em: walk the `HtmlDomNode` tree before serialising and compute, for every symbol, the
   source font (family, weight, style), the size relative to the formula (the `sizing`/`reset-size` chain), x (sum of
   widths, margins, kerns, `mspace`s in em of their own font size) and the baseline height (vlist shifts); KaTeX computes all
   of these symbolically, so no browser is needed. The prototype read them from layout (`mfextract.py`).
3. Keep the cursor in integer font units. For each glyph: gap = round(x * upem) - cursor; emit a named spacer
   then binary ones (positive or negative), or none within half a unit (named first, `Encoder.spacer`); emit the code point;
   cursor += its advance (with the noic twin when the next glyph starts at x + advance - italic correction, a subscript).
   Merge `(base, accent)` pairs into the overlay glyph, bars into rule pieces. Order: document order, or by x when that is shorter
   (stacked scripts: whichever makes fewer negative spacers).
4. If any (font, char, size, shift) is not in the table, or the box contains anything else than glyphs, spacers, bars and
   accents (SVG, borders, boxes), return `None` and emit the compact markup. Optionally keep font runs inside the markup
   for the glyph-only sub-boxes.
5. After a negative spacer anywhere in the formula use the letter twins; add U+200B at TeX break points for formulas wider than
   a line; escape `&`, `<`, `>`; write the trailing width spacer so that the span is as wide as the KaTeX box; write `--h` and `--d`
   (the strut, in em of the formula) and the `aria-label`.
6. Version the font file name; the stored HTML is meaningless with another glyph order.
