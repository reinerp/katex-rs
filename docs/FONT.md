# Maths layout in a generated font

KaTeX lays a formula out with elements: a font switch is a span, spacing an empty span with a margin, an italic correction a
margin on a letter, a script a size span and a vertical shift. A browser pays for each element (about 10 microseconds a formula
element for everything it does, `COMPACT.md`). This is a font that has that layout in its glyphs, and an encoder in the Rust crate
that writes a formula with it: wherever the font can express the nodes of a formula, they are **one text node** of its code
points, and wherever it cannot, they stay the elements the compact output writes, in the same tree. It is the compact output's
`levels::FONT_RUN` step, off by default (the page must serve the font).

```html
<!-- a + \frac{x^2}{y_i} = \mathbf{b}: text, a stack whose rows are text, text, a span in a stock KaTeX font.
     682 bytes and 16 elements (the flat design: 745 bytes, 21); the private-use characters are written as &#x...; here -->
<span class="katex mf f" role="math" aria-label="a + \frac{x^2}{y_i} = \mathbf{b}" style="--h:1.499em;--v:-.4811em">&#xE192;&#xE008;&#xE00E;&#xE00D;&#xE00C;&#xE05D;&#xE005;&#xE011;&#xE010;&#xE00B;<wbr><span class="nulldelimiter"></span>
  <span class="mfrac"><span class="X" style="padding-top:1.0179em;height:.4811em;vertical-align:-.4811em"><span class="W k1" style="top:-2.655em"><span class="F63">&#xE1AA;<span class="F31 zd">&#xE19A;</span></span></span><span class="W k1 iu"><span class="frac-line x0"></span></span><span class="W k1" style="top:-3.394em"><span class="F63">&#xE1A9;<span class="F31 zk">&#xE064;</span></span></span></span></span><span class="nulldelimiter"></span>
  &#xE013;&#xE00F;&#xE00D;&#xE00C;&#xE06F;&#xE008;&#xE011;&#xE00D;&#xE00B;<wbr><span class="mathbf">b</span></span>
```

Every formula gets the font for the maximal parts it can express and markup for the rest. `crates/katex/src/mathrun.rs` is the encoder, `compact::levels::FONT_RUN` the switch, `docs/font-measurements/` the scripts and
numbers. What it gives (the 1,128-formula corpus, "Results" below): **88.7% of the glyphs in text nodes (98.9% if `\text` is
written as glyphs too), 3.2 elements a formula where the flat design has 9.3, 30% fewer bytes in the DOM, 0.6 to 0.8 of the
flat design's browser time for the formulas that are all glyphs** (2 to 3 times the floor of the same text as plain text), and
ideal TeX positions (glyph x within 0.25 px of KaTeX JS, the median formula within 0.02 px; KaTeX's own rounding error is the larger
part of the difference), for a font of **72.8 KB** (WOFF 2) that has to be served.

## Files

| path | what |
| --- | --- |
| `crates/katex/src/mathrun.rs` | the encoder (the tree transform), the stylesheet and the accessors of the font |
| `crates/katex/src/generated/mathrun_table.rs` | the glyph table (`tools/gen_mathrun_table.py` from the font's map), checked in |
| `crates/katex/data/math-font/` | the font, `KaTeXMathRun-<version>.woff2` (ship it; the version is a hash of what the encoder depends on), its `.map.json`, `spec.json` (the inputs it was generated from), `KaTeXMathBlank.woff2` (an optional 344-byte fallback face); `.ttf`/`.woff` are debugging outputs and not checked in |
| `crates/katex/tools/sfnt.py`, `gen_math_font.py` | the font compiler (stdlib Python): glyf outlines, cmap, GPOS, WOFF 1 and WOFF 2 with the transformed glyf/loca/hmtx |
| `crates/katex/tools/gen_mathrun_table.py` | the glyph table of the encoder from the map |
| `crates/katex/tools/mathfont_encode.py` | the prototype encoder (positions measured in a browser), used for the measurements |
| `crates/katex/tests/mathrun_spec.rs` | tests |
| `docs/font-measurements/` | measurement scripts (`tools/`; `mkfont.sh` derives the spec and builds the font and the table, `mfhyb.py` compares with KaTeX JS), results and crops (`results/`, `results/hybrid/`), `regen.sh` (the whole sequence) |

The font compiler needs `brotli` (Google's reference command line tool: the distribution package `brotli`, Ubuntu 24.04's is
1.1.0-2ubuntu0.1; set `BROTLI=/path/to/brotli` if it is not on `PATH`; `regen.sh` and `mkfont.sh` stop with a message if it is not
found) to write the WOFF 2. It is a build-time tool only: the generated `.woff2`
is checked in, so consumers and the Rust build never need it. Everything else is the Python standard library; the Rust
crate has no new dependency.

## Design

### The font

One TrueType font, `unitsPerEm` 4000 (see "Precision"). All its glyphs are at private-use code points (U+E000..U+F8FF, 3 bytes
in UTF-8; about 3,900 of the 6,400), so a run of text in it is one script run (see "f. Shaping").

1. **Base glyphs**: the outlines of KaTeX's own TTFs (Main-Regular, Math-Italic, AMS-Regular as a subset of blackboard capitals
   and the symbols the corpus has, Size1-4), one glyph per (source font, character). Hinting is dropped (the source glyphs carry
   little: 2-5% of `glyf`, and an `fpgm`, `prep`, `cvt`, `gasp` of 3.8 KB per font; Windows rendering of hinted KaTeX fonts may
   differ slightly from these unhinted outlines, only Linux Chrome was tested). Coordinates are scaled by `upem/1000`.
2. **Italic correction**: for Math-Italic the advance includes KaTeX's `margin-right: <italic correction>`. A `noic` twin (the same
   outline as a one-component composite, advance without the correction) is used when a subscript follows, because KaTeX tucks
   it with `margin-left: -ic`.
3. **Script variants** are TrueType composite glyphs: one component (the base glyph), a uniform F2Dot14 scale and a y offset in
   font units; the advance in `hmtx` is the scaled advance. One per (size, vertical shift, glyph), with the size and shift
   **relative to the element that holds the run**: 0.7 and 0.5 of a formula's font, 0.714 (a script inside a script-size span of a
   stack), shifts from -1.3 to +1 em. The set of pairs is derived from the corpus (below).
4. **Spacers**: empty glyphs with exact advances: 16 named ones (the commonest gaps: 5mu, 4mu, 0.05 em script space, 3mu, ...) and a
   binary set of +-2^k units (k = 0..14), so any kern is a few characters. A negative spacer has advance 0 in `hmtx` and a
   negative `XAdvance` from one GPOS single-adjustment lookup (the only lookup of the font, registered under `dist`, `kern`,
   `locl`, `mark` and `rlig`, so that a page that switches kerning off does not lose it).
5. Accent overlays (two-component composites) and rule glyphs (a unit rectangle scaled in x and y) are in the font and are not used by
   the encoder yet (accents are stacks in KaTeX's tree; a fraction's bar is a border).

Vertical metrics: `hhea` and `OS/2` ascent and descent are those of KaTeX_Main (0.903 and 0.272 em), so a line that holds a formula
has the height it has with KaTeX's own fonts (the font is the first of the family, and `.katex` keeps its `line-height: 1.2`).

### The tree transform (`mathrun::transform`)

The built DOM tree of the formula, before `compact` rewrites it, is read the way a browser lays it out, in em of the element
that holds the text: inline flow, `em` margins and `top` offsets, `vlist` tables (a script or limit is an item at `top` below the
baseline by the height of its `pstrut`), `rlap`/`llap` (zero width), centred limits. In every list of sibling nodes:

* consecutive nodes the font can express become one text node: each glyph is its code point, and where it is not where the
  previous glyph ends, spacers close the gap (named first, then binary ones; a negative one moves the cursor back, which is how
  stacked scripts are one run: the superscript, a spacer back, the subscript, a pad). Glue at the ends of such a group is
  absorbed (it ends the line before a `<wbr>` as before);
* a node it cannot express stays an element, and its children are treated by the same rule;
* what is expressible: symbols in the font's source fonts (Main-Regular, Math-Italic, AMS, Size1-4) with their italic corrections
  and margins, `mspace` glue, nodes with `margin-left`/`margin-right`/`top`, lone and stacked scripts and limits whose content is
  expressible and whose (size, shift) the font has, `\not`-style overlaps, `\left.`;
* what stays markup: stacks (fractions, binomials, matrices, tight vlists: the flat design's stack container, rows and pstruts,
  unchanged, with text in the rows), SVG (roots, stretchy arrows and braces), rules and borders (`\boxed`, a fraction's bar),
  glyphs of other fonts (bold, calligraphic, fraktur, typewriter, sans) in a span with the stock KaTeX class, a script whose shift is
  not in the font (the flat design's lone-script span, with a base-plane run inside), size changes (`\large`: the size span, with a run
  inside), per-node styles such as colour (a span with the style, a run inside), text mode (`\text{...}`: stock text, so that it can
  be selected and read; `TEXT_AS_GLYPHS` makes it glyphs too, bit-for-bit the same layout, at 3 bytes a character);
* the root (`.katex`) keeps the strut and `compact_line` handling of the flat design and gets the class `mf`.

`<wbr>` between bases and `compact_wrap_min` are the flat design's (the count is of the characters KaTeX has, not of code points).
A text node's total advance is never negative (Chrome clamps it, below): a group whose width is negative stays elements.

### The stylesheet

`mathrun::stylesheet(font_url, blank_url)` is `compact::stylesheet()` plus the `@font-face` of the font and, for `.katex.mf`, the
generated font first in `font-family` (the stock KaTeX_Main after it) with `text-rendering: geometricPrecision` and
`letter-spacing: normal`, and a rule `.katex.mf <class>` for each class of `katex.css` that sets a font family (`.mathnormal`,
`.amsrm`, `.delimsizing.size1`, `.op-symbol.small-op`, ...) with the generated font first. So a character that is not in the
generated font (every real character: stock text, bold letters) is drawn by the stock font of its element as before, and a glyph
run inside a stock-font span is drawn by the generated font. The stock `@font-face` rules are those of the compact stylesheet:
a browser fetches only the faces a page uses.

### What the consumer must do

1. Enable the step: `Settings::builder().compact_levels(levels::ALL | levels::FONT_RUN)` (it needs `FLAT` and `UNWRAP_HTML`, which
   `ALL` has; without them the step does nothing).
2. Serve the font: `mathrun::FONT_WOFF2` under the name `mathrun::FONT_FILE_NAME` (it has the version in it: a stored formula is only
   valid with the font it was written for, and `mathrun::FONT_VERSION` says which) with a long cache lifetime, and use
   `mathrun::stylesheet(url_of_the_font, None)` instead of `compact::stylesheet()`. Optionally the 344-byte fallback face
   (`mathrun::BLANK_FONT_WOFF2`, second argument), which makes the spacers draw nothing while the font loads or if it fails.
3. Keep `text-rendering: geometricPrecision` on formulas (the stylesheet does it); a page rule that sets `text-rendering`,
   `letter-spacing`, `font-family` or `font` on `.katex` would change the output.
4. The text of a formula that is in the font is private-use characters when selected or copied; the TeX is in the `aria-label`
   (`compact_label`). A page that wants the TeX on the clipboard needs a `copy` handler.

### Precision

`unitsPerEm` 4000. Each glyph's advance is rounded to a font unit, a script variant's to a font unit of 0.7 or 0.5
times the base advance, and a spacer is exact to a unit (0.0025 px at 20 px of font size). Blink lays out in 1/64 px
(0.0156 px), so a measured glyph position has a resolution of 0.0156 px and the figures below cannot show a
difference below that. Max deviation of a glyph's left edge from KaTeX JS 0.18.5 over the 1,002 corpus formulas that were glyphs only (stage 1, one
text node each) with upem 1000 / 4000 / 8000: 0.078 / 0.047 / 0.047 px (p95 0.047 / 0.031 / 0.031); the 4000 build has one formula of 264
glyphs at 0.078. The numbers of the Rust output over all formulas are in "c. Fidelity". The font is 14% smaller
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

### Which pairs the font has

The pairs are the 64 of the table above, in order of the number of script glyphs they cover (the first 16 have the closure glyph set:
all of Math-Italic, ASCII and the common symbols of Main-Regular; the others only the glyphs the corpus had there), plus the pairs
that the Rust encoder needed and the font lacked on the corpus (a pair needed by 2 formulas is added, by 8 it gets the closure glyph
set). `tools/mkfont.sh all 2` does it in two rounds: round 1 adds what the font of the 64 lacked (23 pairs), round 2 starts again from the
64 and adds what the font of round 1 still lacked (10 pairs, 7 of them also in round 1's list). The committed font is that: **74 pairs, 2,763 variants**, and `mkfont.sh all 2` reproduces it byte for
byte. Carrying the first round's pairs over (a cumulative list: 90 pairs, 4,689 variants, 5,862 glyphs, WOFF 2 82,352 bytes) gives
the same share of glyphs in text nodes (88.7% and 83.5%) and 2.96 elements a formula where the committed font has 3.16: 13% more
font for 6% fewer elements: not taken. A formula the corpus did not have is covered by these as far as its scripts sit at the same
shifts: see the held-out check below.

## Results

Measured on the shared machine, headless Chrome 156 on Linux over CDP, KaTeX JS 0.18.5, the 1,128-formula corpus of the
compact work (`~/scratch/katex-harden/data/formulas.jsonl`: 1,127 render, one is a KaTeX error; 365 of them are display
formulas in the corpus), a 16 px body so the formula is 19.36 px. The Rust output is the `FONT_RUN` output of
the commit that has this file. "Flat" is the compact design without the step (`levels::ALL`, `d6a1d304`'s default
output). Other agents ran Chrome on the same machine the whole time (load average 4 to 28), so timings are interleaved and the
minimum is next to the median; a difference under 15% is noise. The raw files are in `docs/font-measurements/results/`.

### a. The font file

The committed font (`KaTeXMathRun-e9c82a06`): 3,936 glyphs (3,935 code points and `.notdef`), **WOFF 2 72,784 bytes** (the same file zstd -19: 71,821; the
untransformed WOFF 2 84,012, WOFF 1 103,180, TTF 205,100), all at U+E000..U+F8FF (3,935 of the 6,400 code points). 588 base glyphs
(Main-Regular, Math-Italic, a 70-glyph subset of AMS, Size1-4), 2,763 script variants of 74 (size, shift) pairs, 46 spacers (16
named, 2 x 15 binary), 468 accent overlays and 11 rule glyphs that the encoder does not use (they are 2,372 bytes of the WOFF
2: 70,412 without them). For scale: KaTeX's own WOFF 2 of the same repertoire are 90 KB.

TTF, WOFF 1, WOFF 2 with untransformed glyf/loca and WOFF 2 with transformed glyf/loca/hmtx all load in Chrome (no OTS error,
`document.fonts` status `loaded`) and render every glyph (3,888 of them) with 0 differing pixels across the four (`tools/fmtcheck.py`,
`results/format_check.txt`). The transformed WOFF 2 is also decoded by an independent stdlib decoder (`tools/woff2dec.py`,
which agrees with Google's encoder's output for `KaTeX_Main-Regular.woff2`) with 0 outline differences. Two things OTS insisted
on: a non-empty `glyf` (also for the blank fallback face), and a transformed `hmtx` only after `glyf` in the table directory.
Source `glyf` instructions are 1.5% of the source fonts' glyph data and the `fpgm`, `prep`, `cvt `, `gasp` tables another 26 KB:
all dropped, so Windows rendering of the hinted KaTeX fonts may differ slightly from these unhinted outlines (only Linux Chrome
was tested).

How the bytes come about and what the alternatives cost (stage-1 builds, which had the letter twins and a few more base
glyphs; the committed build is 72.7 KB where the table says 72,740): composites are worth 45 KB of WOFF 2 (36%) for the variants;
transforming glyf/loca/hmtx is worth 12 KB (13%) over untransformed WOFF 2 and 31 KB over WOFF 1; unitsPerEm 1000 / 2000 / 4000 /
8000 is 65.5 / 71.0 / 73.2 / 76.6 KB. A script variant costs about 5 compressed bytes, so giving every pair the closure glyph set
is cheap; the base outlines are the cost (AMS alone is 28 KB in KaTeX's WOFF 2 and used by 10 glyphs of the corpus, which is why
the font has a 70-glyph subset). The overlays and rules could be dropped for 2.4 KB.

| build (stage 1) | glyphs | TTF | WOFF 1 | WOFF 2 untransf. | **WOFF 2** | formulas fully in the font | glyphs in the font |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Main, Math-Italic, Size1, no variants | 622 | 87,568 | 52,532 | 48,804 | **43,068** | 43.1% | 67.3% |
| the same, 2 pairs (the minimum that is useful) | 1,100 | 99,996 | 58,524 | 52,836 | **45,960** | 82.2% | 82.4% |
| the same, 8 pairs | 2,087 | 125,656 | 68,840 | 58,160 | **50,684** | 85.2% | 87.0% |
| + AMS subset, Size2-4, accents, rules, 16 pairs | 3,331 | 189,872 | 97,732 | 80,264 | **69,432** | 86.4% | 90.3% |
| the same, 64 pairs | 3,763 | 202,076 | 102,864 | 84,060 | **72,740** | 88.7% | 97.4% |
| all 20 KaTeX fonts, 64 pairs | 4,541 | 482,804 | 281,592 | 248,060 | 217,040 | 90.8% | 98.5% |

(The last two columns count the whole-formula design of stage 1: a formula was in the font only if all of it was. With the
hybrid below, the share of glyphs that are in text nodes is what counts.)

### b. What is in text nodes

`tools/mfshare.py`, `mfstock.py`. Of the 13,411 glyphs of the 1,127 inline formulas, **11,896 (88.7%) are in text nodes**; of
the 8,878 glyphs of the 365 corpus formulas that are display style, 7,415 (83.5%). 1,035 of the 1,127 inline formulas have no
glyph outside a text node (284 of 365 display). What remains in stock spans:

* `\text{...}` in all its places: the root, a stack row, a `\boxed`, below an arrow or a brace, a cell of `aligned`: 1,535 of the
  1,854 stock characters (83%) of the inline formulas (`mfstock.py`). It is stock text on purpose (selectable, readable, the KaTeX
  font's own advances). With `TEXT_AS_GLYPHS` on in `mathrun.rs` (the same layout to the pixel, but three bytes a character and
  private-use characters on copy) the shares were **98.9% inline and 98.6% display**, 1,103 of 1,127 inline formulas wholly in text
  nodes;
* the 181 characters (10%) of `katex-error` output (a formula that does not parse);
* `\mathtt`, `\mathbf`, `\mathcal`, `\mathsf`, `\mathfrak` letters (the fonts are not in the build): 138 characters (7%), in about 5%
  of the formulas;
* glyphs at a (size, shift) the font does not have: none in this corpus, which the font was derived from (see the held-out check).

Elements per formula (spans, svg elements and `<wbr>`): flat 9.33 on average, median 5; **hybrid 3.16, median 1**, p95 10,
max 46 (display style, all 1,128 formulas: 9.54 to 3.21, median 6 to 2; the 365 display formulas of the corpus: 5.57). An
element of the flat design that is in the way of nothing (a base, a sizing span, a margin span) is a code point or a spacer now:
the stacks (fractions, matrices, accents) are what is left, with text in their rows.

Bytes per formula, raw: **flat 339 (median 178), hybrid 239 (median 134)**, 30% fewer; display style 369 to 270. zstd -19 of all the
formulas as one stream (what a connection that compresses the whole stream gets; the markup of the flat design is mostly
boilerplate that repeats from formula to formula, so it compresses to almost nothing): 28.6 bytes a formula flat, 29.1 hybrid
(display 28.7 and 29.3). The private-use characters compress worse than the markup does, so on the wire the two are the
same and what the hybrid saves is bytes in the DOM, memory, parse and layout.

Held-out check of the (size, shift) table: the font and its pairs derived from the even-numbered formulas only and evaluated on the
odd ones (`results/hybrid/heldout.txt`), share of glyphs in text nodes with `\text` as stock text: inline 91.4% on the even
formulas it was derived from and **86.0% on the odd ones it had not seen**; display 87.6% and **79.4%**; 516 of 563 unseen inline
formulas (137 of 179 display) have no glyph outside a text node. The committed font, derived from all of them, has 88.7% and
83.5%: a corpus of the same kind gets what the unseen half got, a few points less than the corpus the font was made from. What is
missing is a script at a shift that no formula of the training half needed; an example is the stacked `x_i^2` (see "Risks").
The knob is the number of pairs: the committed font has the 64 of the coverage table above and 10 more that the Rust encoder asked for
(74 pairs, 2,763 variants, `tools/mkfont.sh`).

### c. Fidelity against KaTeX JS 0.18.5

KaTeX JS forced to `text-rendering: geometricPrecision` (its ideal layout; see above) against the Rust output with the font, the
corpus inline (1,127 formulas) and display (365), glyph rectangles from Range rects, pixels at device scale 1 and 2
(`tools/mfhyb.py`, `mfhpix.py`, `mfcent.py`, `mfshift.py`; `results/hybrid/`).

* **Glyph x** (1,111 inline formulas whose glyphs could be paired one to one; the other 17 all have a control space `\ `, which
  KaTeX writes as a no-break space, and match in width): median 0.016 px, p95 0.078, p99 0.125, **max 0.25 px** (p99 0.109, max 0.141
  in display). Formula width: median 0.016, p99 0.109, max 0.234 px (display: all 0.0). The 18 formulas above 0.1 px have 119 KaTeX
  elements on average against 38 for all, the deviation has both signs and grows with the number of elements (max 0.094 px below 50
  elements, 0.25 above 50): it is KaTeX's own error, a position rounded to 1/64 px for every element, added up. The font has the
  ideal TeX positions to a font unit.
* **Glyph y.** Range rects give a text node's line box, not its glyphs, so the vertical position of the glyphs was compared by
  pixels: the **ink centroid** of every formula (`mfcent.py`): median shift 0.001 px (dpr 1) and 0.005 px (dpr 2), p99 0.098 and 0.443,
  max 0.36 and 0.84 device pixels; the formulas that match better shifted by one device pixel (`mfshift.py`, 1-pixel baseline snaps): inline none at scale 1 and 2 of
  1,127 at scale 2 (ids 169, 202); display 2 of 365 at scale 1 and 38 at scale 2. The tops of stock text in other fonts differ by up to 4 px in 24 formulas (`\mathtt` 4 px, `\mathsf` 3 px, bold 1 px):
  that is the rect of the line box, which has the ascent of the first font of the family (KaTeX_Main's, in the generated font)
  against the font's own; the pixels of these formulas are at the same place (ink centroid within 0.01 px at scale 1).
* **Pixels.** Identical pixels are rare and not the criterion: a glyph is drawn at the sub-pixel position the layout gives it,
  Chrome (Skia) draws glyphs on a sub-pixel grid of quarter pixels, as far as seen here, and KaTeX's and the font's positions differ by a few hundredths of a pixel, so a
  glyph is one step off now and then (the red rows of `crops/hybrid_*`: whole glyphs outlined, same shape, the next step). By
  kind of formula (inline; share with no pixel more than 64/255 different, scale 1 / scale 2): plain glyphs, spacing, no scripts
  (483): 93% / 60%, median 15 differing pixels at scale 2; scripts without stacks (523): 72% / 16%, median 248; fractions, matrices,
  aligned (35): 37% / 6%; with `\text` (76): 47% / 26%. Over all 1,127: 77.8% / 35.0%, median 34 / 156 differing pixels. The same on
  the 365 display formulas: 53.7% / 8.2%, median 99 / 680. The ink centroid of formulas with scripts: median |dx| 0.07 px, p90 0.28 px,
  of plain ones 0.000 and 0.025. The script glyphs are the large part of it: a scale-0.7 composite of the outline is not rasterised
  like the font at the smaller size (outline coordinates are rounded to 1/64 px at the rendered size, and the thin
  strokes of a script differ in coverage by a few percent; a flattened simple outline instead of the composite is the same as the
  composite), and KaTeX positions a script as a box that is snapped, the font draws it at its fractional y.
* **Stacks** (`results/hybrid/stacks.txt`, 24 formulas of `\frac`, `\binom`, matrices, `aligned` and scripts on stacks): at scale 1 no
  pixel above 64/255 in any of them, 58% identical; at scale 2 66.7% (83% with a serif page font of 22 px). The same with the row's
  `line-height` 1.2, `normal` and 3: 96 to 100% at scale 1, 66.7% at scale 2; the one that moves with the line height is a formula
  with `\mathbf`, `\mathcal`, `\mathtt` and `\mathsf` next to each other, one device pixel at scale 2.

### d. Line breaking, line height

300 and 760 px paragraphs ("Some words before, FORMULA and some words after, ...", `mfwrap.py`), every corpus formula:

* against KaTeX JS in `geometricPrecision`: at 300 px 322 formulas wrap and **all 322 have the same line starts**, none wraps only in
  the hybrid; at 760 px 19 of 19, none; the line count is the same in all 1,128 paragraphs.
* against KaTeX as the page serves it (`text-rendering: auto`, in which Chrome on Linux rounds every advance to a pixel and
  KaTeX's formulas are up to 17 px off from the ideal): at 300 px 309 of 317 have the same line starts, 8 do not (ids 96, 313, 333,
  356, 381, 412, 451, 768), 6 wrap only in the hybrid, 5 paragraphs have another number of lines; at 760 px all 19 have the same line
  starts and 27 paragraphs have another line count of the whole paragraph, all of them with a formula 1 to 2 px narrower or wider
  than KaTeX's rounded one that decides whether the last word fits. The flat design (rounded the way KaTeX is) has none of these.
  The differences are the width of the formula, not a break opportunity.
* paragraph height against KaTeX `auto` (1,101 paragraphs of the same line count at 760 px): identical to 0.05 px in 832; 0.22 or
  0.44 px taller in 268 (probably the unrounded ascent and descent of the font under `geometricPrecision`) and 0.78 px shorter in one;
  at 300 px the same plus 12 paragraphs differing by more than 1 px (the flat design: 13). The baseline of the text differs in 6 paragraphs (max 1.0 px) at 760 px and 19 at 300 px (flat 14, max 6.3 px).
  Against KaTeX in `geometricPrecision` 787 of the 1,128 paragraphs are 1 px shorter in the hybrid (max 2 px): KaTeX's own paragraphs
  are about 1 px taller in that mode than in `auto`, the hybrid's are not. The paragraph height of KaTeX as served (`auto`) is the
  reference, and the one above.

### e. Speed

**Where the time of a font formula goes** (`tools/mfbench2.py`, the micro page of 3,000 paragraphs "Step i: we have FORMULA here."
with `a+b+c=d` as the one formula, variants of it, load 2 to 3; main-thread CPU time of parse + style + layout + paint in ms, the
minimum of 7 interleaved runs, and the Layout and Style phases):

| variant | cpu min | Layout | Style |
| --- | --- | --- | --- |
| plain text, no span (the floor) | 59 | 30 | 6 |
| plain text in a bare span | 111 | 45 | 13 |
| + a class that sets a web font (KaTeX_Main) | 141 | 65 | 18 |
| + `text-rendering: geometricPrecision` | 153 | 71 | 18 |
| + `line-height: 0`, nowrap, `letter-spacing: normal` | 148 | 70 | 16 |
| the font, with the blank fallback face (`unicode-range`) and a strut | 231 | 154 | 25 |
| the font, no blank face, no strut | 193 | 109 | 17 |
| the font, the blank face, no strut | 188 | 120 | 17 |
| the font, the blank face without `unicode-range`, no strut | 197 | 133 | 18 |
| the font, a strut, no blank face | 232 | 129 | 23 |
| the same strut as a class instead of CSS variables | 222 vs 225 | 129 | 22 |
| negative spacer under 1, 3 or 5 feature tags, or none | 170 to 188 | 91 to 114 | 14 to 18 |
| 13 characters: ASCII / private use / Math-Italic block / ASCII mixed with private use | 142 / 170 / 177 / 203 | 68 / 87 / 96 / 130 | 13 to 16 |
| 3 / 13 / 40 private-use characters | 173 / 163 / 185 | 88 / 97 / 95 | 15 to 18 |
| kerning off, ligatures off, both off | 168 to 182 | 97 to 102 | 13 to 14 |

What it says: a span of its own costs 50 over the bare text (the floor is the text in the paragraph), a web font 30 more, the
font's own shaping 30 to 40 more (a text node of private-use glyphs against ASCII in the same web font; mixing ASCII and
private-use characters in one text node splits the run and costs 33 more); the strut of the formula (a `::before` inline-block,
3,000 of them) costs 40; the blank fallback face and its `unicode-range` cost next to nothing (188 against 193: noise); the number of
characters, the GPOS lookup (any number of feature tags, or no negative spacer) and the kerning and ligature switches do not
matter. So what changed in the build: only private-use code points (no real letters, no twins: no run splits; copy gives
private-use characters, the TeX is in the `aria-label`), and the strut is the compact design's (a class that names a
pseudo-element, and nothing when the host line has the room: `compact_line`), not a CSS-variable one (no faster as a class
either). The blank face is an optional argument of `stylesheet` and not in the default family, for simplicity and not for time.

**Micro benchmark** (3,000 paragraphs, 760 px, one formula each; settle time ms (load, fonts, two animation frames) median /
min of 7 interleaved runs, and main-thread CPU ms of the minimum; load average 10 to 22 the whole time, so numbers are
indicative; `tools/mfbench3.py`, `measure3.py`, `results/hybrid/bench_micro.jsonl`). Floor = the formula's characters as text.

| formula | floor | flat | hybrid | hybrid / flat (cpu) |
| --- | --- | --- | --- | --- |
| `a+b+c=d` | 454 / 323, cpu 189 | 1,685 / 1,074, cpu 831 | 1,158 / 670, cpu 490 | 0.59 |
| `\text{hello world}` | 365 / 342, cpu 203 | 741 / 709, cpu 453 | 863 / 726, cpu 514 | 1.13 |
| `x_i` | 304 / 264, cpu 151 | 845 / 600, cpu 480 | 588 / 508, cpu 362 | 0.75 |
| `x^2` | 216 / 157, cpu 101 | 576 / 458, cpu 374 | 458 / 405, cpu 309 | 0.83 |
| `x_i^2` | 142 / 131, cpu 79 | 853 / 767, cpu 650 | 824 / 787, cpu 667 | 1.03 |
| `\frac{a}{b}` | 293 / 158, cpu 103 | 1,871 / 1,257, cpu 1,087 | 2,024 / 1,089, cpu 923 | 0.85 |
| `\sqrt{x}` | 546 / 194, cpu 118 | 4,105 / 992, cpu 856 | 3,919 / 1,368, cpu 1,113 | 1.30 |
| `\sum_{i=0}^{n} i` | 692 / 497, cpu 198 | 7,174 / 3,025, cpu 1,889 | 6,146 / 4,043, cpu 1,707 | 0.90 |
| `\begin{pmatrix}a&b\\c&d\end{pmatrix}` | 605 / 372, cpu 202 | 9,646 / 8,114, cpu 3,519 | 5,700 / 4,226, cpu 2,552 | 0.73 |
| `a\,b\;c\quad d` | 746 / 460, cpu 203 | 3,046 / 2,134, cpu 854 | 2,055 / 1,615, cpu 500 | 0.59 |

A second run of `\text{hello world}`, `x_i^2` and `\sqrt{x}` (load 20 to 28, 11 runs): cpu 544 / 480, 1,537 / 1,493 and 1,287 / 1,440
(hybrid / flat): the same as flat within the noise, which is what to expect of these three: `x_i^2` is not in the font (a stacked
pair that no corpus formula has: the sub's shift is -0.259 for an `i`, and so on), `\sqrt` is an SVG and `\text` is stock
text that has the generated font first in its family. What the font does for the formulas that are all glyphs is 0.6 to 0.8 of
the flat design's time: 2.4 to 3.1 times the floor's CPU time for `a+b+c=d`, `x_i`, `x^2` and the spacing formula, where the flat
design has 3.2 to 4.4 times (what is left over the floor is the layout of 3,000 shaped runs).

**Fixture** (400 messages of 12 corpus formulas, 4,800 occurrences, `content-visibility` off, 9 interleaved runs, load 16; settle ms
median / min, main-thread CPU ms median / min): floor 912 / 558, cpu 474 / 379; flat 5,719 / 3,647, cpu 3,264 / 2,591; **hybrid 3,027 /
1,923, cpu 1,769 / 1,406**. The hybrid page is 1,414 KB against the flat page's 1,894 KB (the floor's is 226 KB). The phases
(cpu, ms): parse 350 against 844, style 313 against 649, layout 858 against 1,270, pre-paint 212 against 450, paint 36 against 51.

**Server** (`tools/mfsrv.sh`; the corpus, 1,128 formulas, each rendered 25 times, the median per formula, the minimum over 5 or 7 rounds
interleaved with the flat design's): the step costs nothing, it saves time: it writes fewer elements, so the output stage has less to do
than the markup it replaces. Per formula inline 92 us against 123 us (**0.75**, median per formula 0.83), display 102 against 121 us
(0.85); at load 20 0.71 and 0.70. Every length class is faster (hybrid / flat 0.75 for formulas under 40 characters, 0.71 up to 200,
0.49 for the six longest). A first version that cloned every span twice was 35 to 57% slower on the biggest `aligned` and array
formulas (measured at load 15 to 20: rough), which is why `emit_span` copies a span without its children.

### f. Shaping, selection, copy, find

* **Run clamp, the one surprise.** Chrome splits text into runs by script and a run's total advance is clamped at zero (real Latin,
  Greek and Cyrillic letters, private use, and Common characters such as digits, punctuation and the Mathematical Italic block are
  different scripts). `a`, a spacer of the advance of `a`, `b` (real letters) is 42 px wide at 40 px, the spacer lost, and the spacer
  alone is 0; with `1` or `=` or a private-use glyph in place of one of the letters it works, because the spacers share a run with
  a character that is not a script letter, whose total is not negative (`results/shaping.json`, 30 cases). The GPOS adjustment
  itself is applied. The font has only private-use code points for this reason (no real letters, no twins), and so every text node of
  the encoder is one script run. The clamp is per text node, and nothing else: `tools/neg_spacer_across_elements.py` (40 px, widths
  in px with the ideal in brackets): glyph a, spacer -a, an element with `b`: 22.25 (22.24); `ab`, spacer -(a+b), an element with `a`:
  20.00 (20.00); element `a`, then a text node that starts with the spacer -a and has `b`: 22.25 (22.24); an element, then a text node
  with a leading negative spacer and `ba`: 42.25 (42.24); `a`, spacer -2a, element `b` (the text node's total is negative):
  **22.25 where 2.24 is ideal**: the clamp. So a negative spacer is fine anywhere in a text node whose total advance is not negative,
  and the encoder keeps a group whose total is negative as elements (`flush` in `mathrun.rs`); a neighbouring element or run never
  matters, which is what the corpus-wide comparison above shows too (stock spans beside runs, 1,111 formulas, glyph x within 0.25 px).
* Negative spacers (and everything else) are exact from 6 to 100 px of font size, at device scale 1 and 2 (`widths 6..100 px:
  error <= 0.011 px` with `geometricPrecision`; with `auto` every glyph advance is rounded to a pixel, see above). The same
  under default settings, `font-kerning: none`, `text-rendering: optimizeSpeed`, `font-variant-ligatures: none`,
  `font-feature-settings: "kern" 0` and `"kern" 0, "dist" 0`, `direction: rtl`, `zoom: 1.5`. The lookup is registered under `dist`,
  `kern`, `locl`, `mark`, `rlig`, because a first build that had only `kern` and `dist` was lost under `font-feature-settings:
  "kern" 0, "dist" 0`. `letter-spacing` is added to every glyph including the spacers (`letter-spacing: 1px` makes 30 px of a 22
  px case), so the stylesheet resets it for `.katex.mf`. `word-spacing` does not matter (private-use code points are not spaces).
  `writing-mode: vertical-*` is not supported.
* Line breaking: the formula has `white-space: nowrap` as in KaTeX and the compact output, and the `<wbr>` between bases is the
  flat design's (checked above); inside a text node there is no break opportunity (private-use letters do not break with `normal`
  but do with `overflow-wrap: anywhere` and `word-break: break-all`).
* Selection: one highlight rectangle for the whole text node (ascent + descent = 1.175 em high), a drag selects code points, so a
  partial selection of a stacked script has an overlapping highlight; `crops/selection_dpr2.png`. KaTeX's selection is a ragged set of
  boxes of different heights.
* Copy: the text of a text node is private-use characters (the letters too, the spacers about 4.9 per formula in the corpus: so pasted
  text has invisible junk in it). The TeX is in the `aria-label`; a page that wants the TeX on the clipboard needs a `copy` handler.
  `\text` stays text and copies as text. Find in page cannot find the letters of a text node.

### g. Risks

* **Loading and failure** (`mfshape.py`, `results/shape/fail_*.png`): `font-display: swap` shows the characters of the fallback font at
  once, which for private-use code points are missing-glyph boxes (one per glyph and spacer): ugly. The stylesheet has
  `font-display: block` (text invisible for up to 3 s, then the formula); the optional 344-byte blank face (second argument of
  `stylesheet`, an inline `data:` URI or a file) makes the glyphs draw nothing while the font loads or if it fails; it has no
  measurable cost in layout time (see "Where the time goes"); `mfshape.py` has the screenshots. The page text reflows when the font arrives (the
  formula is inline and changes width). A truncated or missing file is `status: error`: no formula is readable then (the `aria-label`
  has the TeX, a screen reader is not affected, a sighted reader is). Stock text, stacks and bold letters are not in the font and show.
* **Other browsers** were not tested (no Firefox, Safari or Edge here). To check: the GPOS single adjustment with negative XAdvance,
  per-script run splitting (the clamp is Blink's; HarfBuzz is shared with Firefox, which shapes per script run as well),
  `text-rendering: geometricPrecision` meaning sub-pixel positioning (on a browser that always has it, KaTeX's own layout is the
  ideal one and the font agrees with that), composite glyphs with a scale and large offsets in Core Text and DirectWrite, WOFF 2
  with transformed glyf. Windows with hinted KaTeX originals will look slightly different from the unhinted outlines.
* **The page must have `text-rendering: geometricPrecision` on the formula** and not override `letter-spacing`, `font-family`,
  `font` on `.katex.mf`; `mathrun::stylesheet` has them. On Linux Chrome `auto` rounds every glyph to a pixel and a formula comes out up
  to 40 px too wide or narrow.
* **No per-glyph styling or search**: a text node is a string. A colour or `\textcolor` is a span with a run inside, which is fine;
  restyling single atoms from CSS is not possible.
* **Print and zoom**: sizes are in em, so zoom is exact (checked at `zoom: 1.5`); print was not compared. Forced-colours and
  `-webkit-text-stroke` apply to the text nodes.
* **What the font lacks** (a `(size, shift)` pair of a script, a glyph, another font) stays an element, so the compact stylesheet
  and the KaTeX fonts still ship; the font has the 74 pairs of the corpus, and a formula the corpus did not have is covered
  by the pairs it did, 3 to 4 points of the glyphs less (b above). A stacked sub and superscript are the common
  case that varies: the shift of the sub depends on the height of its glyphs, `x_i^2` has -0.259 for an `i`.
* **The font version is part of every stored formula**: a change to the glyph order changes the code points, so a stored formula is
  only valid with the font it was written for (`FONT_VERSION` is in the file and family name; the version is a hash of the map). A
  consumer that stores formulas must store the version, or regenerate them when it changes.
* **The font is derived from one corpus**: the pairs and spacers come from this corpus (the formulas of the compact work's
  corpus), see the held-out numbers; another domain wants `tools/mkfont.sh` run on its own formulas.

## Regenerating

`BROTLI=/usr/bin/brotli docs/font-measurements/regen.sh` is the whole sequence (it needs the scratch directory layout described in it,
KaTeX 0.18.5's TTFs, the corpus and Chrome; about an hour). To build only the font and the table from the committed spec
(`data/math-font/spec.json`; `-I` because the tools are not to import from the current directory):

```sh
BROTLI=/usr/bin/brotli python3 -I crates/katex/tools/gen_math_font.py --fonts DIR_WITH_KaTeX_0.18.5_TTFs \
    --spec crates/katex/data/math-font/spec.json --out crates/katex/data/math-font --name KaTeXMathRun
python3 -I crates/katex/tools/gen_mathrun_table.py          # src/generated/mathrun_table.rs from the new map
```

Both are deterministic: they reproduce the committed `.woff2`, `.map.json` and table byte for byte (checked: `mkfont.sh all 2` from the
corpus, and the two commands above from the spec). The font file name has the version, so after a change delete the old `KaTeXMathRun-*`
files (`gen_mathrun_table.py` wants exactly one map). A change to the glyph order or the advances changes the version, and with it the
stored output of every consumer that keeps formulas: serve the old font under its old name as long as such output exists. For another
corpus: `tools/mkfont.sh all 2` with its own `data/` (`mfextract.py` of its formulas).

## Not done

* Firefox, Safari and Windows were not looked at (see Risks); only headless Chrome 156 on Linux.
* The accent overlays and rule glyphs are in the font (2.4 KB of WOFF 2) and unused: KaTeX writes accents as stacks and a fraction's bar
  as a border; using them would need a node-level rule for "base letter plus accent" and a rule glyph for borders.
* Stacks (fractions, matrices, the SVG of roots) stay elements with text in their rows: a text node has no way to lay rows out
  vertically, so what is left of a formula after this step is the skeleton of its stacks.
* The shifts of stacked scripts that the corpus did not have (see Risks); a table by (base height, script height) would cover them at a
  few hundred more variants.
* Text mode as glyphs (`TEXT_AS_GLYPHS`, off): 98.9% of the glyphs instead of 88.7%, at the price of private-use characters on copy and
  3 bytes a character; a decision for the consumer.
