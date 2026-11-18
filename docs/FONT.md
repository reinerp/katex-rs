# Maths layout in a generated font

KaTeX lays a formula out with elements: a font switch is a span, spacing an empty span with a margin, an italic correction a
margin on a letter, a script a size span and a vertical shift. A browser pays for each element (about 10 microseconds a formula
element for everything it does, `COMPACT.md`). This is a font that has that layout in its glyphs, and an encoder in the Rust crate
that writes a formula with it: wherever the font can express the nodes of a formula, they are **one text node** of its code
points, and wherever it cannot, they stay the elements the compact output writes, in the same tree. It is the compact output's
`levels::FONT_RUN` step, off by default (the page must serve the font).

```html
<!-- a+b+c=d: 7 characters (the spacing is in the glyphs); it was 31 -->
<span class="katex mf f" role="math" aria-label="a+b+c=d" style="--h:.7778em;--v:-.0833em">⟨E199⟩⟨ED73⟩⟨E19A⟩⟨ED73⟩⟨E19B⟩⟨ED7F⟩⟨E19C⟩</span>

<!-- f(x)=\alpha x^2+\beta: it wraps, so its line breaks are U+200B between its bases (and the element of the strut) -->
<span class="katex mw mf t11" role="math" aria-label="f(x)=\alpha x^2+\beta">⟨E19E⟩⟨E061⟩⟨E1B0⟩⟨E062⟩⟨ED7F⟩<span class="mu"></span>⟨200B⟩⟨E1BF⟩⟨E1B0⟩⟨F0BF⟩⟨ED73⟩⟨200B⟩⟨E1C0⟩</span>

<!-- a + \frac{x^2}{y_i} = \mathbf{b}: text, a stack whose rows are text, text, a span in a stock KaTeX font; a stack keeps <wbr>.
     634 bytes and 16 elements (the flat design: 745 bytes, 21). ⟨XXXX⟩ is a private-use character (or U+200B) -->
<span class="katex mf f" role="math" aria-label="a + \frac{x^2}{y_i} = \mathbf{b}" style="--h:1.499em;--v:-.4811em">⟨E199⟩⟨ED73⟩<wbr><span class="nulldelimiter"></span><span class="mfrac"><span class="X" style="padding-top:1.0179em;height:.4811em;vertical-align:-.4811em"><span class="W k1" style="top:-2.655em"><span class="F63">⟨E1B1⟩<span class="F31 zd">⟨E1A1⟩</span></span></span><span class="W k1 iu"><span class="frac-line x0"></span></span><span class="W k1" style="top:-3.394em"><span class="F63">⟨E1B0⟩<span class="F31 zk">⟨E06B⟩</span></span></span></span></span><span class="nulldelimiter"></span>⟨ED7F⟩<wbr><span class="mathbf">b</span></span>
```

Every formula gets the font for the maximal parts it can express and markup for the rest. `crates/katex/src/mathrun.rs` is the encoder, `compact::levels::FONT_RUN` the switch, `docs/font-measurements/` the scripts and
numbers. What it gives (the 1,128-formula corpus, "Results" below): **88.7% of the glyphs in text nodes (98.9% if `\text` is
written as glyphs too), 11.1 characters a formula (1.05 a glyph: the spacing between the atoms is in the glyphs), 2.7 elements a formula
where the flat design has 9.3, 15% fewer bytes in the DOM than the first version of this design and 40% fewer than the flat design, 0.45 to 0.85 of the flat design's browser time
for the formulas that are all glyphs** (3 to 3.7 times the floor of the same text as plain text), no `<wbr>` element in a formula that is text
and inline spans, and ideal TeX positions (glyph x within 0.28 px of KaTeX JS, the median formula within 0.02 px; KaTeX's own rounding
error is the larger part of the difference), for a font of **75 KB** (WOFF 2) that has to be served.

## Files

| path | what |
| --- | --- |
| `crates/katex/src/mathrun.rs` | the encoder (the tree transform), the stylesheet and the accessors of the font |
| `crates/katex/src/generated/mathrun_table.rs` | the layout numbers and the repertoire masks (`tools/gen_mathrun_table.py` from the font's map, which checks them against it), checked in |
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
4. **Spacing in the glyphs** (`BAKED`): a composite of the base glyph at an x offset, with the advance of the glyph plus a gap before it and
   a gap after it. For every glyph of Main-Regular and AMS that KaTeX's symbol table puts in a class that TeX spaces: binary operators
   (medium space before and after: `(0.2222, 0.2222)`, `(0.2222, 0)` and `(0, 0.2222)` em), relations (thick space: 0.2778 em the same
   three ways), punctuation (thin space after: `(0, 0.1667)`); and for the glyphs of the three commonest script variants (0.7 at -0.15, 0.363
   and 0.413 em) one with KaTeX's script space (0.05 em) after it. 1,071 glyphs, 352 of them for spacing.
5. **Spacers**: empty glyphs with exact advances: 23 named ones, the distances that the encoder still writes after the glyphs carry the spacing
   (chosen from the corpus, see "Spacing"), and a binary set of +-2^k units (k = 0..14), so any kern is a few characters. A negative
   spacer has advance 0 in `hmtx` and a negative `XAdvance` from one GPOS single-adjustment lookup (the only lookup of the font, registered
   under `dist`, `kern`, `locl`, `mark` and `rlig`, so that a page that switches kerning off does not lose it).
6. **Two real code points**: U+200B (a line break opportunity that `compact` writes) and U+202F (the content of the strut of a formula that wraps), both
   empty and without an advance, so that the browser does not ask another font for them (another font's ascent and descent would be in the line).
7. Accent overlays (two-component composites) and rule glyphs (a unit rectangle scaled in x and y) are in the font and are not used by
   the encoder yet (accents are stacks in KaTeX's tree; a fraction's bar is a border).

Vertical metrics: `hhea` and `OS/2` ascent and descent are those of KaTeX_Main (0.903 and 0.272 em), so a line that holds a formula
has the height it has with KaTeX's own fonts (the font is the first of the family, and `.katex` keeps its `line-height: 1.2`).

### The tree transform (`mathrun::transform`)

The built DOM tree of the formula, before `compact` rewrites it, is read the way a browser lays it out, in em of the element
that holds the text: inline flow, `em` margins and `top` offsets, `vlist` tables (a script or limit is an item at `top` below the
baseline by the height of its `pstrut`), `rlap`/`llap` (zero width), centred limits. In every list of sibling nodes:

* consecutive nodes the font can express become one text node: each glyph is its code point, and where it is not where the
  previous glyph ends, a glyph that carries the gap or spacers close it (a negative spacer moves the cursor back, which is how
  stacked scripts are one run: the superscript, a spacer back, the subscript, a pad). Which, is decided for the whole run, by the fewest
  code points: for each glyph its plain code point or one of the glyphs of `BAKED` for it, and for what is left of a gap a named
  spacer and binary ones (a dynamic programme over the glyphs, `mathrun::encode`: a gap is written once, the glyph on its left that can carry it
  does, and the one on its right uses its plain glyph or the one with no gap before it). Glue at the ends of such a group is
  absorbed (it ends the line before a break as before);
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

The break opportunities between bases and `compact_wrap_min` are the flat design's (the count is of the characters KaTeX has, not of code points).
A text node's total advance is never negative (Chrome clamps it, below): a group whose width is negative stays elements.

#### Line breaks

KaTeX allows a break between its `base` blocks, and the flat design writes a `<wbr>` there (in an element with `white-space: nowrap`, which does not
stop it). A `<wbr>` costs about as much as a span. When a formula that is wide enough to wrap (`compact_wrap_min`, inline) is **text and inline
spans only**, its breaks are **U+200B between its bases**, and the formula is `white-space: normal` (the class `mw`):

* nothing else can break in it. In a run of private-use code points a browser has no break opportunity (they are all letters for it, tried for
  all 6,400, next to each other and to Latin letters); the spaces of stock text (`\text`) are no-break spaces, which look the same; the spans
  are `white-space: nowrap` (`.katex.mf.mw span`) and the formula is `overflow-wrap: normal; word-break: normal; line-break: auto` (a page that
  has `overflow-wrap: anywhere` on its messages would break a long segment otherwise: 6 of the 33 targeted formulas and 2 of the 322 that wrap
  at 300 px did);
* **what keeps `<wbr>`** (and `white-space: nowrap`), because a browser does allow a break there and nothing stops it: a formula that has an
  inline block (a stack, an SVG, a table: Chrome 156 allows a break before and after one whatever the parent's `white-space` is, and a word joiner
  next to it does not stop the break before it), and one that has stock text with an opening bracket, a hyphen, a question mark, another
  non-ASCII character (a break after a hyphen, a question mark, the dashes and the ellipsis and before a bracket that follows
  punctuation: tried for every pair of ASCII punctuation marks). In the corpus 416 of the 470 formulas that wrap are written with U+200B
  (986 of them, instead of 1,105 `<wbr>`), 54 keep 119 `<wbr>`; 52 of those have a stack or an SVG, 2 only stock text with `(`, `-` or `?`;
* the formula's **strut** is an element after its first segment, `<span class="mu"></span>` (an empty inline block with the height and
  vertical alignment of the root's `::before` in the other formulas, from the same rules): the root's `::before` is an inline block, a browser may break
  after one, and the formula's first segment would go to the next line and leave the strut at the end of the previous (a first line 5 px
  taller than KaTeX's; a strut that is the first thing of an element wrapped round the first segment was no better: the wrapper is one more
  inline block per formula, 15 microseconds on a page of 3,000 of them). With the strut after the first segment, the heights and baselines of the
  paragraphs of all 1,128 corpus formulas at 300 and 760 px are those of the `<wbr>` output;
* in the font U+200B has a glyph (no advance), so it is not drawn by another font; in a shaping run it is a character like the others and splits
  nothing. A browser reports the rect of a character next to a zero-width space as a half of the pair (a cluster), which matters
  to whoever measures glyph rects (`mfpos.py`), not to what is drawn.

### The stylesheet

`mathrun::stylesheet(font_url, blank_url)` is `compact::stylesheet()` plus the `@font-face` of the font and, for `.katex.mf`, the
generated font first in `font-family` (the stock KaTeX_Main after it) with `text-rendering: geometricPrecision` and
`letter-spacing: normal`, and a rule `.katex.mf <class>` for each class of `katex.css` that sets a font family (`.mathnormal`,
`.amsrm`, `.delimsizing.size1`, `.op-symbol.small-op`, ...) with the generated font first. So a character that is not in the
generated font (every real character: stock text, bold letters) is drawn by the stock font of its element as before, and a glyph
run inside a stock-font span is drawn by the generated font. The stock `@font-face` rules are those of the compact stylesheet:
a browser fetches only the faces a page uses. For `.katex.mf.mw` (a formula that wraps at U+200B) it has `white-space: normal` and the
`overflow-wrap`, `word-break` and `line-break` that a page's rules cannot change, `white-space: nowrap` for its spans, no `::before` and the
strut rules for `.mu` (a copy of those for `::before`, per strut class).

### What the consumer must do

1. Enable the step: `Settings::builder().compact_levels(levels::ALL | levels::FONT_RUN)` (it needs `FLAT` and `UNWRAP_HTML`, which
   `ALL` has; without them the step does nothing).
2. Serve the font: `mathrun::FONT_WOFF2` under the name `mathrun::FONT_FILE_NAME` (it has the version in it: a stored formula is only
   valid with the font it was written for, and `mathrun::FONT_VERSION` says which) with a long cache lifetime, and use
   `mathrun::stylesheet(url_of_the_font, None)` instead of `compact::stylesheet()`. Optionally the 344-byte fallback face
   (`mathrun::BLANK_FONT_WOFF2`, second argument), which makes the spacers draw nothing while the font loads or if it fails.
3. Keep `text-rendering: geometricPrecision` on formulas (the stylesheet does it); a page rule that sets `text-rendering`,
   `letter-spacing`, `font-family` or `font` on `.katex` would change the output.
4. The text of a formula that is in the font is private-use characters when selected or copied (and U+200B where it can wrap); the TeX is in
   the `aria-label` (`compact_label`). A page that wants the TeX on the clipboard needs a `copy` handler.

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
big-operator positions, one per kind of base. Accents are rare in this corpus (3 occurrences, `\hat` and similar in none with a single-glyph
base), so the 468 accent composites are for completeness. (Spacers are "Spacing in the glyphs and the named spacers", below; the first version
of the table of named spacers had every advance at half its value, so it was hardly used: that is how `a+b+c=d` came to be 31 characters.)

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

### Spacing in the glyphs and the named spacers

`tools/mkfont.sh` (step 2 and 3), `mfgaps.py`, `mfscriptspace.py`, `mfnamed.py`. The encoder wrote every gap between atoms as spacers: `a+b+c=d` is
seven glyphs and 24 spacers (four binary ones around each `+`: 31 characters in all). What the corpus has (the glyph runs the encoder wrote for the 2,256
renderings, gaps in font units, 4,000 to the em; TeX's thin, medium and thick space are 667, 889 and 1,111, the script space after a script 200):

| class (KaTeX's symbol table) | occurrences | gaps before and after (the glyph's own, in the run) |
| --- | --- | --- |
| relation | 3,360 | thick/thick 1,928; script space + thick/thick 840; thick (a gap at the start of the run) /thick 336; 0/thick 104; none 68 |
| binary operator | 3,076 | medium/medium 1,476; script space + medium/medium 1,120; 0/0 204 (an operator at the start of a formula is an ordinary atom); medium (start)/medium 120 |
| punctuation | 3,648 | 0/thin 1,556; 0/0 984 (the comma in `1,000`); script space/0 388; script space/thin 316; 0/quad+thin 172 |

So a relation or a binary operator is one glyph with its space on both sides, and the script space is a gap of its own that follows a script glyph
(5,644 of them follow a script glyph: 4,348 the subscript at -0.15 em, 540 the superscript at 0.413, 496 at 0.363, 3 other pairs the rest; the
three pairs are 95% of them). A gap belongs to **one** glyph:

* the encoder decides for the whole run, by the fewest code points, a dynamic programme over the glyphs (`mathrun::encode`): each glyph is its plain code
  point or one of the glyphs of `BAKED` for it (left gap, right gap), what is left of a gap is a named spacer and binary ones (`spacer`). So where two
  glyphs that carry a gap meet, the one on the left carries it (its right gap) and the one on the right uses the glyph without a left gap, as it
  does after a letter; a relation after a script is the script's glyph with the script space, the relation with its thick space on both sides: no spacer;
* the glyph set is every glyph that the symbol table puts in a class, not those of the corpus: 69 relations and 35 binary operators of Main-Regular,
  11 and 1 of AMS, 4 punctuation marks: 3 glyphs each for the classes with two gaps (352 in all), and the script glyphs of the three pairs (719: all that the font has at those pairs). A glyph costs a few
  compressed bytes (the font is 2,232 bytes larger, 3.1%, with 1,080 more glyphs);
* positions are those of spacers: a baked glyph is the base glyph at x = the left gap and has the advance of both gaps, so the cursor is where it
  would be. The encoder writes a gap to within one font unit (a remainder of one unit is not written, an odd one is made even), so a glyph is within a
  unit or two of the position the layout gave it, as before: of the 3,365 glyphs of the display runs that could be matched with the positions the encoder was given, the old
  output has 2,396 exact, 832 one unit too far right and 137 one too far left, the new 2,814, 457 and 94. The positions of old and new differ by
  at most 3 units for every glyph of the inline formulas and for 99.3% of those of the display ones (up to 8 in the longest runs, where the rounding of
the trailing gap of each of the runs that are one text node adds up; see "h").

What is left after that are 0.56 spacers a formula, 0.45 of them named: the 23 named spacers are chosen greedily from the distances the encoder
wrote spacers for (115 different ones in the corpus), the one that saves most code points first, as long as it saves 30 on the corpus (200 and the thin
and medium space after a script when a glyph has no variant, 1,089 and 1,311 (script space and medium or thick), the quad and the `\qquad` with the
thin space, the negative ones of stacked scripts, ...). The binary spacers write the long tail: 0.11 a formula.

(The 16 named spacers of the first version of this design had every advance at half the value they were chosen for, so they matched
nothing: that is a part of why `a+b+c=d` was 31 characters.)

### Layout: the code points follow a rule (font `bb64ff11`)

The first fonts had a table: for every base glyph, every script variant and every glyph with spacing, its code point (and advance) were listed in `mathrun_table.rs`
(`BASE`, `VARIANTS`, `BAKED`: 93 KB of source tables, 14 KB of the WebAssembly bundle after Brotli). From `bb64ff11` the generator allocates code points by a rule that
`mathrun.rs` computes, so the table is a few dozen numbers and the repertoires of the script variants as bit masks (`spec.json` has `"layout": "implicit"`):

* the planes follow one another in the private-use area (U+E000..) in this order: positive spacers; the base glyphs of Main-Regular, Math-Italic, AMS-Regular and Size1-4
  (`BASE_START + g`); the glyphs without the italic correction (`NOIC_START + rank` in Math-Italic, with holes); the script variants of every (size, shift) pair, the pair with most
  glyphs first; the glyphs with spacing (a plane per class and gap pattern, one per script-space pair); the negative spacers (last, they have no entry in `hmtx`). 4,574 slots
  of the 6,400 of the BMP area; nothing is in plane 15;
* the **rank** of a base glyph is its index in the metrics table the renderer already has (`FontMetricsData::rank_metric`; AMS-Regular: in `AMS_CHARS`, the 47 characters of its subset), so
  `(font, character)` to code point needs no table: `g = FONT_OFFSET[font] + rank`, code point `BASE_START + g`. The font has glyphs only for characters that have metrics (the 8 base glyphs
  of the previous font that did not, U+2002..U+200A and one private-use character of Size4, were never used in the corpus);
* a script variant is at the pair's plane start plus the rank of `g` in the pair's repertoire (`PAIR_MASKS`: the bits of g, popcount below g); a glyph that carries TeX's spacing at the
  class pattern's start plus the rank of `g` in the class mask (`SPACING`); one with the script space after it likewise in the pair's script-space plane;
* **advances are derived**: a base glyph's is `round(width * 4000) + italic correction + ADV_CORR[g]` (the correction is the difference between the font's advance, which comes from the TTF, and
  the rounded advance of the metrics table: -2..2 in 565 of 580 glyphs, the others are glyphs whose metrics have no width, spaces and the like; it keeps every glyph where it was), a script variant's `round(advance * size)` (the generator's rule), the glyph without the correction
  `advance - correction`; spacers: `NAMED` (23 advances, sorted; the negative ones first at `NEG_NAMED_START`) and `POS_BIN_START + k`, `NEG_BIN_START + k`.

`gen_mathrun_table.py` derives every code point and advance of the map by this rule and stops if one differs; `mathrun::layout_tests` do the same in Rust for the whole font (580 base glyphs,
2,763 variants, 1,071 glyphs with spacing, the spacers). The positions of all glyphs are those of `794995a4` (`docs/font-measurements/tools/fontcmp.py` compares the maps: advances, italic corrections,
variants and gaps of every glyph, 0 differences), and the output of the corpus, inline and display, with and without `compact_line`, is the old output with each glyph's code point replaced
by the same glyph's in the new font (`tools/cpcmp.py`: 1,128 formulas, 12,558 glyph code points, 0 differences). The WOFF 2 is 72,508 bytes against 75,016 (74,412 with the rule and overlay
glyphs, which nothing uses, kept: `keep_unused`); the encoder is as fast as before (a binary search in the metrics table in place of one in `BASE`; 679 against 687 ms for the fixture's 4,800 formulas in node, minimum of 20 runs).

## Results

Measured on the shared machine, headless Chrome 156 on Linux over CDP, KaTeX JS 0.18.5, the 1,128-formula corpus of the
compact work (`~/scratch/katex-harden/data/formulas.jsonl`: 1,127 render, one is a KaTeX error; 365 of them are display
formulas in the corpus), a 16 px body so the formula is 19.36 px. The Rust output is the `FONT_RUN` output of
the commit that has this file. "Flat" is the compact design without the step (`levels::ALL`, `d6a1d304`'s default
output). Other agents ran Chrome on the same machine the whole time (load average 4 to 28), so timings are interleaved and the
minimum is next to the median; a difference under 15% is noise. The raw files are in `docs/font-measurements/results/`.

### a. The font file

The committed font (`KaTeXMathRun-794995a4`): 5,016 glyphs, **WOFF 2 75,016 bytes** (the same file zstd -19: 73,904; the
untransformed WOFF 2 87,748, WOFF 1 114,572, TTF 234,720), 5,012 private-use code points (of the 6,400 of U+E000..U+F8FF) and U+200B and U+202F. 588 base
glyphs (Main-Regular, Math-Italic, a 70-glyph subset of AMS, Size1-4), 59 without the italic correction, 2,763 script variants of 74 (size, shift) pairs,
1,071 glyphs that carry spacing, 53 spacers (23 named, 2 x 15 binary), 468 accent overlays and 11 rule glyphs that the encoder does not use (2,372
bytes of the WOFF 2: 72,648 without them). Before the spacing glyphs, the named spacers and the two break characters (`e9c82a06`): 3,936 glyphs, WOFF 2
72,784 bytes. For scale: KaTeX's own WOFF 2 of the same repertoire are 90 KB.

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

`tools/mfshare.py`, `mfstock.py`. Of the 13,411 glyphs of the 1,127 inline formulas, **11,896 (88.7%) are in text nodes** (unchanged by the spacing: the glyphs are the same); of
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

Elements per formula (spans, svg elements and `<wbr>`): flat 9.33 on average, median 5; hybrid before the line breaks as characters 3.16; **now 2.66,
median 1**, p95 9 (display style, all 1,128 formulas: 9.54 to 3.21, median 6 to 2: no line breaks there; the 365 display formulas of the corpus: 5.57).
An element of the flat design that is in the way of nothing (a base, a sizing span, a margin span) is a code point or a spacer now: the
stacks (fractions, matrices, accents) are what is left, with text in their rows.

Bytes per formula, raw: flat 339 (median 178); hybrid before the spacing in the glyphs 239 (134); **now 203 (111)**; display style 369, 270, **226**. zstd -19 of all the
formulas as one stream (what a connection that compresses the whole stream gets; the markup of the flat design is mostly
boilerplate that repeats from formula to formula, so it compresses to almost nothing): 28.65 bytes a formula flat, 29.07 hybrid before, **28.13
now** (display 28.66, 29.30, **27.68**). The private-use characters compress worse than the markup does, so on the wire the three are the
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
* **What a formula that wraps at U+200B asks of the page**: nothing, the stylesheet sets `white-space`, `overflow-wrap`, `word-break` and `line-break` on it and
  `white-space: nowrap` on its spans (a page rule that sets them on `.katex` or on a span of it at a higher specificity would change where it wraps). A host that reads the
  formula's text gets U+200B in it, and the rects of a character next to one are half of a pair (measure the glyphs of such a formula without them: `mfpos.py` does).
  The break opportunities are the ones KaTeX has between its bases; no other is possible in what is written as text (checked on the corpus and on 34 formulas with
  the constructs that have one: `\text` with spaces and hyphens, brackets next to each other, slashes, bars, commas, stacks).
* **The font version is part of every stored formula**: a change to the glyph order changes the code points, so a stored formula is
  only valid with the font it was written for (`FONT_VERSION` is in the file and family name; the version is a hash of the map). A
  consumer that stores formulas must store the version, or regenerate them when it changes.
* **The font is derived from one corpus**: the pairs and spacers come from this corpus (the formulas of the compact work's
  corpus), see the held-out numbers; another domain wants `tools/mkfont.sh` run on its own formulas.

### h. Spacing in the glyphs, named spacers, line breaks as characters

The second hybrid stage (this commit range). "Old" is the output of the commit before it (`583ecb09`, the font `e9c82a06`), "new" the output now
(font `794995a4`); both are `FONT_RUN` output, the same formulas, the same tree. Raw files in `results/hybrid/` (`h_*`).

**Characters, bytes, font.** The 1,128 corpus formulas, inline and in display style (private-use characters in the text; "glyphs" are the characters that are
not spacers: the same number in both):

| | old | new |
| --- | --- | --- |
| characters a formula, inline: mean (median) | 25.80 (17) | **11.13 (8)** |
| characters a visible glyph, inline | 2.441 | **1.053** |
| spacers a formula, inline (named ones) | 15.23 (2.49) | 0.56 (0.45) |
| characters a formula, display: mean (median); per glyph | 26.01 (17); 2.460 | **11.20 (8); 1.059** |
| bytes a formula, raw, inline: mean (median) | 239.2 (134) | **203.4 (111)** |
| bytes a formula, raw, display: mean (median) | 270.2 (172) | **225.8 (138)** |
| bytes a formula, zstd -19 of the corpus as one stream, inline / display | 29.07 / 29.30 | **28.13 / 27.68** |
| elements a formula (span, svg, `<wbr>`), inline: mean (median) | 3.16 (1) | **2.66 (1)** |
| `<wbr>` elements, inline (formulas that have one) | 1,105 (470) | 119 (54) |
| U+200B characters, inline (formulas) / `mu` elements | 0 | 986 (416) / 416 |
| the font: WOFF 2 / zstd of it / glyphs | 72,784 / 71,821 / 3,936 | 75,016 / 73,904 / 5,016 |

**Positions** (`mfpos.py`, `mfunits.py`; headless Chrome 156, Range rects of every glyph, 19.36 px font): the glyphs of the old and the new output are the same in all 1,128
inline and all 1,128 display formulas (the same source character, size and shift in the same number, in the same order), and the stock text is the same characters.
In font units (4,000 to the em), computed from the code points, the glyph positions differ by -3 to +2 (inline: 5,039 equal, 1,321 one unit less, 487 one more; none more than 3) and
by -8 to +6 (display, where the runs of all the bases of a formula are one text node and the trailing gaps add up: 99.3% within 3). Both are within a unit or two of
the positions the encoder was given (above), the new ones nearer. In Chrome, whose rects are in 1/64 px (0.0156): median 0.013 px, p99 0.047, **max 0.078 px**
(inline; display 0.010, 0.031, 0.047); 427 of the 1,128 inline formulas (315 display) have a glyph 2/64 px or more from where it was. Formula width: max 0.078 px inline
(95 above 1/64 px), 0.031 px display (6). Not zero, and not the encoder: the zero-width spaces make a text one text node where `<wbr>` made N, and (probably) the
width of every text fragment is rounded up to 1/64 px, so the new output is up to 4/64 px narrower than the old one (with the same encoder and `<wbr>` instead of U+200B it
is within 1/64 px of the old: `tools/exp_w.py`, 0.0156 px at most, 0.003 px on average); against KaTeX JS the new glyph x is closer (below). Paragraph height, baseline, element
heights: no formula differs by more than 0.02 px. Against KaTeX JS 0.18.5 in `geometricPrecision` (`mfhyb.py`, as in "c"): glyph x median 0.016, p95 0.062, p99 0.109, max 0.281 px
inline (old: 0.016, 0.078, 0.125, 0.250); display 0.031, 0.062, 0.094, 0.156; widths max 0.27 px inline, 0 display.

**Pixels** (`mfold.py`: old and new rendered side by side, rows of 80 px, device scale 1 and 2): inline: identical in 42.0% / 63.9% of the 1,128 formulas (scale
1 / 2), no pixel more than 64/255 different in 93.5% / 82.8%, median 8 / 0 differing pixels; display: 41.9% / 62.6%, 97.8% / 89.5%. Where they differ a glyph is one
quarter-pixel step to the left or right (a unit or two in the position can cross the step Skia draws glyphs on), the shapes are the same: the worst formulas have 723 differing
pixels with at most 71/255 (`T_\downarrow(x)=x\oplus(x\gg1)\oplus...`: a row of `\oplus`). 45 formulas (every 28th of the corpus and the five with the most differing pixels)
side by side with the difference in red, scale 1 and 2: `results/crops/old_new_p*_dpr*.png`; nothing in them is different but those steps.

**Line breaks** (`mfwrap.py`, a 16 px paragraph "Some words before, FORMULA and some words after, ..." of the given width, the source glyph that starts every line of the formula,
against KaTeX JS in `geometricPrecision`, as before; `results/hybrid/h_wrap.txt`):

| | formulas that wrap in KaTeX | same line starts | wrap only here | other line count |
| --- | --- | --- | --- | --- |
| corpus, 300 px | 322 | **322** | 0 | 1 (as before) |
| corpus, 760 px | 19 | **19** | 0 | 0 |
| corpus, 300 px, the host has `overflow-wrap: anywhere; word-break: break-word` | 322 | 322 | 0 | |
| 34 targeted formulas, 60 / 90 / 120 / 150 / 200 / 250 px | 33 / 33 / 33 / 33 / 31 / 33 | all | 0 | 5 / 1 / 0 / 1 / 0 / 0 |

The targeted formulas (`tools/targeted.tex`) have a stack, `\sqrt`, an arrow, a matrix, `\text` with spaces, hyphens, a question mark and brackets, `)(`, `/`, `|`, commas,
`\mathbf`/`\mathcal`/`\mathtt`, colour, sizes, an accent, `\boxed`, spaces and `\quad`s, scripts: 22 of them are written with U+200B, 11 with `<wbr>`, one is too short to wrap (`x_1, x_2, ...`). The
flat design (`<wbr>`, nowrap) has the same line starts at every width. The paragraph heights and baselines of the 1,128 corpus formulas at 300 and 760 px are those of the old output (15 and 0
baselines off against KaTeX, as before). What is not the same as the old output: in a column narrower than a formula's **first segment** (60 px: 5 of the 34 targeted
formulas, 90 px: 1, 150 px: 1, and 3 more formulas of the 1,128 at 150 px, 1 more at 200) the zero-width strut after that segment does not fit on its line and is alone on the next one
(then the next segment): a blank line more than KaTeX and than the old output. At 300 px and above it does not occur in the corpus.

**Speed**, interleaved (`measure3.py`: 9 runs of each page in turn; settle median / minimum in ms and the minimum of the main thread's CPU time for parse, style, layout, pre-paint and paint;
3,000 paragraphs "Step i: we have FORMULA here." in 760 px; load average 2.7 to 4.1: a quiet machine this time). Floor is the formula's characters as text, flat the compact output without the step, old and
new the hybrid before and after:

| formula | floor | flat | old hybrid | **new hybrid** | new / old (cpu) | code points old -> new | elements old -> new |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `a+b+c=d` | 97 / 89, cpu 57 | 456 / 444, cpu 360 | 307 / 296, cpu 228 | **282 / 278, cpu 210** | 0.92 | 31 -> 7 | 1 -> 1 |
| `x^2` | 96 / 86, cpu 58 | 330 / 323, cpu 248 | 287 / 277, cpu 209 | **279 / 275, cpu 208** | 1.00 | 5 -> 2 | 1 -> 1 |
| `x_i` | 112 / 90, cpu 59 | 341 / 324, cpu 251 | 301 / 271, cpu 214 | **282 / 268, cpu 207** | 0.97 | 5 -> 2 | 1 -> 1 |
| `f(x)=\alpha x^2+\beta` | 140 / 118, cpu 80 | 659 / 641, cpu 543 | 366 / 357, cpu 276 | **329 / 318, cpu 247** | 0.89 | 24 -> 10 | 3 -> 2 |
| `a\,b\;c\quad d` | 118 / 98, cpu 64 | 438 / 411, cpu 333 | 303 / 272, cpu 214 | **285 / 276, cpu 213** | 1.00 | 18 -> 7 | 1 -> 1 |
| `(g_2, k_2) \circ (g_1, k_1) = \big(g_2 \cup ...` (83 characters, 5 breaks) | 160 / 120, cpu 72 | 1986 / 1745, cpu 1457 | 590 / 497, cpu 401 | **411 / 354, cpu 279** | 0.70 | 99 -> 35 | 6 -> 2 |
| `x_i^2` (not in the font) | 107 / 91, cpu 61 | 683 / 655, cpu 554 | 690 / 642, cpu 547 | **677 / 650, cpu 553** | 1.01 | 3 -> 3 | 6 -> 6 |
| `\frac{a}{b}` | 105 / 89, cpu 60 | 856 / 811, cpu 695 | 882 / 812, cpu 692 | **861 / 814, cpu 691** | 1.00 | 2 -> 2 | 11 -> 11 |

(Settle: median / minimum ms; cpu: minimum of the main thread's parse + style + layout + pre-paint + paint, ms. `bench4/`, `results/hybrid/bench4_micro.jsonl`.) The glyph-only formulas
are 3.1 to 3.7 times the floor's CPU time (flat: 4.3 to 6.3), `\sqrt` and the stacks are what they were. What the characters cost is little (24 of 31 characters fewer is 8% of
the time of such a page); what a line break costs is not: a `<wbr>` is an element, the zero-width space is a character. Formulas with 1 to 4 breaks (`abcde=fgh`, `abcde=fgh+i`, `abcde=fg+h+i`, `abcde=fg+h+i+j`, and two with 2), the same text in three
writings (`results/hybrid/h_bench_breaks.jsonl`): old / the new encoder with `<wbr>` / the new one with U+200B, the CPU minimum in ms: 1 break 246 / 253 / 250, 2 breaks 285 / 275 / 248, 3 breaks
317 / 290 / 252, 4 breaks 345 / 323 / 260 (the two others with 2 breaks: 286 / 269 / 254 and 273 / 263 / 250), so the zero-width space costs what a character costs and a `<wbr>` about 5 microseconds
(a formula with one break is the same in all three). A first version that wrapped the first segment in an inline block to keep the strut with it cost about 15 microseconds a formula and
was slower than `<wbr>` below 3 breaks; the strut as an element after the first segment is an empty inline block, as the root's `::before` was.

The fixture (400 messages of 12 corpus formulas, 4,800 occurrences, `content-visibility` off, the messages `overflow-wrap: anywhere`, 9 interleaved runs, load 3.6; settle median / min,
main-thread CPU median / min, ms): floor 217 / 201, cpu 155 / 140; flat 1,210 / 1,142, cpu 1,025 / 986; old 736 / 687, cpu 633 / 572; **new 748 / 684, cpu 633 / 572**. Parse 141 against 140, style 82
against 80, layout 248 against 253, pre-paint 83 against 80, paint 18 against 19 (minimum): **no difference on this page**. The page has 12,480 elements in the old page and 13,171 in the new one, 1,177
`<wbr>` against 34 and 1,143 U+200B, 691 `mu`: what the page spends its time on is the stacks, the SVG and the stock text, which are what they were, and the text of the formulas, 57% smaller in
characters, is a part of it that was small.

**Server** (`tools/mfsrv2.sh`; the corpus, 1,128 formulas, each rendered 25 times, the median per formula, the minimum over 5 interleaved rounds, load 2.8): flat 94.3 us a formula inline, old hybrid 74.7, **new
76.2** (1.02 of the old, 0.81 of the flat; display 97.0, 75.4, 75.7). The dynamic programme and the lookup of the glyphs that carry a gap cost 11% at first; not writing everything twice
(the order of the document and the order by x are written only if they differ) and not building a vector to count a spacer's characters brought it to 2%.

**What did not work.** (1) A word joiner (U+2060), a no-break space or any of seven other characters in front of a strut or an inline block does not stop a break before it in Chrome 156, and
`white-space: nowrap` on the element, the parent or a wrapper does not stop one either (an inline block inside a `nowrap` span, in a `normal` block, still has a break opportunity before it):
a formula with a stack keeps `<wbr>` and `nowrap`. (2) The strut of a formula that wraps as an inline box with the line height and a vertical alignment worked out from the font's ascent and descent: a browser rounds
the half-leading to whole pixels, so the paragraphs are 1 to 2 px off. (3) The strut at the end of the formula (an `::after`): no blank line, but the first line has none of its extents: one corpus formula's
baseline 5 px off at 760 px. (4) A `U+2060` as the first character of the text with the strut as the root's `::before`: the text goes to the next line and the strut stays on the last.


## Regenerating

`BROTLI=/usr/bin/brotli docs/font-measurements/regen.sh` is the whole sequence (`tools/mkfont.sh all 2` is the font: the pairs of the script glyphs, then the glyphs that carry spacing
and the script space (`mfscriptspace.py`), then the named spacers (`mfnamed.py`), the encoder in the loop (`MATHRUN_RUNS`, `MATHRUN_SPACES` of the harness)) (it needs the scratch directory layout described in it,
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
* Formulas with an inline block in them (a stack, an SVG: 52 of the 470 that wrap in the corpus) keep `<wbr>` and `white-space: nowrap`: Chrome allows a break before and after an
  inline block whatever is done, so the only ways to have no `<wbr>` there would be KaTeX's own wrapper of every base (an element a base). Stock text with a bracket, a hyphen or a
  question mark does the same (2 formulas): `\text` could be written so that no break is possible, with a word joiner after each such character, which needs a table of Unicode's line breaking
  classes in the encoder.
* The spacing is in the glyphs of the classes TeX spaces on both sides (binary operators, relations, punctuation); what is thin or medium next to the others (an ordinary atom and an operator
  like `\sin`, a closing and an opening bracket in `\left`/`\right`, ...) is a named spacer (one character). Spacing at the script sizes is only the script space (three pairs).
* In a column narrower than a formula's first segment the strut of a formula that wraps at U+200B can be alone on a line (see "h").
* Text mode as glyphs (`TEXT_AS_GLYPHS`, off): 98.9% of the glyphs instead of 88.7%, at the price of private-use characters on copy and
  3 bytes a character; a decision for the consumer.
