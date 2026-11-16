# Formula sizes without a browser

`Settings::compact_sizes` (off by default, `src/sizes.rs`) computes, from the DOM tree that KaTeX built and the font
metrics, what a browser would otherwise have to lay out to know: how wide a stack (`X`, a fraction, a script pair, a
matrix column, a limit) is, how wide the formula is, and how much room it takes in a line. It works on the markup-only
compact output and on the `FONT_RUN` output alike.

With the option on, the output is the output without it plus one declaration, `width` on every stack whose width could be
derived (in em of the stack, rounded **up** to four decimals: a browser takes a length down to a layout unit, so a value
that is too small by a hair makes a box a layout unit too narrow). `compact::take_sizes()` returns, for the formula
rendered last on the thread, a `FormulaSizes`: `width` of the formula on one line (`None` if unknown), KaTeX's `height` and
`depth`, the extents of the stacks in the line, the number of stacks and how many are known, and why something is unknown
(`unknown`). `FormulaSizes::extents(font_px)` gives the formula's box in a line of text: how far it extends above and below
the baseline, and the baseline's offset from the top of a box without a strut (what `padding-top` of a sized root box
needs). `tests/sizes_spec.rs` has the tests; `crate::sizes` the rules.

## Rules

* A symbol is its metric advance plus its italic correction and `margin-left`/`margin-right`; a glyph run of `mathrun`
  is the advance it was written for (`SymbolNode::width` of the run); symbols that KaTeX combined into one (`\text{ab}`,
  `\mathrm{12}`) add their widths (the tree is KaTeX's when the option is off).
* A size span (`katex-sizing reset-sizeN sizeM`) scales what it holds by `MULT[M]/MULT[N]`; margins, `padding-left`,
  `min-width`, `border-*-width` and `width` of a style are in the em of their element.
* A stack (`vlist-t`) is as wide as its widest item with the item's margins. When the compact output merges a stack of one
  item into one element (`Y`) the margins are outside it and are taken off the width.
* `nulldelimiter` is 0.12 em; `rlap`, `llap`, `clap`, `pstrut` and svg children are 0; `hide-tail`, `katex-stretchy` and
  the other `width: 100%` elements are their `min-width` (a shrink-to-fit box does not take a percentage); `x-arrow-pad`,
  `boxpad`, `cd-arrow-pad`, `fbox`, `cancel-pad`, `cancel-lap`, `angl` and `anglpad` add their padding and borders.
* Unknown: any class that is not in the list (it might set a width), an image, MathML, a style value that is not in em, a
  `vertical-separator` of an array (`min-width: 1px`) and a tree deeper than 200 nodes. Unknown propagates to every box
  around it.

The line box needs the host's font size (the formula's font is 1.21 times the text's): Blink rounds a font's ascent and
descent to whole pixels (and moves a pixel from the ascent to the descent where the descent was rounded down), takes
`line-height: 1.2` in 1/64 px and puts half the leading, rounded down to whole pixels, above. `sizes::line_box(font_px)` does
that for the formula's font, and for a font whose `hhea` metrics differ (the markup-only output: the classes `mathnormal`,
`amsrm`, `small-op`, ... select KaTeX fonts, each with a line box of its own; the hybrid stylesheet puts the generated
font first everywhere, so only one font counts). That is Chrome 156 on Linux with subpixel positioning; another platform
rounds differently.

## Validation

`~/scratch/katex-size/REPORT.md` has the whole measurement. Against headless Chrome 156 (`text-rendering:
geometricPrecision`, 15 px body so the formula is 18.15 px), over the 1,127 corpus formulas that render, inline and in
display mode:

| | hybrid inline | hybrid display | markup-only inline | markup-only display |
|---|---|---|---|---|
| stacks / unknown | 160 / 0 | 183 / 0 | 170 / 1 | 208 / 0 |
| stack width within 1/64 px / 0.05 px / max | 41 % / 66 % / 0.31 px | 47 % / 73 % / 0.31 px | 37 % / 72 % / 0.31 px | 47 % / 77 % / 0.31 px |
| formula width unknown | 1 | 1 | 1 | 1 |
| formula width within 1/64 px / 0.05 px / max | 34 % / 74 % / 0.40 px | 33 % / 68 % / 0.40 px | 31 % / 75 % / 0.41 px | 31 % / 75 % / 0.41 px |
| box above / below / baseline offset, within 1/64 px (formulas that do not wrap) | 100 % | 100 % | 100 % (one baseline offset 0.11 px off) | 100 % |

The widths are too large, by 0.003 px for each glyph of a run (a fit over the 992 formulas that are one run: 0.0034 px a
glyph, 0.0002 px a spacer, residual 0.012 px): Blink's text advances are a little smaller than the font's. A run of a
hundred glyphs is 0.3 px too wide. The one formula whose width is unknown has an array with a vertical rule.
