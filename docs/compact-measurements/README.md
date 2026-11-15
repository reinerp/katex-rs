# Measurements behind the compact output

Everything here was measured on 2026-11-15 on the shared machine `agenthost` (load average 3 to 9
from other agents' work during the timing runs; timings are medians of interleaved runs with the
minimum next to them). Browser: headless Google Chrome (156), Linux, CDP, `--force-color-profile=srgb`.
The reference is KaTeX JS 0.18.5 (`katex.min.js`/`katex.min.css`), the 1,128-formula corpus of the
hardening work (formulas from agent transcripts), and the fixture is the agentserver rig's: 400
messages of 12 corpus formulas each, `corpus[k % 1128]`, 4,800 formula occurrences, in 760 px
wide paragraphs of 15 px text (the page is built by `tools/mkpage.py`; agentserver itself was not
touched).

`tools/` holds the scripts and the Rust harness `ch.rs` (path dependency on the fork; build with
`tools/sxb cargo build --release`). They assume `~/scratch/katex-compact` and the sandbox wrappers
`sxb`/`sxc`/`sxw` (`sbx --no-net`). `results/` has the raw numbers, `crops/` the pictures.

## 1. Where the bytes and the time go in the current output (HTML + MathML)

Corpus: 1,127 formulas rendered (one is a parse error), 2,348,108 bytes, 2,082 B per formula,
63,539 elements (56 per formula), at most 29 levels deep; zstd -9 69,036 B, gzip -9 95,055 B.
Fixture: 10,090,878 B (2,102 B per formula), 272,766 elements; zstd 69,798 B (the repeats
compress away), gzip 410,721 B (32 KB window).

By branch: the HTML branch is 78.9% of the bytes (183,565 of the 272,766 fixture elements), the MathML
branch 19.3% (82,802 elements, 30% of the nodes), the wrappers 1.9%. By cause, over everything:

| cause | share of bytes |
| --- | --- |
| tags: names, closing tags and brackets of the 272,766 elements | 34.5% |
| `class` attributes | 32.8% |
| `style` attributes (HTML branch only) | 21.7% |
| MathML attributes (`xmlns`, `encoding`, `stretchy`, `mathvariant`, ...) | 6.3% |
| text (HTML 0.9%, MathML 2.0%) | 2.9% |
| other attributes (`aria-hidden`, SVG `d`, `viewBox`) | 1.8% |

Style bytes by property (corpus): `margin-right` 157 KB in 7,319 declarations, `height` 115 KB in
7,898, `vertical-align` 41 KB, `top` 26 KB, `margin-left` 25 KB. By construct (share of the HTML
branch's bytes): empty `mspace` spans 12.4% (3,954), vertical-list cells 9.0% and rows 5.1% and
tables 3.0% and `vlist-s` 2.3%, struts 9.6% (2,458), pstruts 5.6% (2,087), `mord` wrappers 5.3%,
`katex-sizing` spans 5.0%, `katex-base` 4.2% (2,451). The visible text is under 1%.

Repetition (`tools/repeat.py`): in the corpus 2,395 occurrences are 1,128 distinct formulas (53%
repeats; in the fixture 77% of the bytes are repeats of a formula already in the page, by
construction); inside the 1,128 distinct formulas 81% of the HTML elements are in subtrees that
occur at least twice, and later copies of maximal repeated subtrees (40 bytes or more) are 45% of
the bytes (32% of the compact output). Compression already takes most of that (zstd 36.6 KB for
the HTML branch alone, 8.1 MB raw in the fixture).

Browser, fixture page, 4,800 formulas (Chrome trace of one run, ms): `ParseHTML` 1,936,
`UpdateLayoutTree` 995, `Layout` 2,715, `PrePaint` 593, `Paint` 92, `Commit` 151; the page settles
(load, fonts, two animation frames) in 6.8 s median (5.6 s minimum), 389,743 DOM nodes. So parsing is
about a quarter, style a seventh, layout 40%. Dropping the MathML branch alone: 4.2 s, layout
2.7 to 1.3 s, 255,936 nodes. KaTeX JS rendering all formulas in the page: 6.1 to 6.7 s, 1.9 s of it script.

Which constructs: 3,000 copies of one formula in a paragraph each (`results/micro.jsonl`; settle
time in ms, minimum of 5, current output / compact (ALL) / compact (EXACT); layout in the same
order is 870/297/308 for the first row):

| formula | current | ALL | EXACT |
| --- | --- | --- | --- |
| `\text{hello world}` | 668 | 325 | 332 |
| `a+b+c=d` | 1,951 | 882 | 1,015 |
| `x_i` | 1,668 | 496 | 550 |
| `x^2` | 1,452 | 495 | 531 |
| `x_i^2` | 1,915 | 678 | 913 |
| `\frac{a}{b}` | 2,092 | 895 | 989 |
| `\sqrt{x}` | 1,879 | 860 | 811 |
| `\sum_{i=0}^{n} i` | 2,622 | 865 | 1,064 |
| 2x2 `pmatrix` | 4,072 | 1,404 | 1,458 |
| `a\,b\;c\quad d` | 1,477 | 441 | 523 |

The paragraphs alone cost about 300 ms of these. Vertical-list tables (anything with a script, a
fraction, a big operator or a matrix) are the dominant cost: layout of `x_i` is 743 ms against 231
ms for plain text; the MathML branch is the second.

## 2. Levers, cumulative (`tools/levels.py`, `results/levels.json`)

Corpus and fixture sizes are of the markup; "elements" are start tags. Settle time is the
fixture page (4,800 formulas), median / minimum of 7 interleaved runs at load 4 to 9, so
differences under about 400 ms between neighbouring rows are noise. The stylesheet (44 KB, once per
page) is not in the sizes.

| step | corpus B | zstd -9 | gzip -9 | elements | fixture B | fixture elements | settle ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 current fork output (HTML + MathML) | 2,348,108 | 69,036 | 95,055 | 63,539 | 10,090,878 | 272,766 | 6,777 / 5,637 |
| 1 drop the MathML (KaTeX output=html) | 1,894,646 | 36,571 | 51,155 | 44,210 | 8,143,870 | 189,964 | 4,240 / 3,321 |
| 2 compact: katex-html wrapper dropped | 1,858,582 | 36,439 | 50,843 | 43,083 | 7,990,398 | 185,168 | 3,598 / 3,302 |
| 3 + unused classes dropped | 1,673,588 | 35,485 | 47,548 | 43,083 | 7,194,979 | 185,168 | 3,745 / 3,403 |
| 4 + short class names | 1,450,401 | 34,745 | 45,073 | 43,083 | 6,237,665 | 185,168 | 3,638 / 3,139 |
| 5 + shorter numbers | 1,418,757 | 34,659 | 44,749 | 43,083 | 6,101,959 | 185,168 | 3,781 / 3,092 |
| 6 + common declarations as classes | 1,145,466 | 33,475 | 41,579 | 43,083 | 4,930,327 | 185,168 | 3,674 / 3,252 |
| 7 + vertical lists as inline blocks | 904,011 | 32,211 | 38,053 | 33,551 | 3,893,052 | 144,198 | 2,712 / 2,378 |
| 8 + struts and pstruts as pseudo-elements | 728,259 | 30,949 | 33,944 | 29,277 | 3,140,639 | 125,890 | 2,847 / 2,656 |
| 9 + one element for a one-item list | 692,801 | 30,695 | 33,715 | 27,700 | 2,989,462 | 119,166 | 2,482 / 2,359 |
| 10 + group classes (= `levels::EXACT`) | 676,020 | 30,822 | 33,372 | 27,700 | 2,917,869 | 119,166 | 2,435 / 2,327 |
| 11 + spacing as margins | 613,407 | 31,072 | 33,969 | 23,791 | 2,649,848 | 102,431 | 2,666 / 2,214 |
| 12 + `msupsub` and size spans merged | 562,182 | 30,714 | 32,882 | 21,074 | 2,430,928 | 90,831 | 2,844 / 2,020 |
| 13 + grouping spans removed | 519,997 | 29,135 | 31,325 | 18,262 | 2,248,580 | 78,669 | 3,102 / 2,094 |
| 14 + text runs joined (= `levels::ALL`) | 491,855 | 27,872 | 30,052 | 15,378 | 2,128,601 | 66,344 | 2,199 / 1,782 |
| 15 ALL + `aria-label` with the TeX | 528,116 | 40,171 | 43,312 | 15,378 | 2,286,302 | 66,344 | 2,531 / 1,747 |

Settle time of the key rows over 9 interleaved runs (`results/bench3.jsonl`, `bench4.jsonl`),
median / minimum: row 0 7,555 / 6,438; KaTeX JS in the page 6,699 / 5,902; row 1 4,255 / 3,771; row 10
2,894 / 2,402; row 14 2,073 to 2,236 / 1,837 to 1,841; row 15 2,117 / 1,875.

The dictionaries (`VALUES`, `GEOMS`, `ITEMS`, `STRUTS`, `PSTRUTS`) were built from the even-numbered
corpus formulas; on the odd ones, which they have not seen, the output is 430 B per formula against
436 for all, so the ratio to the current output is 4.67 on the held-out half and 4.78 on all.

Not a property of the output but of the page: `content-visibility: auto; contain-intrinsic-size:
auto 420px` on each message (400 messages of about 25 KB in the fixture) lets the browser skip
layout and paint of the messages that are off screen: settle 2.2 s for the current output (from 7.6
s), 2.2 s for KaTeX JS, 0.70 s for compact ALL and 0.65 s with the label. It changes no visible
pixel; the scroll height of unrendered messages is the guessed one until they are first shown.

## 3. Pixels, against KaTeX JS 0.18.5, all 1,128 formulas

Each formula as in a 200 px row of 20 px text (`white-space: nowrap`), inline style and display
style (every corpus formula both ways), device scale 1 and 2, whole-page screenshots compared
pixel by pixel (`tools/pix.py`, `pixcmp.mjs`).

| output | scale 1 inline | scale 1 display | scale 2 inline | scale 2 display |
| --- | --- | --- | --- | --- |
| KaTeX-compatible fork output | 0 | 0 | 0 | 0 |
| `levels::EXACT` | 0 | 0 | 0 | 0 |
| `levels::ALL` (default) | 0 | 1 formula, 238 px | 14 formulas, 6,583 px | 19 formulas, 8,973 px |

The differences of ALL are an anti-aliased glyph edge a device pixel away
(`crops/glyph_flip_id154_dpr2.png`, `glyph_flip_display_id4_dpr1.png`: rows are KaTeX JS, compact,
and the difference in red). The layout is the same: for the 13,592 glyphs of the corpus the
rectangle of every character equals KaTeX's to the last bit of a layout unit (1/64 px) with
ALL and with EXACT (`tools/geom.py`, `geocmp.py`, `results/geo/js_vs_p0.txt`), the box of each
formula and the baseline and height of a paragraph of running text around it are unchanged
(`results/geo/js_vs_p0_context.txt`: 0 of 1,128 differ). The cause is that Blink sums the widths
of separate text spans and margins in layout units and paints a joined run with float advances, so
a glyph near a rounding boundary changes device pixel. Bisected: the spacing step, the removal of
grouping spans, and the merges each cause some of it; `EXACT` leaves all of them out.

## 4. Precision of the numbers (`compact_precision`)

Rounding to N significant figures the margins, offsets and widths (the vertical extents stay exact
unless asked; see below), all other steps on, dictionaries rebuilt for each setting
(`results/precsize_*.json`). Shifts are of every glyph's rectangle against the unrounded
compact output, in CSS px of a 16 px page (the formula is at 19.36 px, 1.21 em); at scale 2 a CSS
pixel is two device pixels.

| digits | corpus B (raw / zstd / gzip) | no value tables, raw | differing formulas vs KaTeX JS: scale 1 inline / display, scale 2 inline / display | glyph shift median / p99 / max | longest-formula width change | line of text around an inline formula |
| --- | --- | --- | --- | --- | --- | --- |
| all (4 decimals) | 491,855 / 27,872 / 30,052 | 813,095 | 0 / 1, 14 / 19 | 0 / 0 / 0 | 0 | unchanged |
| 4 significant | 491,775 / 27,836 / 30,020 | 812,877 | 0 / 1, 14 / 19 | 0 / 0.016 px / 0.016 px (0.0008 em) | 0 | unchanged |
| 3 significant | 491,392 / 27,852 / 29,987 | 807,984 | 130 / 65, 221 / 137 | 0 / 0.094 px / 0.156 px (0.008 em) | 0.06 px | unchanged (0 of 1,128) |
| 2 significant | 490,237 / 27,758 / 29,653 | 799,701 | 809 / 566, 884 / 684 | 0.11 / 0.97 / 2.5 px (0.13 em) | 2.5 px | unchanged (0 of 1,128) |

Rounding the vertical extents (`height`, `padding-top`, `vertical-align`, `levels::ROUND_EXTENTS`)
changes the line: at 4 digits up to 0.016 px in 20 paragraphs, at 3 up to 0.11 px in 71, at 2 up to
5.8 px in 101 (`results/geo/p0_vs_ext*_context.txt`). That is why they are excluded by default.

Rounding saves 0.1% (3 digits) to 0.3% (2) of the raw bytes with the dictionaries and 0.6% to 1.7%
without them, because the numbers that occur most are in class names anyway. Errors do not
accumulate along a formula: the mean shift per 150 px of formula width is flat (0.01 px at 3 digits,
0.4 to 0.7 px at 2), the largest ones are in long arrows (`\xrightarrow`, id 291) and in widely
spaced alignments, where the 2-digit rounding of a `2em`-class margin shows. Worst cases side by
side (rows: KaTeX JS, compact exact, 3 digits, 2 digits, and the difference of the 2-digit row):
`crops/precision_id291.png` (an arrow chain, the 2-digit row is visibly wider),
`precision_id101.png`, `precision_id183.png`. Recommendation: do not round (default 0); 3 digits is
harmless to the eye (largest shift 0.16 px) but gains 0.1%.

## 5. Copy and paste

`window.getSelection().toString()` over `before <formula> after` (`tools/seltest.py`), Chrome. The
current output gives, for `s_{i+1} = f_i(s_i)`, the MathML text one symbol per line in mathematical
italic code points (`𝑠`, `𝑖`, `+`, ...), then the HTML text with zero-width spaces and extra
newlines at every script. The compact output gives `before si+1=fi(si) after`, once, with plain
letters; fractions and matrices still break lines where blocks are. The TeX is in `aria-label`
(not copied) unless `compact_label` is off.

## 6. Tried and dropped

- `<q>` for `<span>` (a tag that is neither a formatting element nor a block): 18.6% fewer raw bytes
  (493 KB to 400 KB; zstd -3.6%), pixel-identical, but each `q` has `::before`/`::after` from the user
  agent style sheet and style recalculation doubled (282 to 648 ms), so settle time rose from
  2.07 s to 2.41 s. Kept as a commit marked REJECTED (`REJECTED/katex-rs-short-tags`).
- Precision (section 4): no gain.
- Folding the glue into text (a span around the neighbouring text) saves bytes but no elements.
- Merging a size span with a non-text child, and `font-size` onto the vertical list itself:
  not exact, not tried further.
