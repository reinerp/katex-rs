//! Glyph runs in a generated font for the compact output (the `FONT_RUN` step).
//!
//! KaTeX lays a formula out with elements: a font switch is a span, spacing an empty span with a margin, an italic
//! correction a margin on a letter, a script a size span and a vertical shift. The font made by
//! `tools/gen_math_font.py` has all of that in its glyphs: a letter, an operator, a digit at any of the sizes and baseline
//! shifts of the scripts that occur, and spacers of any advance, positive and negative.
//!
//! [`transform`] rewrites the built DOM tree of a formula (before [`crate::compact`] makes its markup of it): in every list
//! of sibling nodes, consecutive nodes that the font can express (symbols, space, kerns, italic corrections, lone and
//! stacked scripts, `\not`-style overlaps, limits centred over an operator) become one text node of code points of the
//! font. What it cannot express stays an element, and its children are treated the same way: so text and elements alternate,
//! and `a + \frac{x^2}{y_i} = \mathbf{b}` is a text node, a fraction whose rows are text, a text node, a span in a stock KaTeX
//! font and a text node.
//!
//! The layout is read the way a browser lays the tree out (inline flow, `vlist` tables, `em` margins and offsets) in em of
//! the element that holds the run, so that a run is placed exactly where the nodes it replaces were: every glyph is written
//! as its code point, and where it is not where the previous glyph ends there are spacer glyphs, negative ones included
//! (stacked scripts are one run: the superscript, a spacer back, the subscript).
//!
//! The formula's root then sets the font ([`stylesheet`]): the generated font first, the stock KaTeX font after it, so that
//! characters that are not in the generated font (stock text, bold letters, ...) are drawn by the stock fonts of their
//! classes as before. The generated font has only private-use code points, so a run is one script run: Chrome splits text
//! into runs by script and clamps the total advance of a run at zero, which would lose a negative spacer between two real
//! letters. The page must have `text-rendering: geometricPrecision` on the formula (the stylesheet does that): without it
//! Chrome on Linux rounds the advance of every glyph to a whole pixel, and the spacers with them.

use core::cell::RefCell;
use core::fmt::Write as _;

use crate::compact;
use crate::dom_tree::{DomSpan, HtmlDomNode, SymbolNode};
use crate::types::{ClassList, CssStyle, Settings};

include!("generated/mathrun_table.rs");

/// How deep a tree may nest before it is not read.
const MAX_DEPTH: usize = 256;

/// Whether text mode (`\text{...}`) is written as glyphs too. It can be (the KaTeX fonts have no kerning, so the stock text and
/// the glyphs are laid out alike), but then the text of a formula is private-use characters when selected or copied, and takes
/// three bytes a character. Off: text mode stays text in the stock font.
const TEXT_AS_GLYPHS: bool = false;

/// Layout positions are compared to this (em) when asking whether a glyph is at a variant's baseline shift.
const SHIFT_TOLERANCE: f64 = 0.0015;

/// The stylesheet of the compact output with the font rules.
///
/// The `@font-face` of the generated font (served from `font_url`; its file name is [`FONT_FILE_NAME`]), and, for a formula
/// with the class `mf`, the font first in its font family and in those of the classes that set one (`.katex.mf .N` ...),
/// `text-rendering: geometricPrecision` and no letter spacing (the font size and line height of `.katex` stay: the generated font has the ascent and descent of KaTeX_Main, so the line box of the formula is that of KaTeX's). With `blank_url` ([`BLANK_FONT_WOFF2`], 344 bytes) the spacers draw nothing while the font loads or
/// if it fails, where otherwise they are boxes in the fallback font (no measurable cost in layout time).
#[must_use]
pub fn stylesheet(font_url: &str, blank_url: Option<&str>) -> String {
    let compact = compact::stylesheet();
    let mut css = String::with_capacity(compact.len() + 4096);
    css.push_str(compact);
    let _ = writeln!(
        css,
        "@font-face{{font-family:{FONT_FAMILY};src:url({font_url})format(\"woff2\");font-display:block}}"
    );
    let first = if let Some(url) = blank_url {
        let _ = writeln!(
            css,
            "@font-face{{font-family:KaTeXMathBlank;src:url({url})format(\"woff2\");unicode-range:U+E000-F8FF,U+F0000-FFFFD}}"
        );
        format!("{FONT_FAMILY},KaTeXMathBlank")
    } else {
        FONT_FAMILY.to_owned()
    };
    let _ = writeln!(
        css,
        ".katex.mf{{font-family:{first},KaTeX_Main,Times New Roman,serif;text-rendering:geometricPrecision;letter-spacing:normal}}"
    );
    // The classes that set a font family: the generated font stays first.
    for line in compact.lines() {
        if !line.starts_with(".katex ") || !line.contains("font-family:KaTeX_") {
            continue;
        }
        let Some((selectors, body)) = line.split_once('{') else {
            continue;
        };
        let rule: Vec<String> = selectors
            .split(',')
            .map(|s| s.replacen(".katex ", ".katex.mf ", 1))
            .collect();
        let body = body.replace("font-family:", &format!("font-family:{first},"));
        let _ = writeln!(css, "{}{{{body}", rule.join(","));
    }
    css
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Main,
    Math,
    Ams,
    Size(u8),
    /// A font that the generated font does not have (bold, calligraphic, ...): its text stays text.
    Other,
}

/// What a span's classes set for the text inside it.
#[derive(Clone, Copy)]
struct Ctx {
    /// The font size relative to the element that holds the run.
    size: f64,
    /// The baseline's height above that element's baseline, in em of that element.
    y: f64,
    family: Family,
    italic: bool,
    /// Inside text mode (`\text{...}`): stock text, which stays selectable and readable.
    stock: bool,
}

#[derive(Clone)]
struct Glyph {
    font: u8,
    ch: u32,
    size: f64,
    x: f64,
    y: f64,
}

/// A node or nodes written as positioned glyphs.
struct Expr {
    glyphs: Vec<Glyph>,
    width: f64,
}

struct Walker {
    items: Vec<Glyph>,
    x: f64,
    /// Inside `rlap` or `llap`: a strut has no width there.
    in_lap: bool,
}

thread_local! {
    /// The (font, character, size, shift) of the variants asked for and not in the font, while collecting.
    static MISSING: RefCell<Option<Vec<MissingVariant>>> = const { RefCell::new(None) };
}

/// A variant of a glyph that a formula needed and the font does not have: (font, character, size and shift in
/// thousandths of the em of the element that holds the run).
pub type MissingVariant = (u8, u32, u16, i16);

/// Runs `f` and returns what it returned with the variants that were needed and are not in the font (for deriving the
/// sizes and shifts of a new font from a corpus: `tools/gen_math_font.py`, `docs/FONT.md`).
#[doc(hidden)]
pub fn collect_missing_variants<R>(f: impl FnOnce() -> R) -> (R, Vec<MissingVariant>) {
    MISSING.with(|m| *m.borrow_mut() = Some(Vec::new()));
    let result = f();
    let missing = MISSING.with(|m| m.borrow_mut().take()).unwrap_or_default();
    (result, missing)
}

/// `value` of `em`, or `None` if it is anything else.
fn em_value(value: &str) -> Option<f64> {
    let n = value.strip_suffix("em")?;
    n.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn font_id(family: Family, italic: bool) -> Option<u8> {
    match (family, italic) {
        (Family::Main, false) => Some(0),
        (Family::Math, _) => Some(1),
        (Family::Ams, false) => Some(2),
        (Family::Size(n @ 1..=4), _) => Some(2 + n),
        _ => None,
    }
}

fn base_entry(font: u8, ch: u32) -> Option<&'static (u8, u32, u32, i32, i32, u32, u32)> {
    let i = BASE
        .binary_search_by(|e| (e.0, e.1).cmp(&(font, ch)))
        .ok()?;
    BASE.get(i)
}

/// The variant of a glyph at a size and shift, if the font has one within [`SHIFT_TOLERANCE`].
fn variant_entry(
    font: u8,
    ch: u32,
    size: f64,
    y: f64,
) -> Option<&'static (u8, u32, u16, i16, u32, i32)> {
    let size_milli = (size * 1000.0).round();
    if !(1.0..=65535.0).contains(&size_milli) {
        return None;
    }
    let key = (font, ch, size_milli as u16);
    let start = VARIANTS.partition_point(|e| (e.0, e.1, e.2) < key);
    let mut best: Option<(&(u8, u32, u16, i16, u32, i32), f64)> = None;
    for e in VARIANTS.get(start..)? {
        if (e.0, e.1, e.2) != key {
            break;
        }
        let d = (f64::from(e.3) / 1000.0 - y).abs();
        if d <= SHIFT_TOLERANCE && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((e, d));
        }
    }
    best.map(|(e, _)| e)
}

/// The classes of a span or symbol: font, size and position changes it makes. `strict`: `None` for any class this module
/// does not know or whose font it does not have (the node is not one to write as glyphs); otherwise those are ignored, and
/// the result is for the children of an element that stays (their size is relative to it, so it is not changed). The second
/// result: whether the element is `position: relative` (so that `top` moves it).
fn read_classes<'a, I: Iterator<Item = &'a str>>(
    classes: I,
    mut ctx: Ctx,
    strict: bool,
) -> Option<(Ctx, bool)> {
    let (mut sizing, mut reset, mut size) = (false, None, None);
    let (mut delim, mut op_symbol, mut small, mut large) = (false, false, false, false);
    let mut relative = false;
    for c in classes {
        match c {
            "katex-sizing" | "fontsize-ensurer" => sizing = true,
            "delimsizing" => delim = true,
            "op-symbol" => {
                op_symbol = true;
                relative = true;
            }
            "small-op" => small = true,
            "large-op" => large = true,
            "delimcenter" => relative = true,
            "mathnormal" => {
                ctx.family = Family::Math;
                ctx.italic = true;
            }
            "mathrm" => ctx.italic = false,
            "textrm" => ctx.family = Family::Main,
            "textit" => ctx.italic = true,
            "amsrm" | "mathbb" | "textbb" => ctx.family = Family::Ams,
            // Classes that no rule of katex.css mentions, or that only structure the markup.
            "text" if !TEXT_AS_GLYPHS => {
                if strict {
                    return None;
                }
                ctx.stock = true;
            }
            "text" => {}
            "mord" | "mbin" | "mrel" | "mopen" | "mclose" | "mpunct" | "minner" | "mop"
            | "mtight" | "allowbreak" | "nobreak" | "mspace" | "msupsub" | "op-limits"
            | "katex-vbox" | "katex-thinbox" | "katex-inner" | "katex-fix" | "rlap" | "llap"
            | "nulldelimiter" => {}
            "mathbf" | "boldsymbol" | "textbf" | "mathit" | "mainrm" | "texttt" | "mathtt"
            | "textsf" | "mathsf" | "mathboldsf" | "textboldsf" | "mathsfit" | "mathitsf"
            | "textitsf" | "mathcal" | "mathfrak" | "textfrak" | "mathboldfrak"
            | "textboldfrak" | "mathscr" | "textscr" => {
                if strict {
                    return None;
                }
                ctx.family = Family::Other;
            }
            _ => {
                if let Some(n) = c.strip_prefix("reset-size") {
                    reset = n.parse::<usize>().ok().filter(|n| (1..=11).contains(n));
                } else if let Some(n) = c.strip_prefix("size") {
                    size = n.parse::<usize>().ok().filter(|n| (1..=11).contains(n));
                } else if strict {
                    return None;
                }
            }
        }
    }
    if delim {
        // `delimsizing size1`: the size class names a font here.
        if let Some(n) = size
            .take()
            .and_then(|n| u8::try_from(n).ok())
            .filter(|n| (1..=4).contains(n))
        {
            ctx.family = Family::Size(n);
        } else {
            ctx.family = Family::Other;
        }
        reset = None;
    } else if sizing {
        const MULT: [f64; 11] = [0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.2, 1.44, 1.728, 2.074, 2.488];
        match (size.take(), reset.take()) {
            (Some(s), Some(r)) => {
                if strict {
                    ctx.size *= MULT[s - 1] / MULT[r - 1];
                }
            }
            _ => {
                if strict {
                    return None;
                }
            }
        }
    }
    if strict && (size.is_some() || reset.is_some()) {
        return None;
    }
    if op_symbol {
        if small == large {
            if strict {
                return None;
            }
            ctx.family = Family::Other;
        } else {
            ctx.family = Family::Size(if small { 1 } else { 2 });
        }
    } else if (small || large) && strict {
        return None;
    }
    if !strict {
        ctx.size = 1.0;
        ctx.y = 0.0;
    }
    Some((ctx, relative))
}

impl Walker {
    fn node(&mut self, node: &HtmlDomNode, ctx: Ctx, depth: usize) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        match node {
            HtmlDomNode::Symbol(symbol) => self.symbol(symbol, ctx),
            HtmlDomNode::DomSpan(span) => self.span(span, ctx, depth),
            HtmlDomNode::Fragment(fragment) => {
                for child in &fragment.children {
                    self.node(child, ctx, depth + 1)?;
                }
                Some(())
            }
            _ => None,
        }
    }

    fn symbol(&mut self, symbol: &SymbolNode, ctx: Ctx) -> Option<()> {
        let (mut ctx, mut relative) = read_classes(symbol.classes.iter(), ctx, true)?;
        let (mut left, mut right, mut top) = (0.0, 0.0, 0.0);
        for (property, value) in symbol.style.iter() {
            match property.as_str() {
                "margin-left" => left = em_value(value)?,
                "margin-right" => right = em_value(value)?,
                "top" => top = em_value(value)?,
                "position" if value == "relative" => relative = true,
                _ => return None,
            }
        }
        if top != 0.0 {
            if !relative {
                return None;
            }
            ctx.y -= top * ctx.size;
        }
        self.x += left * ctx.size;
        for ch in symbol.text.chars() {
            if ch == '\u{200b}' {
                continue;
            }
            let font = font_id(ctx.family, ctx.italic)?;
            let entry = base_entry(font, u32::from(ch))?;
            let glyph = Glyph {
                font,
                ch: u32::from(ch),
                size: ctx.size,
                x: self.x,
                y: ctx.y,
            };
            lookup(&glyph, false, false)?;
            self.items.push(glyph);
            self.x += f64::from(entry.3 - entry.4) / UPEM as f64 * ctx.size;
        }
        self.x += (symbol.italic + right) * ctx.size;
        Some(())
    }

    fn span(&mut self, span: &DomSpan, ctx: Ctx, depth: usize) -> Option<()> {
        if span.attributes.iter().next().is_some() || span.is_middle.is_some() {
            return None;
        }
        if span.classes.contains("vlist-t") {
            // Only as the content of `msupsub` and `op-limits` (below): in a column, a fraction, ... the text alignment of the
            // parent decides where the items are.
            return None;
        }
        if span.classes.len() == 1
            && span.classes.contains("katex-strut")
            && span.children.is_empty()
            && self.in_lap
        {
            // A strut inside a zero-width box (`\not`): it gives the box its height, and no width.
            return Some(());
        }
        let (mut ctx, mut relative) = read_classes(span.classes.iter(), ctx, true)?;
        let (mut left, mut right, mut top) = (0.0, 0.0, 0.0);
        for (property, value) in span.style.iter() {
            match property.as_str() {
                "margin-left" => left = em_value(value)?,
                "margin-right" => right = em_value(value)?,
                "top" => top = em_value(value)?,
                "position" if value == "relative" => relative = true,
                _ => return None,
            }
        }
        if top != 0.0 {
            if !relative {
                return None;
            }
            ctx.y -= top * ctx.size;
        }
        self.x += left * ctx.size;
        if span.classes.contains("nulldelimiter") {
            // `\left.`: an empty inline block 0.12em wide.
            if !span.children.is_empty() {
                return None;
            }
            self.x += 0.12 * ctx.size;
        }
        let lap = span.classes.contains("rlap") || span.classes.contains("llap");
        let (start_x, start_items) = (self.x, self.items.len());
        let limits = span.classes.contains("op-limits");
        let scripts = span.classes.contains("msupsub");
        let was_in_lap = self.in_lap;
        self.in_lap |= lap;
        for child in &span.children {
            match child {
                HtmlDomNode::DomSpan(table)
                    if (limits || scripts) && table.classes.contains("vlist-t") =>
                {
                    self.vlist(table, ctx, depth + 1, limits)?;
                }
                _ => self.node(child, ctx, depth + 1)?,
            }
        }
        self.in_lap = was_in_lap;
        if lap {
            // `rlap` and `llap` have no width: what is inside is at the right of the point (`rlap`, starting there) or at its
            // left (`llap`, ending there).
            if span.classes.contains("llap") {
                let width = self.x - start_x;
                for g in &mut self.items[start_items..] {
                    g.x -= width;
                }
            }
            self.x = start_x;
        }
        self.x += right * ctx.size;
        Some(())
    }

    /// A `vlist-t`: an inline table whose first row holds one zero-height block per item, each at its own `top`
    /// below the baseline by the height of its `pstrut`, and a second row that only gives it a depth.
    fn vlist(&mut self, table: &DomSpan, ctx: Ctx, depth: usize, centered: bool) -> Option<()> {
        if !table.style.is_empty() || table.attributes.iter().next().is_some() {
            return None;
        }
        let (table_ctx, _) = read_classes(
            table
                .classes
                .iter()
                .filter(|c| *c != "vlist-t" && *c != "vlist-t2"),
            ctx,
            true,
        )?;
        let mut spacers = 0;
        let mut first: Option<&DomSpan> = None;
        for (i, row) in table.children.iter().enumerate() {
            let HtmlDomNode::DomSpan(row) = row else {
                return None;
            };
            if row.classes.len() != 1 || !row.classes.contains("vlist-r") || !row.style.is_empty() {
                return None;
            }
            for cell in &row.children {
                let HtmlDomNode::DomSpan(cell) = cell else {
                    return None;
                };
                if cell.classes.len() == 1 && cell.classes.contains("vlist-s") {
                    spacers += 1;
                } else if cell.classes.len() == 1 && cell.classes.contains("vlist") {
                    if i == 0 {
                        first = Some(cell);
                    }
                } else {
                    return None;
                }
            }
        }
        // The spacer column is 2 px wide and the `vlist-t2` margin takes it back.
        if spacers != usize::from(table.classes.contains("vlist-t2")) {
            return None;
        }
        let list = first?;
        let start = self.x;
        let mut width = 0.0f64;
        // Items of `.op-limits > .vlist-t` are centred in the width of the table: (first glyph, end, left margin, content, right margin).
        let mut spans: Vec<(usize, usize, f64, f64, f64)> = Vec::new();
        for item in &list.children {
            let HtmlDomNode::DomSpan(item) = item else {
                return None;
            };
            if !item.classes.is_empty() || item.attributes.iter().next().is_some() {
                return None;
            }
            let (mut left, mut right, mut top) = (0.0, 0.0, 0.0);
            for (property, value) in item.style.iter() {
                match property.as_str() {
                    "margin-left" => left = em_value(value)?,
                    "margin-right" => right = em_value(value)?,
                    "top" => top = em_value(value)?,
                    _ => return None,
                }
            }
            let mut kids = item.children.iter();
            let Some(HtmlDomNode::DomSpan(pstrut)) = kids.next() else {
                return None;
            };
            if !(pstrut.classes.len() == 1
                && pstrut.classes.contains("pstrut")
                && pstrut.children.is_empty())
            {
                return None;
            }
            let mut pheight = None;
            for (property, value) in pstrut.style.iter() {
                if property.as_str() == "height" {
                    pheight = Some(em_value(value)?);
                } else {
                    return None;
                }
            }
            let below = pheight? + top;
            let item_ctx = Ctx {
                y: table_ctx.y - below * table_ctx.size,
                ..table_ctx
            };
            self.x = start + left * table_ctx.size;
            let first = self.items.len();
            let content_start = self.x;
            for child in kids {
                self.node(child, item_ctx, depth + 1)?;
            }
            let (ml, mr) = (left * table_ctx.size, right * table_ctx.size);
            spans.push((first, self.items.len(), ml, self.x - content_start, mr));
            width = width.max(self.x + mr - start);
        }
        if centered {
            for (from, to, ml, content, mr) in spans {
                let delta = (width - ml - mr - content) / 2.0;
                for g in &mut self.items[from..to] {
                    g.x += delta;
                }
            }
        }
        self.x = start + width;
        Some(())
    }
}

// ---------------------------------------------------------------------------------------------
// Encoding

fn popcount_cost(units: i64) -> usize {
    let n = units.unsigned_abs();
    let max = 1u64 << 14;
    (n / max) as usize + (n % max).count_ones() as usize
}

/// Code points that move the cursor by `units`, to within `tol` (the rest as binary spacers); the amount moved.
fn spacer(units: i64, tol: i64) -> (Vec<u32>, i64) {
    if units.abs() <= tol {
        return (Vec::new(), 0);
    }
    let mut best: Option<(usize, i64, Option<u32>, i64)> = None;
    let candidates = std::iter::once((0i64, None)).chain(
        NAMED
            .iter()
            .filter(|(n, _)| (*n > 0) == (units > 0) && i64::from(n.abs()) <= units.abs() + tol)
            .map(|(n, cp)| (i64::from(*n), Some(*cp))),
    );
    for (n, cp) in candidates {
        let mut rem = units - n;
        if rem.abs() <= tol {
            rem = 0;
        } else if rem.abs() % 2 == 1 {
            rem += rem.signum();
        }
        let cost = usize::from(cp.is_some()) + popcount_cost(rem);
        if best.is_none_or(|(c, ..)| cost < c) {
            best = Some((cost, n, cp, rem));
        }
    }
    let Some((_, n, cp, rem)) = best else {
        return (Vec::new(), 0);
    };
    let mut out = Vec::new();
    let mut moved = 0;
    if let Some(cp) = cp {
        out.push(cp);
        moved += n;
    }
    let table = if rem > 0 { &POS } else { &NEG };
    let mut left = rem.abs();
    let mut k = 14usize;
    while left > 0 {
        while (1i64 << k) > left {
            k -= 1;
        }
        out.push(table[k]);
        left -= 1 << k;
        moved += rem.signum() * (1 << k);
    }
    (out, moved)
}

/// The code point and advance of a glyph, if the font has it there.
fn lookup(g: &Glyph, no_ic: bool, alt: bool) -> Option<(u32, i64)> {
    if (g.size - 1.0).abs() < 1e-6 && g.y.abs() < 0.0006 {
        let e = base_entry(g.font, g.ch)?;
        if no_ic && e.5 != 0 {
            return Some((e.5, i64::from(e.3 - e.4)));
        }
        if alt && e.6 != 0 {
            return Some((e.6, i64::from(e.3)));
        }
        return Some((e.2, i64::from(e.3)));
    }
    let Some(e) = variant_entry(g.font, g.ch, g.size, g.y) else {
        MISSING.with(|m| {
            if let Some(list) = m.borrow_mut().as_mut() {
                list.push((
                    g.font,
                    g.ch,
                    (g.size * 1000.0).round() as u16,
                    (g.y * 1000.0).round() as i16,
                ));
            }
        });
        return None;
    };
    Some((e.4, i64::from(e.5)))
}

fn units(em: f64) -> i64 {
    (em * UPEM as f64).round() as i64
}

/// Writes glyphs as code points, in document order or ordered by x.
fn encode(glyphs: &[Glyph], width: f64, by_x: bool, alt: bool) -> Option<Vec<u32>> {
    let mut order: Vec<&Glyph> = glyphs.iter().collect();
    if by_x {
        let mut keyed: Vec<(usize, &Glyph)> = order.into_iter().enumerate().collect();
        keyed.sort_by(|a, b| {
            let ka = (a.1.x * 1e4).round();
            let kb = (b.1.x * 1e4).round();
            ka.partial_cmp(&kb)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(a.0.cmp(&b.0))
        });
        order = keyed.into_iter().map(|(_, g)| g).collect();
    }
    let mut out = Vec::with_capacity(order.len() * 2);
    let mut cursor = 0i64;
    let tol = 1;
    for (pos, g) in order.iter().enumerate() {
        let xu = units(g.x);
        let (cps, moved) = spacer(xu - cursor, tol);
        cursor += moved;
        out.extend(cps);
        // A subscript that starts where the glyph would end without its italic correction.
        let mut no_ic = false;
        if (g.size - 1.0).abs() < 1e-6
            && g.y.abs() < 0.0006
            && let Some(e) = base_entry(g.font, g.ch)
            && e.4 > 0
            && e.5 != 0
            && let Some(next) = order.get(pos + 1)
        {
            let nx = units(next.x);
            no_ic = (nx - (xu + i64::from(e.3 - e.4))).abs() < (nx - (xu + i64::from(e.3))).abs();
        }
        let (cp, adv) = lookup(g, no_ic, alt)?;
        out.push(cp);
        cursor += adv;
    }
    let (cps, _) = spacer(units(width) - cursor, tol);
    out.extend(cps);
    Some(out)
}

/// The shortest writing: document order or by x, with the private-use twins of letters where a negative spacer is in it
/// and the font has twins (a font with real letter code points).
fn encode_best(glyphs: &[Glyph], width: f64) -> Option<Vec<u32>> {
    let negative = |cps: &[u32]| {
        cps.iter()
            .any(|cp| NEG.contains(cp) || NAMED.iter().any(|(n, c)| *n < 0 && c == cp))
    };
    let mut best: Option<Vec<u32>> = None;
    for by_x in [false, true] {
        let mut cps = encode(glyphs, width, by_x, false)?;
        if negative(&cps) && BASE.iter().any(|e| e.6 != 0) {
            cps = encode(glyphs, width, by_x, true)?;
        }
        if best.as_ref().is_none_or(|b| cps.len() < b.len()) {
            best = Some(cps);
        }
    }
    best
}

// ---------------------------------------------------------------------------------------------
// The tree

/// The tree with glyph runs in it, and the shortest formula (in characters of KaTeX's text) that may wrap, as
/// `compact::flatten` has to count it before the text is code points.
pub(crate) struct Hybrid {
    pub root: DomSpan,
    pub wrap_min: usize,
}

fn count_chars(node: &HtmlDomNode) -> usize {
    match node {
        HtmlDomNode::Symbol(s) => s.text.chars().count(),
        HtmlDomNode::DomSpan(s) => s.children.iter().map(count_chars).sum(),
        HtmlDomNode::Anchor(a) => a.children.iter().map(count_chars).sum(),
        HtmlDomNode::Fragment(f) => f.children.iter().map(count_chars).sum(),
        _ => 0,
    }
}

fn run_node(cps: &[u32], width: f64) -> HtmlDomNode {
    let text: String = cps.iter().filter_map(|c| char::from_u32(*c)).collect();
    HtmlDomNode::Symbol(SymbolNode {
        is_text_ord: false,
        text,
        height: 0.0,
        depth: 0.0,
        italic: 0.0,
        skew: 0.0,
        // the run's advance, in em of the element that holds it (for `crate::sizes`)
        width,
        max_font_size: 0.0,
        classes: ClassList::default(),
        style: CssStyle::default(),
    })
}

/// Treats the formula `root` as described in the module documentation. `None` if it is not one to treat.
pub(crate) fn transform(root: &DomSpan, settings: &Settings) -> Option<Hybrid> {
    if settings.fleqn || !(root.classes.contains("katex") || root.classes.contains("katex-display"))
    {
        return None;
    }
    let chars: usize = root.children.iter().map(count_chars).sum();
    let ctx = Ctx {
        size: 1.0,
        y: 0.0,
        family: Family::Main,
        italic: false,
        stock: false,
    };
    let new = emit_span(root, ctx, 0);
    Some(Hybrid {
        root: new,
        wrap_min: if chars >= settings.compact_wrap_min {
            0
        } else {
            usize::MAX
        },
    })
}

/// `node` as the glyphs the font has for it, if it is all of that.
fn express(node: &HtmlDomNode, ctx: Ctx) -> Option<Expr> {
    if ctx.stock {
        return None;
    }
    let mut walker = Walker {
        items: Vec::new(),
        x: 0.0,
        in_lap: false,
    };
    let ctx = Ctx {
        size: 1.0,
        y: 0.0,
        ..ctx
    };
    walker.node(node, ctx, 0)?;
    Some(Expr {
        glyphs: walker.items,
        width: walker.x,
    })
}

/// A span that stays an element, its children treated by [`children`].
fn emit_span(span: &DomSpan, ctx: Ctx, depth: usize) -> DomSpan {
    // the span without its children (they are written, or copied, below: not cloned twice)
    let mut new = DomSpan {
        children: Vec::new(),
        attributes: span.attributes.clone(),
        classes: span.classes.clone(),
        style: span.style.clone(),
        is_middle: span.is_middle.clone(),
        ..*span
    };
    new.children = if depth < MAX_DEPTH
        && let Some((inner, _)) = read_classes(span.classes.iter(), ctx, false)
    {
        children(&span.children, inner, depth + 1)
    } else {
        span.children.clone()
    };
    new
}

fn emit(node: &HtmlDomNode, ctx: Ctx, depth: usize) -> HtmlDomNode {
    match node {
        HtmlDomNode::DomSpan(span) => HtmlDomNode::DomSpan(emit_span(span, ctx, depth)),
        other => other.clone(),
    }
}

/// The nodes of a list with the children of document fragments (which are no element) in their place.
fn flatten<'a>(list: &'a [HtmlDomNode], out: &mut Vec<&'a HtmlDomNode>) {
    for node in list {
        if let HtmlDomNode::Fragment(fragment) = node {
            flatten(&fragment.children, out);
        } else {
            out.push(node);
        }
    }
}

/// A list of sibling nodes: runs of the nodes the font can write become one text node each, the others stay.
fn children(list: &[HtmlDomNode], ctx: Ctx, depth: usize) -> Vec<HtmlDomNode> {
    let mut out = Vec::with_capacity(list.len());
    let mut pending: Vec<(&HtmlDomNode, Expr)> = Vec::new();
    let mut flat = Vec::with_capacity(list.len());
    flatten(list, &mut flat);
    for node in flat {
        if let Some(expr) = express(node, ctx) {
            pending.push((node, expr));
        } else {
            flush(&mut pending, &mut out, ctx, depth);
            out.push(emit(node, ctx, depth));
        }
    }
    flush(&mut pending, &mut out, ctx, depth);
    out
}

/// The pending nodes (all expressible): one run if there is a glyph in them, else they are as they were (space).
fn flush(
    pending: &mut Vec<(&HtmlDomNode, Expr)>,
    out: &mut Vec<HtmlDomNode>,
    ctx: Ctx,
    depth: usize,
) {
    let nodes = core::mem::take(pending);
    let start = 0;
    if !nodes.iter().any(|(_, e)| !e.glyphs.is_empty()) {
        out.extend(nodes.iter().map(|(n, _)| (*n).clone()));
        return;
    }
    let mut glyphs = Vec::new();
    let mut offset = 0.0;
    for (_, e) in &nodes[start..] {
        glyphs.extend(e.glyphs.iter().map(|g| Glyph {
            x: g.x + offset,
            ..g.clone()
        }));
        offset += e.width;
    }
    if offset < 0.0 {
        // a text node's total advance must not be negative: Chrome clamps it at zero
        out.extend(nodes[start..].iter().map(|(n, _)| emit(n, ctx, depth)));
        return;
    }
    if let Some(cps) = encode_best(&glyphs, offset) {
        out.push(run_node(&cps, offset));
    } else {
        out.extend(nodes[start..].iter().map(|(n, _)| emit(n, ctx, depth)));
    }
}
