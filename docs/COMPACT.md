# Compact HTML output

`OutputFormat::CompactHtml` renders a formula as HTML that browsers draw the same way as
KaTeX's, with about a fifth of the bytes and a quarter of the elements, and that they lay out and
settle three to four times faster. It is HTML and CSS only: text stays text in the same fonts, no
script is needed, nothing is an image.

```rust
let settings = Settings::builder()
    .output(OutputFormat::CompactHtml)
    // .compact_levels(katex::compact::levels::EXACT) // pixel-identical steps only
    // .compact_label(false)                          // no aria-label with the TeX
    .build();
let html = render_to_string(&ctx, r"s_{i+1} = f_i(s_i)", &settings)?;
```

Serve `katex::compact::stylesheet()` (also `crates/katex/data/katex-compact.css`, next to a
`fonts/` directory with KaTeX's WOFF2 files) instead of `katex.css`. It contains every rule of
KaTeX 0.18.5's `katex.css`, so a page can mix compact and KaTeX-compatible markup. The other
output formats are unchanged and byte-stable (`tests/compact_spec.rs` pins a golden).

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
| `compact_precision` | `em` numbers of margins, offsets, widths to N significant figures | not used (see below) |

`levels::EXACT` is the first group; `levels::ALL` (the default) adds the rest.

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

### What the last three groups cost

They do not move anything: every glyph and box has the same layout position to 1/64 px (the
layout units Blink uses), also next to running text, and the formula's box and baseline in a line
of text are the same (`docs/compact-measurements/results/geo`). What differs is how Blink
rounds when it paints: a margin and an empty inline block of the same width, or two text spans
and one run, give widths that are summed differently in layout units and in floating point, and
a glyph that sits near a rounding boundary lands on the neighbouring device pixel. On the corpus
that is 0 formulas of 1,128 at scale 1 (1 of 1,128 for display style, 238 pixels) and 14 (inline)
and 19 (display) at scale 2, a few hundred pixels each, always an anti-aliased edge of a glyph
that is a device pixel away. A mismatch on a formula is shown in
`docs/compact-measurements/README.md`. Use `levels::EXACT` if that is not acceptable.

### Precision

`compact_precision` rounds the numbers of margins, offsets and widths to N significant figures
(0 keeps every digit KaTeX computes, four decimals). The heights, depths and `vertical-align`
that set how tall a formula is in a line of text are kept unless `levels::ROUND_EXTENTS` is on:
rounding those moved the line of text around an inline formula by up to a tenth of a pixel at 3
significant figures and by up to six pixels at 2. At 3 figures the largest glyph shift is 0.16 px
(0.008 em); at 2 it is 2.5 px. It saves 0.1% (3 figures) to 0.3% (2) of the raw bytes, because the value tables already
cover most numbers, so the default is 0.

## Regenerating

The tables in `src/compact.rs` between `// *-BEGIN` and `// *-END` markers and
`data/katex-compact.css` are generated; the generators use the standard library only.

```sh
# the corpus is rendered with the steps that need the tables off, then on; see
# docs/compact-measurements/tools/regen.sh for the whole sequence
python3 crates/katex/tools/gen_compact_values.py OUTPUT.jsonl 3 --even [--values-only --groups]
python3 crates/katex/tools/gen_compact_css.py path/to/katex-0.18.5/dist/katex.css crates/katex/data/katex-compact.css
```

The tables were derived from the even-numbered corpus formulas (`--even`), so the odd ones are a
held-out check of the dictionaries (`results/levels.json`, key `heldout`).

## Risks

- Other browsers: only Chrome (headless 156 on Linux) was tested. `contain: layout` fixing the
  baseline of an inline block is in CSS Containment 1 and shipped in Firefox and Safari, but a
  browser that ignores it would put every script at the wrong height. Test before relying on it.
- Fonts: nothing changes in how the KaTeX fonts are used; if they fail to load, both outputs fall
  back alike, but the fallback widths are not the KaTeX ones and the two diverge by layout-unit
  rounding as above.
- Zoom and print: sizes are in em as before, so zooming scales both alike; print was not
  compared. Browser zoom at 125% and other non-integer device scales were not compared.
- Accessibility: there is no MathML. A screen reader gets the TeX from `aria-label` (children of
  `role="math"` are presentational) unless `compact_label` is off, which hides the formula
  entirely, as KaTeX's HTML-only output does.
- Copy and paste: the HTML text of the formula is what is copied (`si+1=fi(si)`); KaTeX's
  `htmlAndMathml` output copies both the MathML text, one symbol per line, and the HTML text.
- Page CSS: the compact classes (`B`, `S`, `X`, `Q`, ...) are only styled below `.katex`.
- Trees nested more than 512 elements deep are written as KaTeX markup, which the stylesheet also
  styles.
