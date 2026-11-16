//! Sizes of a formula, from its tree and the font metrics, without a browser.
//!
//! The compact output leaves the browser to find out how wide a stack of items is (a fraction, a script pair, a matrix
//! column), how wide the formula is and how much room it takes in its line. Here they are computed from what KaTeX built:
//! a symbol's advance from the metric tables (the glyph runs of [`crate::mathrun`] have their summed advance as width), glue
//! and kerns are their em values, a size span scales what it holds, a stack (`vlist-t`) is as wide as its widest item with
//! the item's margins, delimiters, rules and borders come from the styles and from the rules of `katex.css` that give an
//! element a width. [`annotate`] puts the width of every stack it can compute in the `width` of the stack's span (which the
//! compact output writes as `width` of the `X`) and returns the formula's sizes.
//!
//! A subtree whose width cannot be derived has no width (`None`) and that propagates to every box around it. That is: any
//! class that this module does not know (it might give the element a width), a `\includegraphics` image, a vertical
//! separator (`min-width: 1px`) and a `\newline`. Widths are in em of the element they are measured on: a stack's width in
//! the em of the stack, which is the em that its `style` uses.

use crate::dom_tree::{DomSpan, HtmlDomNode};
use crate::types::CssProperty as P;

/// The ascent and descent of the formula's font (KaTeX_Main and the generated font have the same), in em, and the line height
/// that `.katex` sets.
const ASCENT: f64 = 0.903;
const DESCENT: f64 = 0.272;
const LINE_HEIGHT: f64 = 1.2;

/// How far the line box of an element with the formula's font and `line-height: 1.2` extends above and below the baseline, in
/// px, at a font size of `font_px`: Blink rounds the font's ascent and descent to whole pixels (and moves a pixel from the
/// ascent to the descent where the descent was rounded down: Linux with subpixel positioning), takes the line height in
/// 1/64 px and puts half of the leading (rounded down to whole pixels) above. Chrome 156 on Linux; other platforms round
/// differently.
#[must_use]
pub fn line_box(font_px: f64) -> (f64, f64) {
    font_line_box(font_px, ASCENT, DESCENT)
}

/// [`line_box`] of a font with another ascent and descent (em).
#[must_use]
pub fn font_line_box(font_px: f64, ascent: f64, descent: f64) -> (f64, f64) {
    font_line_box_h(font_px, ascent, descent, LINE_HEIGHT)
}

/// [`font_line_box`] for another line height (em; 0 for the elements that the compact output shifts: an inline box that is a
/// point).
#[must_use]
pub fn font_line_box_h(font_px: f64, ascent: f64, descent: f64, line_height: f64) -> (f64, f64) {
    let mut a = (ascent * font_px).round();
    let mut d = (descent * font_px).round();
    // Blink on Linux with subpixel positioning: if the descent was rounded down it takes a pixel from the ascent
    if d < descent * font_px && a >= 1.0 {
        d += 1.0;
        a -= 1.0;
    }
    let line = (line_height * font_px * 64.0).round() / 64.0;
    let above = a + ((line - (a + d)) / 2.0).floor();
    (above, line - above)
}

const MULT: [f64; 11] = [0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.2, 1.44, 1.728, 2.074, 2.488];

/// What [`annotate`] found out about a formula. Lengths are in em of the formula's font.
#[derive(Debug, Clone, Default)]
pub struct FormulaSizes {
    /// The width of the formula on one line, `None` if it cannot be derived.
    pub width: Option<f64>,
    /// KaTeX's height above the baseline and depth below it (the extent of the strut).
    pub height: f64,
    /// See `height`.
    pub depth: f64,
    /// How far the stacks that sit in the formula's line extend above the baseline (the `padding-top` of the tallest).
    pub stack_above: f64,
    /// How far they extend below it (the `height` of the deepest).
    pub stack_below: f64,
    /// The ascent and descent (em) of the other fonts that text in the formula's line is set in (a font has its own line box).
    pub fonts: Vec<(f64, f64)>,
    /// The lone scripts and raised boxes in the line (a stack of one item that is plain text, which the compact output writes
    /// as an inline element shifted by `vertical-align` with `line-height: 0`): how far the item's baseline is below the
    /// formula's (em of the formula) and the font size ratio of the item. Such an element is a point in the line box.
    pub shifted: Vec<(f64, f64)>,
    /// Why a width could not be derived (class names this module does not know, styles and nodes it cannot read), for
    /// finding what to add.
    pub unknown: Vec<String>,
    /// Stacks (`vlist-t`) in the formula.
    pub stacks: usize,
    /// Stacks whose width was computed.
    pub stacks_known: usize,
    /// `false` if something that sets the extent could not be read (the extents are then not to be used).
    pub extents_known: bool,
}

struct Acc {
    strut_above: f64,
    strut_below: f64,
    stack_above: f64,
    stack_below: f64,
    stacks: usize,
    known: usize,
    extents_ok: bool,
    /// The output is the hybrid one: its stylesheet puts the generated font (that of the formula) first in every class that
    /// sets a font, so no inline box has another font's metrics.
    hybrid: bool,
    /// Symbols (text) seen so far in the formula's line; a font span that holds some adds its font to `fonts`.
    text_seen: usize,
    /// The ascent and descent (em) of the fonts of inline boxes that hold text in the line, other than the formula's own.
    fonts: Vec<(f64, f64)>,
    shifted: Vec<(f64, f64)>,
    reasons: Vec<String>,
}

/// The ascent and descent of the font that a class sets for the text in its element (the `hhea` metrics of the KaTeX fonts).
fn font_metrics(class: &str) -> Option<(f64, f64)> {
    Some(match class {
        "mathnormal" => (0.717, 0.218),
        "mathit" | "textit" => (0.75, 0.25),
        "mathbf" | "textbf" => (0.951, 0.268),
        "boldsymbol" => (0.725, 0.216),
        "amsrm" | "mathbb" | "textbb" => (1.003, 0.463),
        "mathcal" => (0.789, 0.143),
        "mathfrak" | "textfrak" => (0.741, 0.257),
        "mathboldfrak" | "textboldfrak" => (0.741, 0.242),
        "mathscr" | "textscr" => (0.735, 0.314),
        "mathtt" | "texttt" => (0.694, 0.229),
        "mathsf" | "textsf" | "mathboldsf" | "textboldsf" => (0.75, 0.25),
        "mathsfit" | "mathitsf" | "textitsf" => (0.751, 0.25),
        "small-op" => (0.85, 0.35),
        "large-op" => (1.36, 0.862),
        _ => return None,
    })
}

/// `delimsizing size1` .. `size4`.
fn delim_font(classes: &crate::types::ClassList) -> Option<(f64, f64)> {
    if !classes.contains("delimsizing") {
        return None;
    }
    [
        ("size1", (0.85, 0.35)),
        ("size2", (1.36, 0.862)),
        ("size3", (1.45, 0.95)),
        ("size4", (1.75, 1.25)),
    ]
    .into_iter()
    .find(|(c, _)| classes.contains(c))
    .map(|(_, m)| m)
}

fn em_value(value: &str) -> Option<f64> {
    let v = value.trim();
    if v == "0" {
        return Some(0.0);
    }
    let n = v.strip_suffix("em")?;
    n.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// `[left, right]` of a `margin` shorthand.
fn margin_shorthand(value: &str) -> Option<(f64, f64)> {
    let parts: Vec<f64> = value
        .split_whitespace()
        .map(em_value)
        .collect::<Option<_>>()?;
    match parts.as_slice() {
        [a] => Some((*a, *a)),
        [_, h] | [_, h, _] => Some((*h, *h)),
        [_, r, _, l] => Some((*l, *r)),
        _ => None,
    }
}

/// The sizes of the formula whose tree is `root` (the `katex` span or the `katex-display` that holds it), and the width of
/// every stack set in its `width`.
pub fn annotate(root: &mut DomSpan, hybrid: bool) -> FormulaSizes {
    let mut acc = Acc {
        strut_above: 0.0,
        strut_below: 0.0,
        stack_above: 0.0,
        stack_below: 0.0,
        stacks: 0,
        known: 0,
        extents_ok: true,
        hybrid,
        text_seen: 0,
        fonts: Vec::new(),
        shifted: Vec::new(),
        reasons: Vec::new(),
    };
    let katex: Option<&mut DomSpan> = if root.classes.contains("katex") {
        Some(root)
    } else {
        root.children.iter_mut().find_map(|c| match c {
            HtmlDomNode::DomSpan(s) if s.classes.contains("katex") => Some(s),
            _ => None,
        })
    };
    let Some(katex) = katex else {
        return FormulaSizes::default();
    };
    let width = sum_children(&mut katex.children, true, false, &mut acc, 0);
    FormulaSizes {
        width,
        height: acc.strut_above,
        depth: acc.strut_below,
        stack_above: acc.stack_above,
        stack_below: acc.stack_below,
        fonts: acc.fonts,
        shifted: acc.shifted,
        unknown: {
            let mut r = acc.reasons;
            r.sort();
            r.dedup();
            r
        },
        stacks: acc.stacks,
        stacks_known: acc.known,
        extents_known: acc.extents_ok,
    }
}

/// The sum of the widths of `nodes` (which all get visited); `None` if one has none.
fn sum_children(
    nodes: &mut [HtmlDomNode],
    line: bool,
    in_sqrt: bool,
    acc: &mut Acc,
    depth: usize,
) -> Option<f64> {
    let mut sum = Some(0.0);
    for node in nodes {
        let w = walk(node, line, in_sqrt, acc, depth + 1);
        sum = match (sum, w) {
            (Some(a), Some(b)) => Some(a + b),
            _ => None,
        };
    }
    sum
}

/// The width of a node in em of its parent, and the annotation of what is in it.
fn walk(
    node: &mut HtmlDomNode,
    line: bool,
    in_sqrt: bool,
    acc: &mut Acc,
    depth: usize,
) -> Option<f64> {
    if depth > 200 {
        acc.reasons.push("too deep".to_owned());
        return None;
    }
    match node {
        HtmlDomNode::Symbol(s) => {
            if line && !s.text.is_empty() {
                acc.text_seen += 1;
                if !acc.hybrid
                    && let Some(m) = s
                        .classes
                        .iter()
                        .find_map(font_metrics)
                        .or_else(|| delim_font(&s.classes))
                {
                    acc.fonts.push(m);
                }
            }
            let mut w = s.width + s.italic.max(0.0);
            for (p, v) in s.style.iter() {
                match p {
                    P::MarginLeft | P::MarginRight => w += em_value(v)?,
                    P::Color | P::BackgroundColor | P::Top | P::Position | P::BorderColor => {}
                    _ => {
                        acc.reasons.push(format!("symbol style {p}"));
                        return None;
                    }
                }
            }
            Some(w)
        }
        HtmlDomNode::Fragment(f) => sum_children(&mut f.children, line, in_sqrt, acc, depth),
        HtmlDomNode::Anchor(a) => sum_children(&mut a.children, line, in_sqrt, acc, depth),
        HtmlDomNode::SvgNode(_) => Some(0.0),
        HtmlDomNode::DomSpan(span) => span_width(span, line, in_sqrt, acc, depth),
        HtmlDomNode::Img(_) | HtmlDomNode::MathML(_) => {
            acc.reasons.push("image or MathML".to_owned());
            None
        }
    }
}

#[derive(Default)]
struct Kind {
    /// The factor of the font size a size span sets.
    ratio: f64,
    sizing: bool,
    delim: bool,
    vlist: bool,
    vbox: bool,
    /// Its width is its content's (an element with `width: 100%` in a shrink-to-fit box has none).
    percent: bool,
    zero: bool,
    nulldelimiter: bool,
    /// Width from the padding, margins and borders the stylesheet gives the class (em).
    extra: f64,
    /// A class this module does not know: the width is not to be trusted.
    unknown: bool,
    unknown_class: Option<String>,
    /// `katex-newline` and the like: the formula is not one line.
    strut: bool,
}

fn classify(span: &DomSpan, in_sqrt: bool) -> Kind {
    let mut k = Kind {
        ratio: 1.0,
        ..Kind::default()
    };
    let (mut size, mut reset) = (None, None);
    for c in span.classes.iter() {
        match c {
            "katex" | "katex-html" | "katex-base" | "mord" | "mbin" | "mrel" | "mopen"
            | "mclose" | "mpunct" | "minner" | "mop" | "mtight" | "allowbreak" | "nobreak"
            | "mspace" | "msupsub" | "op-limits" | "mathnormal" | "mathrm" | "mathit"
            | "mathbf" | "boldsymbol" | "amsrm" | "mathbb" | "textbb" | "mathcal" | "mathfrak"
            | "textfrak" | "mathboldfrak" | "textboldfrak" | "mathtt" | "mathscr" | "textscr"
            | "mathsf" | "textsf" | "mathboldsf" | "textboldsf" | "mathsfit" | "mathitsf"
            | "textitsf" | "mainrm" | "textbf" | "textit" | "textrm" | "texttt" | "text"
            | "op-symbol" | "small-op" | "large-op" | "delimcenter" | "mult" | "delim-size1"
            | "delim-size4" | "mfrac" | "mover" | "munder" | "x-arrow" | "katex-accent"
            | "accent-full" | "katex-overline" | "katex-underline" | "overline" | "underline"
            | "katex-smash" | "svg-align" | "vlist" | "vlist-r" | "vlist-t2" | "mtable"
            | "col-align-c" | "col-align-l" | "col-align-r" | "arraycolsep" | "sqrt"
            | "katex-inner" | "katex-fix" | "katex-sout" | "cancel" => {}
            "vlist-t" => k.vlist = true,
            "katex-strut" => k.strut = true,
            "pstrut" | "rlap" | "llap" | "clap" | "katex-thinbox" | "accent-body" => k.zero = true,
            "katex-vbox" => k.vbox = true,
            "nulldelimiter" => k.nulldelimiter = true,
            "katex-sizing" | "fontsize-ensurer" => k.sizing = true,
            "delimsizing" => k.delim = true,
            "frac-line" | "overline-line" | "underline-line" | "katex-hline"
            | "katex-hdashline" | "katex-stretchy" | "hide-tail" => k.percent = true,
            "brace-left" | "brace-center" | "brace-right" | "halfarrow-left"
            | "halfarrow-right" => {
                k.zero = true;
            }
            "x-arrow-pad" => k.extra += 1.0,
            "boxpad" => k.extra += 0.6,
            "cd-arrow-pad" => k.extra += 0.55556 + 0.27778,
            "fbox" | "fcolorbox" => k.extra += 0.08,
            "cancel-pad" => k.extra += 0.4,
            "cancel-lap" => k.extra -= 0.4,
            "anglpad" => k.extra += 2.0 * 0.03889,
            "angl" => k.extra += 0.049 + 0.03889,
            "katex-root" if in_sqrt => k.extra += 0.2777777778 - 0.5555555556,
            "katex-rule" => {}
            _ => {
                if let Some(n) = c.strip_prefix("reset-size") {
                    reset = n.parse::<usize>().ok().filter(|n| (1..=11).contains(n));
                } else if let Some(n) = c.strip_prefix("size") {
                    size = n.parse::<usize>().ok().filter(|n| (1..=11).contains(n));
                } else {
                    k.unknown = true;
                    k.unknown_class = Some(c.to_owned());
                }
            }
        }
    }
    if k.delim {
        // `delimsizing size1`: the size class names a font.
    } else if k.sizing {
        match (size, reset) {
            (Some(s), Some(r)) => k.ratio = MULT[s - 1] / MULT[r - 1],
            _ => k.unknown = true,
        }
    } else if size.is_some() || reset.is_some() {
        k.unknown = true;
    }
    k
}

fn span_width(
    span: &mut DomSpan,
    line: bool,
    in_sqrt: bool,
    acc: &mut Acc,
    depth: usize,
) -> Option<f64> {
    let kind = classify(span, in_sqrt);
    let mut ok = !kind.unknown;
    if let Some(c) = &kind.unknown_class {
        acc.reasons.push(format!("class {c}"));
    } else if kind.unknown {
        acc.reasons.push("size class without its pair".to_owned());
    }
    // What the styles add (em of this element).
    let (mut ml, mut mr, mut pad, mut border, mut min_w, mut width_set) =
        (0.0, 0.0, 0.0, 0.0, 0.0, None);
    for (p, v) in span.style.iter() {
        match p {
            P::MarginLeft => {
                ml += em_value(v).unwrap_or_else(|| {
                    ok = false;
                    0.0
                })
            }
            P::MarginRight => {
                mr += em_value(v).unwrap_or_else(|| {
                    ok = false;
                    0.0
                })
            }
            P::Margin => match margin_shorthand(v) {
                Some((l, r)) => {
                    ml += l;
                    mr += r;
                }
                None => ok = false,
            },
            P::PaddingLeft => {
                pad += em_value(v).unwrap_or_else(|| {
                    ok = false;
                    0.0
                })
            }
            P::BorderRightWidth => {
                border += em_value(v).unwrap_or_else(|| {
                    ok = false;
                    0.0
                })
            }
            P::BorderWidth => {
                border += 2.0
                    * em_value(v).unwrap_or_else(|| {
                        ok = false;
                        0.0
                    })
            }
            P::MinWidth => {
                min_w = em_value(v).unwrap_or_else(|| {
                    ok = false;
                    0.0
                })
            }
            // a percentage (`calc(100% - .1em)` of a wide accent's svg) is not a width in a shrink-to-fit box: the content is
            P::Width if v.contains('%') => {}
            P::Width => {
                width_set = Some(em_value(v).unwrap_or_else(|| {
                    ok = false;
                    0.0
                }))
            }
            P::Height
            | P::VerticalAlign
            | P::Top
            | P::Bottom
            | P::Left
            | P::Position
            | P::Color
            | P::BackgroundColor
            | P::BorderColor
            | P::BorderStyle
            | P::BorderRightStyle
            | P::BorderTopWidth
            | P::BorderBottomWidth
            | P::TextShadow
            | P::MarginTop => {}
        }
    }
    let struts_here = kind.strut;
    if struts_here {
        let (mut h, mut v) = (None, 0.0);
        for (p, val) in span.style.iter() {
            match p {
                P::Height => h = em_value(val),
                P::VerticalAlign => v = em_value(val).unwrap_or(f64::NAN),
                _ => {}
            }
        }
        match h {
            Some(h) if v.is_finite() => {
                acc.strut_above = acc.strut_above.max(h + v);
                acc.strut_below = acc.strut_below.max(-v);
            }
            _ => acc.extents_ok = false,
        }
    }
    let inner = if kind.vlist {
        vlist_width(span, line, acc, depth)
    } else {
        // Inside an inline block the stacks do not sit in the formula's line.
        let child_line = line && !kind.sizing && !kind.nulldelimiter && !kind.percent && !kind.zero;
        let in_sqrt = span.classes.contains("sqrt");
        let text_before = acc.text_seen;
        let w = if kind.vbox {
            vbox_width(&mut span.children, acc, depth)
        } else {
            sum_children(&mut span.children, child_line, in_sqrt, acc, depth)
        };
        if child_line
            && !acc.hybrid
            && acc.text_seen > text_before
            && let Some(m) = span
                .classes
                .iter()
                .find_map(font_metrics)
                .or_else(|| delim_font(&span.classes))
        {
            acc.fonts.push(m);
        }
        if kind.zero {
            Some(0.0)
        } else if kind.nulldelimiter {
            Some(0.12)
        } else if kind.percent {
            // `width: 100%`: no more than its `min-width`, in a box that is as wide as its content
            Some(min_w)
        } else {
            w.map(|w| w.max(min_w))
        }
    };
    let own = if let Some(w) = width_set {
        Some(w)
    } else {
        inner.map(|w| w + kind.extra + pad + border)
    };
    let own = own?;
    if !ok {
        acc.reasons.push("style value that is not in em".to_owned());
        return None;
    }
    Some((own + ml + mr) * kind.ratio)
}

fn vbox_width(children: &mut [HtmlDomNode], acc: &mut Acc, depth: usize) -> Option<f64> {
    let mut max = Some(0.0f64);
    for c in children {
        let w = walk(c, false, false, acc, depth + 1);
        max = match (max, w) {
            (Some(a), Some(b)) => Some(a.max(b)),
            _ => None,
        };
    }
    max
}

/// A stack: an inline table whose width is that of the widest item, with the item's margins. Sets `width`.
fn vlist_width(span: &mut DomSpan, line: bool, acc: &mut Acc, depth: usize) -> Option<f64> {
    acc.stacks += 1;
    let mut max = Some(0.0f64);
    let mut items = 0usize;
    let mut plain = true;
    let (mut top_h, mut depth_h) = (None, None);
    let mut lone = None;
    for (ri, row) in span.children.iter_mut().enumerate() {
        let HtmlDomNode::DomSpan(row) = row else {
            max = None;
            continue;
        };
        for cell in &mut row.children {
            let HtmlDomNode::DomSpan(cell) = cell else {
                max = None;
                continue;
            };
            if cell.classes.contains("vlist-s") {
                continue;
            }
            let h = cell.style.get(P::Height).and_then(em_value);
            if ri == 0 {
                top_h = h;
            } else {
                depth_h = h;
            }
            for item in &mut cell.children {
                let HtmlDomNode::DomSpan(item) = item else {
                    max = None;
                    continue;
                };
                if ri != 0 {
                    continue;
                }
                items += 1;
                let (mut l, mut r) = (0.0, 0.0);
                let mut item_ok = true;
                for (p, v) in item.style.iter() {
                    match p {
                        P::MarginLeft => {
                            l = em_value(v).unwrap_or_else(|| {
                                item_ok = false;
                                0.0
                            })
                        }
                        P::MarginRight => {
                            r = em_value(v).unwrap_or_else(|| {
                                item_ok = false;
                                0.0
                            })
                        }
                        P::Top => {}
                        // see `span_width`
                        P::Width if v.contains('%') => {}
                        _ => item_ok = false,
                    }
                }
                if !item.classes.is_empty() && !item.classes.contains("svg-align") {
                    plain = false;
                }
                if item.children.iter().any(|c| {
                    !matches!(c, HtmlDomNode::DomSpan(s) if s.classes.contains("pstrut")
                        || s.classes.contains("katex-sizing") || s.classes.len() <= 3)
                        && !matches!(c, HtmlDomNode::Symbol(_))
                }) {
                    plain = false;
                }
                // the item's baseline is `pstrut height + top` below the formula's; its size span sets the font ratio
                let mut down = None;
                let mut ratio = 1.0;
                for c in &item.children {
                    if let HtmlDomNode::DomSpan(c) = c {
                        if c.classes.contains("pstrut") {
                            down = c.style.get(P::Height).and_then(em_value);
                        } else if c.classes.contains("katex-sizing") {
                            let k = classify(c, false);
                            if !k.unknown {
                                ratio = k.ratio;
                            }
                        }
                    }
                }
                let top = item.style.get(P::Top).and_then(em_value).unwrap_or(0.0);
                lone = down.map(|d| (d + top, ratio));
                let w = sum_children(&mut item.children, false, false, acc, depth + 1);
                max = match (max, w, item_ok) {
                    (Some(m), Some(w), true) => Some(m.max(l + w + r)),
                    _ => None,
                };
            }
        }
    }
    if line {
        // A stack is an inline block: it takes `padding-top` above the baseline and `height` below it. A lone item that is
        // plain text is not a stack in the output (a shifted inline element), and takes no room.
        match (top_h, depth_h.or(Some(0.0))) {
            (Some(t), Some(d)) => {
                if items == 1 && plain {
                    match lone {
                        Some(l) => acc.shifted.push(l),
                        None => acc.extents_ok = false,
                    }
                } else {
                    acc.stack_above = acc.stack_above.max(t);
                    acc.stack_below = acc.stack_below.max(d);
                }
            }
            _ => acc.extents_ok = false,
        }
    }
    if let Some(w) = max {
        span.width = Some(w);
        acc.known += 1;
    }
    max
}

impl FormulaSizes {
    /// The box that the formula takes in a line of text whose font is `host_px` (the formula's font is 1.21 times that, set by
    /// `.katex`): `(above, below, line_above)` in em of the formula, where `above` and `below` are the extents of the whole
    /// box (the line box of the formula's own font, the strut and the stacks in the line) and `line_above` is the baseline's
    /// distance from the top of a box that has no strut (what the line box and the stacks make). `None` if an extent could
    /// not be read.
    #[must_use]
    pub fn extents(&self, font_px: f64) -> Option<(f64, f64, f64)> {
        if !self.extents_known || font_px <= 0.0 {
            return None;
        }
        let (mut la, mut lb) = line_box(font_px);
        for (asc, desc) in &self.fonts {
            let (a, b) = font_line_box(font_px, *asc, *desc);
            la = la.max(a);
            lb = lb.max(b);
        }
        for (down, ratio) in &self.shifted {
            let px = font_px * ratio;
            let (a, b) = font_line_box_h(px, ASCENT, DESCENT, 0.0);
            // the point is `a` above the shifted baseline, which is `down` em of the formula below the formula's
            let y = a - down * font_px;
            la = la.max(y);
            lb = lb.max(-y);
            let _ = b;
        }
        let (la, lb) = (la / font_px, lb / font_px);
        let line_above = la.max(self.stack_above);
        Some((
            line_above.max(self.height),
            lb.max(self.depth).max(self.stack_below),
            line_above,
        ))
    }
}
