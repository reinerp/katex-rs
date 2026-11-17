# Compact HTML output

`OutputFormat::CompactHtml` renders a formula as HTML that browsers draw the same way as
KaTeX's, with about a sixth of the bytes and a sixth of the elements, and that they lay out and
settle about five times faster. It is HTML and CSS only: text stays text in the same fonts, no
script is needed, nothing is an image.

```rust
let settings = Settings::builder()
    .output(OutputFormat::CompactHtml)
    // .compact_levels(katex::compact::levels::EXACT)  // pixel-identical steps only
    // .compact_label(false)                           // no aria-label with the TeX
    // .compact_wrap_min(8)                            // shortest formula that may wrap (default 8)
    // .compact_line((0.88, 0.36))                     // room of the host's line, see below
    .build();
let html = render_to_string(&ctx, r"s_{i+1} = f_i(s_i)", &settings)?;
```

Serve `katex::compact::stylesheet()` (also `crates/katex/data/katex-compact.css`, next to a
`fonts/` directory with KaTeX's WOFF2 files) instead of `katex.css`. It contains every rule of
KaTeX 0.18.5's `katex.css` (except two rules for every element, see "Stylesheet"), so a page can
mix compact and KaTeX-compatible markup. The other output formats are unchanged and byte-stable
(`tests/compact_spec.rs` pins a golden). The file name and the API of the stylesheet are those of
the first version of the compact output. The markup changed (new classes on the formula), so a
stylesheet of the first version does not style the new markup; the new stylesheet styles both.

## What changes

| Step (`compact::levels`) | What it does | Pixels |
| --- | --- | --- |
| `UNWRAP_HTML` | no `katex-html` wrapper element | identical |
| (HTML only) | no MathML branch; the TeX is an `aria-label` on `role="math"` | identical |
| `DEAD_CLASSES` | drops `mord`, `mtight`, ... which no CSS rule uses | identical |
| `SHORT_NAMES` | `katex-base` is `B`, `reset-size6 size3` is `F63`, ... | identical |
| `NUM_TRIM` | `0.5em` is `.5em`, no last `;` | identical |
| `VALUE_CLASSES`, `COMBO` | common declarations, and common groups of them, are classes | identical |
| `VLIST` | a `vlist-t` inline table (table, rows, cells, `vlist-s`) is one inline block with `contain: layout` | identical |
| `PSEUDO` | struts and pstruts are `::before` pseudo-elements | identical |
| `MERGE_ITEM` | a vertical list of one item is one element | identical |
| `GLUE` | an empty `mspace` is the margin of the box before or after it | layout identical, see below |
| `MERGE`, `MERGE_CONTENT`, `UNWRAP` | `msupsub`, size spans and grouping spans merged or removed | layout identical, see below |
| `TEXT`, `UNWRAP_TEXT` | text spans that style alike are one run of text | layout identical, see below |
| `FLAT` | no `katex-base` inline blocks: one run of inline content, `<wbr>` where KaTeX may break, one strut | layout identical, see below |
| `SINGLE` | a lone subscript or superscript is one inline span (`vertical-align`, `line-height: 0`), not a stack | layout identical |
| `INLINE_GLUE` | spaces that could not become margins are empty inline spans, not inline blocks | layout identical |
| `compact_precision` | `em` numbers of margins, offsets, widths to N significant figures | not used (see below) |

`levels::EXACT` is the first group of steps (the output of the first version, unchanged byte
for byte); `levels::CLASSIC` adds the three inexact ones of the first version (also unchanged
byte for byte); `levels::ALL` (the default) is everything.

### Why the vertical list can be an inline block

In `katex.css` a `vlist-t` is an inline table: the first row holds one zero-height block per
stacked item, bottom-aligned in a cell of the full height above the baseline, and a second row of
the depth's height hangs below the baseline (the inline table's baseline is the first row's bottom
edge). `.X` (the compact vertical list) is an inline block with `padding-top` for the first row,
`height` for the second and `vertical-align` of minus the depth: the item blocks start at the same
place, the box is as tall, and `contain: layout` puts the inline block's baseline at its bottom
edge as the table's is (without it the baseline would be that of the last item's line). The
table's `vlist-s` cell, which is 2 px wide, and the `-2px` margin that cancels it, are no longer
needed. The rules of `katex.css` that look at `.vlist-t` or its cell are aliased to `.X`.
Pixel-checked against KaTeX JS 0.18.5 for all 1,128 corpus formulas, inline and display, at
device scale 1 and 2: no pixel differs.

### What the three inexact steps of the first version cost

They do not move anything: every glyph and box has the same layout position to 1/64 px (the
layout units Blink uses), also next to running text, and the formula's box and baseline in a line
of text are the same. What differs is how Blink rounds when it paints: a margin and an empty
inline block of the same width, or two text spans and one run, give widths that are summed
differently in layout units and in floating point, and a glyph that sits near a rounding boundary
lands on the neighbouring device pixel. Use `levels::EXACT` if that is not acceptable.

## The second version: runs and lines

Measurements (`docs/compact-measurements/README.md`, section 7) showed that the time a browser
needs for a formula follows the number of nodes it has to create, style, lay out and paint (about
10 microseconds a formula element for everything), and within that atomic inlines (inline blocks)
more than plain inline spans. Bytes did not matter. So the second version removes elements, and
boxes that are formatting contexts of their own, wherever a plain inline run draws the same
pixels: 13.6 elements a formula (corpus) became 8.9.

### No base blocks (`FLAT`)

KaTeX makes a formula a row of `katex-base` inline blocks, each with a `katex-strut` inline block
first, because an inline block is one unit for line breaking and the strut gives the line the
height of what the base holds. Neither needs a box per base:

- The formula's content is inline in the `.katex` element, which has `white-space: nowrap` (the
  class `f`, `tN` or `w`, see below), as the bases had.
- A line box has the largest extent above and below the baseline of everything it holds, so one
  strut with the largest extents of all the bases' struts gives the line the same height. It is
  the `::before` of the formula (an inline block of that height and `vertical-align`): its
  height is a class (`t0`, ... for the 40 most frequent in the corpus) or two custom properties
  (`style="--h:1.0404em;--v:-.345em"` on a formula of the class `f`).
- Where KaTeX allows a line break, between two bases, there is a `<wbr>` (inline style only: a
  display formula is not wrapped). `<wbr>` breaks in Chrome (156) also inside `white-space:
  nowrap` and `pre`; a ZWSP does not break in `nowrap`. A break opportunity as a character would
  need normal wrapping, and then the line breaking rules of Unicode allow breaks KaTeX does not
  (after `|` and `/`, between `)` and `(`, after a comma before a parenthesis), so each would need
  a word joiner. A `<wbr>` costs about as much as a span (in a page of 3,000 copies of `a+b+c=d`,
  90 ms for three each), so `compact_wrap_min` (default 8, in characters of the formula) leaves
  them out of short formulas, which then do not wrap (in a line of text they move to the next
  line whole). With 8, the corpus formulas break at the same places as KaTeX's in paragraphs of
  300 px and of 760 px (318 and 20 formulas that wrap, no difference in the line starts); with 10
  three differ, with 16 fifty-nine, with 24 166. 0 gives every base boundary a `<wbr>`.
- Glue at the end of a base (the space after an operator) stays before the `<wbr>`, so it ends
  the line before the break as in KaTeX. Where it is folded into the left margin of the span after
  the break, the `<wbr>` is moved into that span, after its margin, for the same reason.
- A formula that has anything but bases (`\newline`, a tag), and a formula with `fleqn`, keeps its
  bases. A formula that wraps has the strut of the whole formula in its first line only; the
  other lines are as tall as their own content. See "Risks".

### Lone scripts (`SINGLE`)

A vertical list of one item (a subscript or superscript alone, a raised box) is a zero-height
block at the baseline with a pstrut that puts the item's baseline `P` below its top, shifted by
`top`: the item's baseline is `P + top` below the formula's. The same is an inline span with
`vertical-align: -(P + top)` and `line-height: 0`, in the em of the span (the offsets are
divided by the size the item's size span sets, to six decimals: a number rounded to four makes a
margin truncate to another layout unit once in a hundred cases). The span's zero-height inline
box adds nothing to the line, the room that the list took is in the formula's strut, and there
is no inline block and no pstrut. (`position: relative; top` does the same, but measured about
10% slower in a 3,000 copy page: every shifted element is a paint layer. `line-height: 0` keeps
the shifted box of the smaller font from enlarging the line.) Only items of text and of spans
around text are converted: an inline block inside would take part in the line. Margins are not
summed (a sum is not laid out as two margins are: each truncates to layout units).

### Space

An empty `mspace` that cannot be the margin of a neighbouring span is an inline span with a
margin (`INLINE_GLUE`: an empty inline box with a margin takes the room an empty inline block
does, and is not an atomic inline). A space between two runs of text (an operator with its
spaces) is the margins of one span around the text, and a margin on a size span is in its em.

## Font runs (`FONT_RUN`, off by default)

`levels::ALL | levels::FONT_RUN` writes the part of every formula that a generated font can express as text nodes of that
font, and everything else as the flat design does, in the same tree. The font has the layout in its glyphs: every letter,
digit and operator of Main-Regular, Math-Italic, a subset of AMS and Size1-4, at the sizes and baseline shifts of the scripts
that occur, glyphs that carry TeX's spacing (a relation or a binary operator with its space before and after it, a comma with the
thin space after it, a script glyph with the script space), and spacers of any advance, positive and negative: `a+b+c=d` is seven
characters. So

- consecutive nodes that the font can express (symbols, the glue between atoms, kerns, italic corrections, lone and stacked
  sub- and superscripts whose content is expressible and whose (size, shift) the font has, `\not`-style overlaps, limits
  centred over an operator) are one text node;
- a node it cannot express stays an element (a fraction's stack, SVG, a rule or border, a bold or calligraphic letter, a script
  whose shift the font lacks, text mode), and its children are treated the same way, so a numerator is a text node in a row
  of the stack and `a + \frac{x^2}{y_i} = \mathbf{b}` is a text node, a stack, a text node, a span and a text node;
- a formula that wraps (inline, `compact_wrap_min` characters) and is text and inline spans only has its line break opportunities
  between its bases written as U+200B in its text, and gets the class `mw`: its stylesheet rules let it wrap there and nowhere
  else, and its strut is the element `<span class="mu"></span>` after its first segment. A formula that has an inline block (a
  stack, an SVG, a table: a browser allows a break next to one whatever the parent's `white-space` is) or stock text that a
  browser may break keeps the `<wbr>` between its bases and `white-space: nowrap`;
- the root (`.katex`) gets the class `mf`, which puts the generated font first in its font family; stock KaTeX fonts stay behind it
  and in the families of the classes that set one, so the text that is not in the font is drawn as before.

`mathrun::stylesheet(font_url, blank_url)` is the compact stylesheet with the font rules and `@font-face`;
`mathrun::FONT_WOFF2` is the font (serve it under `mathrun::FONT_FILE_NAME`, which has the font's version in it) and
`mathrun::FONT_VERSION`, `FONT_FAMILY` name it. `compact_label`, `compact_line` (no strut where the host's line has the room,
as before) and `compact_wrap_min` (the break between bases, counted in the characters KaTeX has) work as they do without the
step. The text of a formula with the step on is private-use characters where it is in the font: the TeX is in the
`aria-label`, and text mode (`\text`) stays text in the stock font so that it can be selected. `docs/FONT.md` has the design, the
measurements and the risks (the page needs `text-rendering: geometricPrecision` on the formula, which the stylesheet sets).

## Settings that depend on the page

- `compact_wrap_min` (default 8): see above.
- `compact_line((ascent, descent))`: how far the host's line extends above and below its
  baseline, in em of the formula (the formula's font is 1.21 times the text's; a 16 px page with
  `line-height: 1.5` has 17 px and 7 px, 0.878 em and 0.361 em of 19.36 px). A formula whose
  height and depth fit gets no strut at all (class `w`, which is only `white-space: nowrap`).
  Unset, every formula has its strut, which is exact for every line. With the numbers of that
  page the geometry of the corpus in it is the same as with struts.

## Stylesheet

The classes the second version adds: `tN` (a strut's height, on the formula: `.katex.tN::before`),
`f` and `w` (the formula, `white-space: nowrap`; `f` has the strut from the custom properties) and
`zN` (a shifted item: `vertical-align`, `line-height: 0`, and its margins). `katex.css` has two
rules for every element, `.katex *`: the old Edge's `-ms-high-contrast-adjust` (not known to other
browsers) and `border-color: currentColor`. The first is gone and the second is a rule for the
classes that draw a border, because a rule for every element makes the browser check the
ancestors of every element; a page that resets border colours on `*` still gets KaTeX's where
they show. `unwrap_html` leaves the children of `katex-html` in `katex`, so
`.katex>.katex-newline` and `.katex-display>.katex>.katex-tag` have the rules that
`.katex-html>...` had (not tested: no corpus formula has them).

## Regenerating

The tables in `src/compact.rs` between `// *-BEGIN` and `// *-END` markers and
`data/katex-compact.css` are generated; the generators use the standard library only.

```sh
# the corpus is rendered with the steps that need the tables off, then on; see
# docs/compact-measurements/tools/regen.sh for the whole sequence
python3 crates/katex/tools/gen_compact_values.py OUTPUT.jsonl 3 --even [--values-only --groups]
python3 crates/katex/tools/gen_compact_flat.py OUTPUT.jsonl 3 --even   # struts of formulas, shifts
python3 crates/katex/tools/gen_compact_css.py path/to/katex-0.18.5/dist/katex.css crates/katex/data/katex-compact.css
```

The first group of tables (`VALUES`, `GEOMS`, `ITEMS`, `STRUTS`, `PSTRUTS`) is frozen: it keeps
the output of `levels::EXACT` and `levels::CLASSIC` what it was. `FSTRUTS` and `SHIFTS` are for the
second version. They were derived from the even-numbered corpus formulas (`--even`), so the odd
ones are a held-out check.

## Risks

- Other browsers: only Chrome (headless 156 on Linux) was tested. `contain: layout` fixing the
  baseline of an inline block is in CSS Containment 1 and shipped in Firefox and Safari, but a
  browser that ignores it would put every stack at the wrong height. That `<wbr>` breaks inside
  `white-space: nowrap` is Chrome's behaviour; a browser that does not break there would not
  wrap long inline formulas. `vertical-align` with `line-height: 0`, `::before` and custom
  properties are old CSS. Test before relying on it.
- `<wbr>` also breaks inside a `white-space: nowrap` or `pre` ancestor of the formula, where
  KaTeX's formulas do not: an inline formula of `compact_wrap_min` or more characters in a table
  cell with `white-space: nowrap` may wrap there.
- Wrapped formulas: a formula that wraps has its strut in the first line only, so a line without
  the tall part is shorter than in KaTeX and the first line may be taller. Of the 318 corpus
  formulas that wrap in 300 px paragraphs the paragraph height differs for 18 (by 0.2 to 31 px).
  Where that matters use `levels::CLASSIC` or `levels::EXACT`.
- Geometry: the left and width of every character are KaTeX's to the last layout unit (1/64 px)
  in the whole corpus; the top of 19% of the glyph boxes differs by 1/64 to 3/64 px (not
  explained: the same with `position: relative`), and at scale 2 some formulas have glyph edges
  that anti-alias on a neighbouring device pixel (measurements, section 7).
- Fonts: nothing changes in how the KaTeX fonts are used; if they fail to load, both outputs fall
  back alike, but the fallback widths are not the KaTeX ones.
- Zoom and print: sizes are in em as before, so zooming scales both alike; print was not
  compared. Browser zoom at 125% and other non-integer device scales were not compared.
- Accessibility: there is no MathML. A screen reader gets the TeX from `aria-label` (children of
  `role="math"` are presentational) unless `compact_label` is off, which hides the formula
  entirely, as KaTeX's HTML-only output does.
- Copy and paste: the HTML text of the formula is what is copied (`si+1=fi(si)`); KaTeX's
  `htmlAndMathml` output copies both the MathML text, one symbol per line, and the HTML text.
  Stacks (fractions, matrices) still break lines where blocks are.
- Page CSS: the compact classes (`B`, `S`, `X`, `Q`, `zN`, ...) are only styled below `.katex`,
  and the formula's own `tN`, `f` and `w` as `.katex.tN`. A page rule on `.katex` that sets
  `white-space` or `display` would change the output.
- Trees nested more than 512 elements deep are written as KaTeX markup, which the stylesheet also
  styles.

## Tried and not built in

See `docs/compact-measurements/README.md`, sections 6 and 7: dropping the wrapper span of a stack row's content (a pixel in some parentheses moved), a stack as lines with line heights
instead of blocks with pstruts (not built: the pstrut pseudo-elements cost nothing in the
fixture), a composite font family with a spacer font (shelved on the bookmark
`shelved/composite-km-font`; it needs `text-rendering: geometricPrecision`, because Chrome on
Linux rounds the advance of every glyph to a pixel otherwise, and a generated font).
