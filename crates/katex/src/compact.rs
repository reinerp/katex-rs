//! Compact HTML output ([`OutputFormat::CompactHtml`]).
//!
//! KaTeX's HTML is built for generality: every atom is a `span` with several class names, most
//! spacing is an empty `span` with a `style` attribute, and a subscript costs a dozen nested
//! elements. This module rewrites the finished DOM tree into an equivalent one that browsers
//! draw the same way with far fewer, smaller elements. The result needs [`stylesheet`] (a
//! superset of KaTeX's `katex.css`), not `katex.css` itself.
//!
//! The rewrite works on a small owned tree ([`El`]), in steps that can be switched off with
//! [`levels`] bits (they exist so the gain of each can be measured, and so a pixel difference
//! can be bisected). Each step only changes markup whose effect on layout is known from
//! `katex.css`; see the comments at each step for why it is neutral. Trees nested deeper than
//! [`MAX_DEPTH`] are written as ordinary KaTeX markup instead, which the stylesheet also covers.

use core::fmt::Write as _;
use core::mem;

use crate::ParseError;
use crate::dom_tree::{DomSpan, HtmlDomNode, SvgChildNode};
use crate::mathrun;
use crate::svg_geometry::PATH_MAP;
use crate::types::{ParseErrorKind, Settings};
use crate::units::make_em;
use crate::utils::escape_into;

/// Bits of [`Settings::compact_levels`].
pub mod levels {
    /// Drop the `katex-html` wrapper element.
    pub const UNWRAP_HTML: u32 = 1 << 0;
    /// Drop class names that no CSS rule uses (`mord`, `mtight`, ...).
    pub const DEAD_CLASSES: u32 = 1 << 1;
    /// Use short class names.
    pub const SHORT_NAMES: u32 = 1 << 2;
    /// Replace common inline `style` declarations with classes.
    pub const VALUE_CLASSES: u32 = 1 << 3;
    /// Shorten numbers (`0.5em` to `.5em`) and leave out the last `;` of a `style`.
    pub const NUM_TRIM: u32 = 1 << 4;
    /// Turn empty spacing spans into margins of their neighbours.
    pub const GLUE: u32 = 1 << 5;
    /// Remove wrapper elements that have no effect.
    pub const UNWRAP: u32 = 1 << 6;
    /// Merge an element with its only child where that changes nothing.
    pub const MERGE: u32 = 1 << 7;
    /// Merge adjacent text spans that are styled alike.
    pub const TEXT: u32 = 1 << 8;
    /// Replace the vertical list tables with inline blocks.
    pub const VLIST: u32 = 1 << 9;
    /// Replace the struts' elements with pseudo-elements.
    pub const PSEUDO: u32 = 1 << 10;
    /// Also remove wrappers that hold only text, which then sits in its parent's text run.
    ///
    /// Glyphs are then placed by the sum of unrounded advances and not by sums of layout units that
    /// are rounded per element, so one in a few hundred glyphs lands a pixel away.
    pub const UNWRAP_TEXT: u32 = 1 << 11;
    /// A vertical list of one item is one element, not two.
    pub const MERGE_ITEM: u32 = 1 << 12;
    /// A size span that holds one run of text takes the run's classes.
    pub const MERGE_CONTENT: u32 = 1 << 13;
    /// Frequent groups of declarations of a vertical list (its size, and its item's place) have
    /// one class each.
    pub const COMBO: u32 = 1 << 14;
    /// The steps of the first design, which keeps the formula's `katex-base` blocks. Their output
    /// is exactly what it was before [`FLAT`] and the steps after it were added.
    pub const CLASSIC: u32 = (1 << 15) - 1;
    /// No `katex-base` inline blocks: the formula's content sits directly in the `katex` element
    /// with `white-space: nowrap`, a `<wbr>` between what were bases (inline formulas only, so they
    /// wrap where KaTeX's do), and one strut of the union of the bases' struts. Needs
    /// [`UNWRAP_HTML`]; a formula that has a `\newline` or tag keeps its bases.
    pub const FLAT: u32 = 1 << 16;
    /// A vertical list of one item (a lone subscript or superscript, a raised box) is an inline
    /// element shifted with `position: relative`, not a stack. Needs [`VLIST`].
    pub const SINGLE: u32 = 1 << 17;
    /// An empty spacing span is an ordinary inline element with a margin, not an inline block.
    pub const INLINE_GLUE: u32 = 1 << 18;
    /// Every step.
    pub const ALL: u32 = CLASSIC | FLAT | SINGLE | INLINE_GLUE;
    /// Text nodes of glyphs of a generated font in place of what it can express (`crate::mathrun`).
    ///
    /// Symbols, space and kerns, italic corrections and scripts (also stacked) that the font has become one text node of its
    /// code points; what it cannot express stays markup as the other steps write it, and its children are treated the same way.
    /// Needs [`FLAT`] and [`UNWRAP_HTML`]. Not in [`ALL`]: the page must serve the font and `mathrun::stylesheet`.
    pub const FONT_RUN: u32 = 1 << 19;
    /// Round the vertical extents too (`height`, `padding-top`, `vertical-align`).
    ///
    /// They set how tall a formula is in a line of text; with a precision set, only margins, offsets and
    /// widths are rounded unless this is on. Not part of [`ALL`].
    pub const ROUND_EXTENTS: u32 = 1 << 15;
    /// The steps after which every formula of the test corpus is drawn with the very same pixels
    /// as KaTeX's, at scale 1 and 2.
    ///
    /// The others (spacing as margins, removing wrappers, joining
    /// text) leave every glyph where it was to 1/64 px but let about one formula in twenty
    /// anti-alias a glyph differently at scale 2.
    pub const EXACT: u32 = CLASSIC & !(UNWRAP_TEXT | TEXT | UNWRAP | GLUE | MERGE | MERGE_CONTENT);
}

/// The default [`Settings::compact_precision`].
pub const DEFAULT_PRECISION: u32 = 0;

/// The default [`Settings::compact_wrap_min`].
pub const DEFAULT_WRAP_MIN: usize = 8;

/// Trees deeper than this are not rewritten.
const MAX_DEPTH: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Span,
    A,
    Img,
    Svg,
    Path,
    Line,
    /// A line break opportunity (`<wbr>`), written without `/`.
    Wbr,
}

impl Tag {
    const fn name(self) -> &'static str {
        match self {
            Self::Span => "span",
            Self::A => "a",
            Self::Img => "img",
            Self::Svg => "svg",
            Self::Path => "path",
            Self::Line => "line",
            Self::Wbr => "wbr",
        }
    }

    const fn is_void(self) -> bool {
        matches!(self, Self::Img | Self::Path | Self::Line | Self::Wbr)
    }
}

#[derive(Debug, Clone)]
enum Node {
    Text(String),
    El(El),
}

#[derive(Debug, Clone)]
struct El {
    tag: Tag,
    classes: Vec<String>,
    style: Vec<(&'static str, String)>,
    attrs: Vec<(String, String)>,
    kids: Vec<Node>,
}

impl El {
    fn has(&self, class: &str) -> bool {
        self.classes.iter().any(|c| c == class)
    }

    fn get_style(&self, prop: &str) -> Option<&str> {
        self.style
            .iter()
            .find(|(p, _)| *p == prop)
            .map(|(_, v)| v.as_str())
    }
}

/// Converts a tree; `None` if it is nested too deeply or holds a node this module does not know.
fn convert(node: &HtmlDomNode, depth: usize, sf: Rounding) -> Option<Node> {
    if depth > MAX_DEPTH {
        return None;
    }
    Some(match node {
        HtmlDomNode::DomSpan(span) => {
            let mut kids = Vec::with_capacity(span.children.len());
            for child in &span.children {
                push_node(&mut kids, convert(child, depth + 1, sf)?);
            }
            let mut style: Vec<(&'static str, String)> = span
                .style
                .iter()
                .map(|(p, v)| (p.as_str(), sf.apply(p.as_str(), v)))
                .collect();
            if let Some(w) = span.width
                && span.classes.contains("vlist-t")
            {
                // `crate::sizes` computed it; `to_inline_block` makes it the `width` of the stack
                style.push(("--sz", ceil_em(w)));
            }
            Node::El(El {
                tag: Tag::Span,
                classes: span.classes.iter().map(str::to_owned).collect(),
                style,
                attrs: span
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                kids,
            })
        }
        HtmlDomNode::Anchor(anchor) => {
            let mut kids = Vec::with_capacity(anchor.children.len());
            for child in &anchor.children {
                push_node(&mut kids, convert(child, depth + 1, sf)?);
            }
            Node::El(El {
                tag: Tag::A,
                classes: anchor.classes.iter().map(str::to_owned).collect(),
                style: anchor
                    .style
                    .iter()
                    .map(|(p, v)| (p.as_str(), sf.apply(p.as_str(), v)))
                    .collect(),
                attrs: anchor
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                kids,
            })
        }
        HtmlDomNode::Img(img) => Node::El(El {
            tag: Tag::Img,
            classes: img.classes.iter().map(str::to_owned).collect(),
            style: img
                .style
                .iter()
                .map(|(p, v)| (p.as_str(), sf.apply(p.as_str(), v)))
                .collect(),
            attrs: vec![
                ("src".to_owned(), img.src.clone()),
                ("alt".to_owned(), img.alt.clone()),
            ],
            kids: Vec::new(),
        }),
        HtmlDomNode::Symbol(symbol) => symbol_node(symbol, sf),
        HtmlDomNode::SvgNode(svg) => {
            let mut kids = Vec::with_capacity(svg.children.len());
            for child in &svg.children {
                match child {
                    SvgChildNode::Path(path) => {
                        let d = path.alternate.clone().unwrap_or_else(|| {
                            PATH_MAP
                                .get(&path.path_name)
                                .map_or_else(String::new, |s| (*s).to_owned())
                        });
                        kids.push(Node::El(El {
                            tag: Tag::Path,
                            classes: Vec::new(),
                            style: Vec::new(),
                            attrs: vec![("d".to_owned(), d)],
                            kids: Vec::new(),
                        }));
                    }
                    SvgChildNode::Line(line) => kids.push(Node::El(El {
                        tag: Tag::Line,
                        classes: Vec::new(),
                        style: Vec::new(),
                        attrs: line
                            .attributes
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                        kids: Vec::new(),
                    })),
                }
            }
            let mut attrs = vec![("xmlns".to_owned(), "http://www.w3.org/2000/svg".to_owned())];
            attrs.extend(svg.attributes.iter().map(|(k, v)| (k.clone(), v.clone())));
            Node::El(El {
                tag: Tag::Svg,
                classes: Vec::new(),
                style: Vec::new(),
                attrs,
                kids,
            })
        }
        HtmlDomNode::Fragment(fragment) => {
            // A fragment has no element of its own: splice its children in.
            let mut kids = Vec::with_capacity(fragment.children.len());
            for child in &fragment.children {
                push_node(&mut kids, convert(child, depth + 1, sf)?);
            }
            Node::El(El {
                tag: Tag::Span,
                classes: vec!["\u{0}fragment".to_owned()],
                style: Vec::new(),
                attrs: Vec::new(),
                kids,
            })
        }
        HtmlDomNode::MathML(_) => return None,
    })
}

/// A symbol (one character of a formula) as a node: plain text if it needs nothing, otherwise a
/// span with its font classes, its italic correction and its colour.
///
/// This is the one place where a run of symbols is written out; the steps after it (merging text
/// spans, unwrapping, folding spaces) only see text and spans.
fn symbol_node(symbol: &crate::dom_tree::SymbolNode, sf: Rounding) -> Node {
    let needs_span = symbol.italic > 0.0 || !symbol.classes.is_empty() || !symbol.style.is_empty();
    if !needs_span {
        return Node::Text(symbol.text.clone());
    }
    let mut style = Vec::new();
    if symbol.italic > 0.0 {
        style.push((
            "margin-right",
            sf.apply("margin-right", &make_em(symbol.italic)),
        ));
    }
    style.extend(
        symbol
            .style
            .iter()
            .map(|(p, v)| (p.as_str(), sf.apply(p.as_str(), v))),
    );
    Node::El(El {
        tag: Tag::Span,
        classes: symbol.classes.iter().map(str::to_owned).collect(),
        style,
        attrs: Vec::new(),
        kids: vec![Node::Text(symbol.text.clone())],
    })
}

/// How numbers are rounded: to `sf` significant figures (0 for not at all), for the properties
/// that do not set how tall a formula is unless `extents`.
#[derive(Clone, Copy)]
struct Rounding {
    sf: u32,
    extents: bool,
}

impl Rounding {
    fn apply(self, prop: &str, value: &str) -> String {
        if !self.extents && matches!(prop, "height" | "padding-top" | "vertical-align") {
            return value.to_owned();
        }
        round_sig(value, self.sf)
    }
}

/// An `em` value with at most `sf` significant figures (all of them if `sf` is 0). A value that
/// is not a plain number of ems is kept; a non-zero one stays non-zero.
fn round_sig(value: &str, sf: u32) -> String {
    if sf == 0 || cfg!(not(feature = "std-float")) {
        // (without `std-float` the precision is ignored: it needs core's `{:e}` formatter)
        return value.to_owned();
    }
    let Some(number) = value.strip_suffix("em") else {
        return value.to_owned();
    };
    let plain = !number.is_empty()
        && number
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_digit() || b == b'.' || (i == 0 && b == b'-'));
    let Ok(x) = number.parse::<f64>() else {
        return value.to_owned();
    };
    if !plain || x == 0.0 || !x.is_finite() {
        return value.to_owned();
    }
    let rounded = format!("{:.*e}", (sf - 1) as usize, x)
        .parse::<f64>()
        .unwrap_or(x);
    // Four decimals is as fine as `make_em` writes, so a rounded value is written in full.
    let text = format!("{rounded:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}em")
}

/// Appends `node` to `kids`, splicing in a fragment's children.
fn push_node(kids: &mut Vec<Node>, node: Node) {
    match node {
        Node::El(el) if el.classes.len() == 1 && el.classes[0] == "\u{0}fragment" => {
            kids.extend(el.kids);
        }
        node => kids.push(node),
    }
}

#[cfg(feature = "sizes")]
thread_local! {
    /// The sizes of the formula rendered last with [`Settings::compact_sizes`] on.
    static LAST_SIZES: core::cell::RefCell<Option<crate::sizes::FormulaSizes>> = const { core::cell::RefCell::new(None) };
}

/// The sizes of the formula that this thread rendered last with [`Settings::compact_sizes`] on (and no other since).
#[cfg(feature = "sizes")]
#[must_use]
pub fn take_sizes() -> Option<crate::sizes::FormulaSizes> {
    LAST_SIZES.with(|l| l.borrow_mut().take())
}

/// The `--sz` that [`convert`] gives a stack whose width is known and nothing took: removed.
#[cfg(feature = "sizes")]
fn strip_sizes(el: &mut El) {
    el.style.retain(|(p, _)| *p != "--sz");
    for kid in &mut el.kids {
        if let Node::El(child) = kid {
            strip_sizes(child);
        }
    }
}

/// `em` rounded up to four decimals: a browser takes a length down to a layout unit, so a width that is too small by a
/// hair makes a box that is a unit too narrow.
fn ceil_em(v: f64) -> String {
    let n = (v * 10_000.0 - 1e-7).ceil() / 10_000.0;
    let text = crate::numfmt::fixed4(n);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    let text = text
        .strip_prefix("0.")
        .map_or_else(|| text.to_owned(), |r| format!(".{r}"));
    format!(
        "{}em",
        if text.is_empty() || text == "-" {
            "0"
        } else {
            &text
        }
    )
}

/// Renders `root` (the tree `build_tree` made for [`OutputFormat::CompactHtml`]) as compact
/// markup, or `None` if the tree is not one this module handles.
pub(crate) fn render(root: &DomSpan, settings: &Settings) -> Option<Result<String, ParseError>> {
    let is_katex = root.classes.contains("katex") || root.classes.contains("katex-display");
    if !is_katex {
        return None;
    }
    // `FONT_RUN`: glyph runs in the generated font in place of what it can express (see `mathrun`).
    let hybrid = (settings.compact_levels & levels::FONT_RUN != 0
        && settings.compact_levels & levels::FLAT != 0
        && settings.compact_levels & levels::UNWRAP_HTML != 0)
        .then(|| mathrun::transform(root, settings))
        .flatten();
    let local;
    let (root, settings) = if let Some(h) = &hybrid {
        local = Settings {
            compact_wrap_min: h.wrap_min,
            ..settings.clone()
        };
        (&h.root, &local)
    } else {
        (root, settings)
    };
    let rounding = Rounding {
        sf: settings.compact_precision,
        extents: settings.compact_levels & levels::ROUND_EXTENTS != 0,
    };
    // `compact_sizes`: the widths of the stacks, computed from the tree (see `crate::sizes`).
    #[cfg_attr(not(feature = "sizes"), allow(unused_mut))]
    let mut tree = root.clone();
    #[cfg(feature = "sizes")]
    if settings.compact_sizes {
        let sizes = crate::sizes::annotate(&mut tree, hybrid.is_some());
        LAST_SIZES.with(|l| *l.borrow_mut() = Some(sizes));
    }
    let Node::El(mut el) = convert(&HtmlDomNode::DomSpan(tree), 0, rounding)? else {
        return None;
    };
    rewrite(&mut el, settings, rounding, hybrid.is_some());
    #[cfg(feature = "sizes")]
    if settings.compact_sizes {
        strip_sizes(&mut el);
    }
    let mut out = String::new();
    write_el(
        &mut out,
        &el,
        settings.compact_levels,
        settings.max_output_size,
    )
    .map(|()| out)
    .map_or_else(
        |()| {
            Some(Err(ParseErrorKind::OutputTooLarge {
                max: settings.max_output_size,
            }
            .into()))
        },
        |out| Some(Ok(out)),
    )
}

// ---------------------------------------------------------------------------------------------
// Rewrite steps

/// Class names that only set (inherited) font properties.
const FONT: &[&str] = &[
    "mathnormal",
    "mathit",
    "mathrm",
    "mathbf",
    "boldsymbol",
    "amsrm",
    "mathbb",
    "textbb",
    "mathcal",
    "mathfrak",
    "textfrak",
    "mathboldfrak",
    "textboldfrak",
    "mathtt",
    "mathscr",
    "textscr",
    "mathsf",
    "textsf",
    "mathboldsf",
    "textboldsf",
    "mathsfit",
    "mathitsf",
    "textitsf",
    "mainrm",
    "textbf",
    "textit",
    "textrm",
    "texttt",
];

fn is_size_class(c: &str) -> bool {
    c == "katex-sizing"
        || c.strip_prefix("reset-size")
            .or_else(|| c.strip_prefix("size"))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether the children of `el` sit in an ordinary inline formatting context in which no CSS rule
/// looks at their parent: `el` is a plain wrapper, a font or size span, a base or the formula.
fn plain_parent(el: &El) -> bool {
    el.tag == Tag::Span
        && el.classes.iter().all(|c| {
            DEAD.contains(&c.as_str())
                || FONT.contains(&c.as_str())
                || is_size_class(c)
                || matches!(c.as_str(), "katex" | "katex-base" | "msupsub")
        })
}

/// A span with nothing that sets any property: it only groups.
fn is_neutral(el: &El) -> bool {
    el.tag == Tag::Span
        && el.style.is_empty()
        && el.attrs.is_empty()
        && el.classes.iter().all(|c| DEAD.contains(&c.as_str()))
}

/// Whether `el` is an inline-level box a margin can be put on instead of next to it.
fn takes_margin(el: &El) -> bool {
    el.tag == Tag::Span
        && el.classes.iter().all(|c| {
            DEAD.contains(&c.as_str())
                || FONT.contains(&c.as_str())
                || matches!(c.as_str(), "msupsub" | "X" | "Y")
        })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Normal,
    /// A child of a `vlist` cell or of an `X`: a block that holds one stacked item.
    Wrapper,
    /// What a wrapper holds: a pstrut and one inline-block.
    Content,
    /// Table rows and cells of the old vertical list.
    Table,
}

fn rewrite(root: &mut El, settings: &Settings, rounding: Rounding, font: bool) {
    let levels = settings.compact_levels;
    if levels & levels::UNWRAP_HTML != 0 && !settings.fleqn {
        unwrap_html(root, settings.compact_label);
    }
    if levels & levels::DEAD_CLASSES != 0 {
        strip_dead(root);
    }
    let strut = if levels & levels::FLAT != 0
        && levels & levels::UNWRAP_HTML != 0
        && !settings.fleqn
    {
        flatten(root, settings.compact_wrap_min, settings.compact_line)
    } else {
        None
    };
    if levels & levels::SINGLE != 0 && levels & levels::VLIST != 0 {
        single_pass(root, false, levels, rounding);
    }
    if levels
        & (levels::UNWRAP
            | levels::VLIST
            | levels::PSEUDO
            | levels::GLUE
            | levels::MERGE
            | levels::TEXT)
        != 0
    {
        walk(root, Role::Normal, levels);
    }
    if levels & levels::INLINE_GLUE != 0 {
        inline_glue(root);
    }
    if font && levels & levels::PSEUDO != 0 && let Some(katex) = katex_mut(root) {
        char_breaks(katex);
    }
    if let Some(strut) = strut
        && let Some(katex) = katex_mut(root)
    {
        let Strut {
            height,
            align,
            needed,
        } = strut;
        if font {
            katex.classes.push("mf".to_owned());
        }
        let table_class = if needed && levels & levels::PSEUDO != 0 {
            pseudo_class(FSTRUTS, "t", &height, &align)
        } else {
            None
        };
        if let Some(class) = table_class {
            katex.classes.push(class);
        } else if !needed {
            // The line of text already has the room: `w` is only `white-space: nowrap`.
            katex.classes.push("w".to_owned());
        } else if levels & levels::PSEUDO != 0 {
            // The strut is the formula's `::before`, sized by two custom properties.
            katex.classes.push("f".to_owned());
            katex.style.push(("--h", height));
            if !align.is_empty() {
                katex.style.push(("--v", align));
            }
        } else {
            katex.classes.push("f".to_owned());
            let mut style = vec![("height", height)];
            if !align.is_empty() {
                style.push(("vertical-align", align));
            }
            katex
                .kids
                .insert(0, Node::El(span(&["katex-strut"], style, Vec::new())));
        }
        if katex.has("mw") && !katex.has("w") {
            // The strut of a formula that wraps at zero-width spaces is an element after its first segment (see `mathrun::stylesheet`).
            if let Some(at) = katex
                .kids
                .iter()
                .position(|k| matches!(k, Node::Text(t) if t == "\u{200b}"))
            {
                katex.kids.insert(at, Node::El(span(&["mu"], Vec::new(), Vec::new())));
            }
        }
    }
}

fn strip_dead(el: &mut El) {
    el.classes
        .retain(|c| !DEAD.contains(&c.as_str()) || c == "katex-error");
    for kid in &mut el.kids {
        if let Node::El(child) = kid {
            strip_dead(child);
        }
    }
}

fn walk(el: &mut El, role: Role, levels: u32) {
    let kid_role = if el.has("X") || el.has("vlist") {
        Role::Wrapper
    } else if role == Role::Wrapper {
        Role::Content
    } else if el.has("vlist-t") || el.has("vlist-r") {
        Role::Table
    } else {
        Role::Normal
    };
    if levels & levels::VLIST != 0 && kid_role == Role::Normal {
        for kid in &mut el.kids {
            if let Node::El(child) = kid
                && child.has("vlist-t")
            {
                to_inline_block(child);
            }
        }
    }
    for kid in &mut el.kids {
        if let Node::El(child) = kid {
            walk(child, kid_role, levels);
        }
    }
    let plain = kid_role == Role::Normal && plain_parent(el);
    if kid_role == Role::Wrapper && levels & levels::PSEUDO != 0 && el.has("X") {
        for kid in &mut el.kids {
            if let Node::El(wrapper) = kid {
                pstrut_to_pseudo(wrapper);
            }
        }
    }
    if levels & levels::PSEUDO != 0 && el.has("katex-base") {
        strut_to_pseudo(el);
    }
    if levels & levels::MERGE_ITEM != 0 && el.has("X") {
        merge_item(el);
    }
    if levels & levels::MERGE_CONTENT != 0 && (role == Role::Wrapper || el.has("Y")) {
        for kid in &mut el.kids {
            if let Node::El(content) = kid {
                merge_content(content);
            }
        }
    }
    if plain && levels & levels::UNWRAP != 0 {
        let kids = mem::take(&mut el.kids);
        for kid in kids {
            match kid {
                Node::El(child)
                    if is_neutral(&child)
                        && (levels & levels::UNWRAP_TEXT != 0
                            || child.kids.is_empty()
                            || child.kids.iter().any(|k| matches!(k, Node::El(_)))) =>
                {
                    el.kids.extend(child.kids);
                }
                other => el.kids.push(other),
            }
        }
    }
    if plain && levels & levels::MERGE != 0 {
        for kid in &mut el.kids {
            if let Node::El(child) = kid
                && child.has("msupsub")
                && child.style.is_empty()
                && child.kids.len() == 1
                && matches!(&child.kids[0], Node::El(c) if c.has("X") || c.has("Y"))
            {
                let Some(Node::El(mut inner)) = child.kids.pop() else {
                    continue;
                };
                inner.classes.push("msupsub".to_owned());
                child.classes.clear();
                *kid = Node::El(inner);
            }
        }
    }
    if plain && levels & levels::GLUE != 0 {
        fold_glue(&mut el.kids, levels & levels::FLAT != 0);
    }
    if plain && levels & levels::TEXT != 0 {
        merge_text(&mut el.kids);
    }
    merge_adjacent_text(&mut el.kids);
}

/// Ten-thousandths of an em of a number KaTeX wrote (`-0.25em`, `1.5em`, `0`), or `None`.
pub(crate) fn em4(value: &str) -> Option<i64> {
    let n = value.strip_suffix("em").unwrap_or(value);
    let (neg, n) = n.strip_prefix('-').map_or((false, n), |r| (true, r));
    let (int, frac) = n.split_once('.').unwrap_or((n, ""));
    if int.len() > 6
        || frac.len() > 4
        || (int.is_empty() && frac.is_empty())
        || !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let int: i64 = if int.is_empty() { 0 } else { int.parse().ok()? };
    let frac: i64 = if frac.is_empty() {
        0
    } else {
        format!("{frac:0<4}").parse().ok()?
    };
    let x = int * 10_000 + frac;
    Some(if neg { -x } else { x })
}

pub(crate) fn em_str(x: i64) -> String {
    make_em(x as f64 / 10_000.0)
}

/// `v` ten-thousandths of an em of the parent as ems of an element whose font is `ratio` times
/// the parent's, to six decimals: a rounded value changes the layout unit a margin or offset
/// truncates to for one in a hundred sizes of an em of a page of 16 px.
fn em_scaled(v: i64, ratio: f64) -> String {
    let x = v as f64 / 1e4 / ratio;
    let text = crate::numfmt::fixed(x, 6);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" {
        "0".to_owned()
    } else {
        format!("{text}em")
    }
}

fn span(classes: &[&str], style: Vec<(&'static str, String)>, kids: Vec<Node>) -> El {
    El {
        tag: Tag::Span,
        classes: classes.iter().map(|c| (*c).to_owned()).collect(),
        style,
        attrs: Vec::new(),
        kids,
    }
}

/// The part of a base's strut above and below the baseline, in ten-thousandths of an em.
fn strut_extent(base: &El) -> Option<(i64, i64)> {
    let Some(Node::El(strut)) = base.kids.first() else {
        return None;
    };
    if !(strut.tag == Tag::Span
        && strut.classes == ["katex-strut"]
        && strut.kids.is_empty()
        && strut.attrs.is_empty())
    {
        return None;
    }
    let (mut height, mut align) = (None, 0);
    for (p, v) in &strut.style {
        match *p {
            "height" => height = Some(em4(v)?),
            "vertical-align" => align = em4(v)?,
            _ => return None,
        }
    }
    let height = height?;
    Some((height + align, -align))
}

/// The bases of a formula (`katex-base` inline blocks, each with its strut) become one run of
/// inline content.
///
/// A base is an inline block only so that it is one unit for line breaking, and its strut gives
/// the line the height the base's content has. As the line box takes the largest extent above and
/// below the baseline of what it holds, one strut with the largest extents of all the bases gives
/// the line the same height, and what was a base is a run of inline content in the one that
/// holds the formula. Where KaTeX allows a line break, between two bases, a `<wbr>` does (the
/// formula has `white-space: nowrap`, which does not stop it in the browsers tried): not in
/// display style, where the formula is not wrapped. A formula that has anything else than bases
/// (a newline, a tag) is left as it is.
fn flatten(root: &mut El, wrap_min: usize, line: Option<(f64, f64)>) -> Option<Strut> {
    let display = root.has("katex-display");
    let katex = katex_mut(root)?;
    let (mut up, mut down) = (i64::MIN, i64::MIN);
    for kid in &katex.kids {
        let Node::El(base) = kid else { return None };
        if !(base.tag == Tag::Span
            && base.classes == ["katex-base"]
            && base.attrs.is_empty()
            && base.style.is_empty())
        {
            return None;
        }
        let (a, d) = strut_extent(base)?;
        up = up.max(a);
        down = down.max(d);
    }
    if katex.kids.is_empty() {
        return None;
    }
    let height = em_str(up + down);
    let align = if down == 0 {
        String::new()
    } else {
        em_str(-down)
    };
    let fits = line.is_some_and(|(a, d)| {
        up as f64 <= (a * 1e4).floor() && down as f64 <= (d * 1e4).floor()
    });
    let wrap = !display && text_len(&katex.kids) >= wrap_min;
    let mut kids = Vec::new();
    for (i, kid) in mem::take(&mut katex.kids).into_iter().enumerate() {
        let Node::El(mut base) = kid else { continue };
        base.kids.remove(0);
        if i > 0 && wrap && !base.kids.is_empty() {
            kids.push(Node::El(wbr()));
        }
        kids.append(&mut base.kids);
    }
    katex.kids = kids;
    Some(Strut {
        height,
        align,
        needed: !fits,
    })
}

/// A formula of text and inline spans, whose line breaks are `<wbr>` between its bases, has them as U+200B in its text instead.
///
/// With `white-space: normal` on the formula (the class `mw`) the zero-width space is the only place where it wraps, if nothing
/// else can break: in a run of private-use code points nothing does, spaces of stock text are no-break spaces here, the
/// spans are `white-space: nowrap` by the stylesheet (`mathrun::stylesheet`), and there is no inline block: a browser allows a
/// break before and after one whatever else is there (a word joiner does not stop it before one), so a formula that has a
/// stack, an SVG, a table ... keeps its `<wbr>` and `white-space: nowrap`. So does a formula with stock text that has an opening
/// bracket, a hyphen or a question mark, or a character that is not ASCII (Chrome 156 breaks before a bracket that follows
/// punctuation, after a hyphen, a question mark, a dash or an ellipsis: tried for every pair of ASCII punctuation marks, and
/// for none of the others).
fn char_breaks(katex: &mut El) -> bool {
    let is_wbr = |k: &Node| matches!(k, Node::El(e) if e.tag == Tag::Wbr);
    if !katex.kids.iter().any(is_wbr) || !katex.kids.iter().all(|k| is_wbr(k) || breakless(k)) {
        return false;
    }
    for kid in &mut katex.kids {
        if is_wbr(kid) {
            *kid = Node::Text("\u{200b}".to_owned());
        } else if let Node::Text(text) = kid {
            *text = text.replace(' ', "\u{a0}");
        }
    }
    katex.classes.push("mw".to_owned());
    true
}

/// Text that has no break opportunity in it and spans that make none.
fn breakless(node: &Node) -> bool {
    match node {
        Node::Text(text) => text.chars().all(|c| {
            c.is_alphanumeric()
                || c == '\u{a0}'
                || ('\u{e000}'..='\u{f8ff}').contains(&c)
                || (c.is_ascii_graphic() && !matches!(c, '(' | '[' | '{' | '-' | '?'))
                || c == ' '
        }),
        Node::El(el) => {
            el.tag == Tag::Span
                && el.attrs.is_empty()
                && el.classes.iter().all(|c| {
                    DEAD.contains(&c.as_str()) || FONT.contains(&c.as_str()) || is_size_class(c) || c == "mspace"
                })
                && el.style.iter().all(|(p, v)| {
                    matches!(*p, "margin-left" | "margin-right" | "vertical-align" | "line-height" | "color" | "top")
                        || (*p == "position" && v == "relative")
                })
                && el.kids.iter().all(breakless)
        }
    }
}

/// A formula's strut: the height of its `::before`, and whether it needs one.
struct Strut {
    height: String,
    align: String,
    needed: bool,
}

/// The number of characters of the text in `kids`.
fn text_len(kids: &[Node]) -> usize {
    kids.iter()
        .map(|k| match k {
            Node::Text(t) => t.chars().count(),
            Node::El(e) => text_len(&e.kids),
        })
        .sum()
}

/// KaTeX's size multipliers: `size1` to `size11`.
const SIZE_MULT: [f64; 11] = [
    0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.2, 1.44, 1.728, 2.074, 2.488,
];

/// The font size of a `katex-sizing reset-sizeA sizeB` span, relative to its parent's.
fn font_ratio(classes: &[String]) -> Option<f64> {
    let (mut reset, mut size) = (None, None);
    for c in classes {
        if let Some(n) = c.strip_prefix("reset-size") {
            reset = n.parse::<usize>().ok().filter(|n| (1..=11).contains(n));
        } else if let Some(n) = c.strip_prefix("size") {
            size = n.parse::<usize>().ok().filter(|n| (1..=11).contains(n));
        }
    }
    Some(SIZE_MULT[size? - 1] / SIZE_MULT[reset? - 1])
}

/// Content that is only text and spans that style it, with nothing that is an inline block, so
/// that its extents are at most those of its text.
fn is_plain_inline(node: &Node) -> bool {
    match node {
        Node::Text(_) => true,
        Node::El(el) => {
            el.tag == Tag::Span
                && el.attrs.is_empty()
                && el.classes.iter().all(|c| {
                    DEAD.contains(&c.as_str())
                        || FONT.contains(&c.as_str())
                        || is_size_class(c)
                        || c == "mspace"
                })
                && el.style.iter().all(|(p, _)| {
                    matches!(
                        *p,
                        "margin-left"
                            | "margin-right"
                            | "vertical-align"
                            | "line-height"
                            | "color"
                    )
                })
                && el.kids.iter().all(is_plain_inline)
        }
    }
}

/// Vertical lists of one item become inline elements (see [`single_item`]).
fn single_pass(el: &mut El, in_wrapper: bool, levels: u32, rounding: Rounding) {
    let wrappers = el.has("vlist");
    for kid in &mut el.kids {
        if let Node::El(child) = kid {
            single_pass(child, wrappers, levels, rounding);
        }
    }
    if in_wrapper {
        return;
    }
    if el.classes == ["msupsub"]
        && el.style.is_empty()
        && el.attrs.is_empty()
        && let [Node::El(table)] = el.kids.as_mut_slice()
        && table.has("vlist-t")
    {
        to_inline_block(table);
        if let Some(item) = single_item(table, levels, rounding) {
            *el = item;
            return;
        }
    }
    for kid in &mut el.kids {
        if let Node::El(table) = kid
            && table.has("vlist-t")
        {
            to_inline_block(table);
            if let Some(item) = single_item(table, levels, rounding) {
                *table = item;
            }
        }
    }
}

/// A vertical list of one item (`x_i`, `x^2`, a raised box), as an inline element.
///
/// The list's one item is a block at the baseline, with a pstrut that puts the line's baseline
/// `P` below the block's top, and the block is shifted by `top`: the item's baseline is `P + top`
/// below the formula's. The same is an inline element shifted by `position: relative; top: P +
/// top`, which moves what it holds and nothing around it, and the room that the list took above
/// and below the baseline (its height and depth) is in the formula's strut. The offsets are in
/// the em of the item, so they are divided by the size the item's size span sets. Only text and
/// spans around text are put inline: an inline block inside would take part in the line that
/// holds the formula.
fn single_item(x: &mut El, levels: u32, _rounding: Rounding) -> Option<El> {
    if !x.has("X") {
        return None;
    }
    let [Node::El(wrapper)] = x.kids.as_slice() else {
        return None;
    };
    if wrapper.tag != Tag::Span || !wrapper.classes.is_empty() || !wrapper.attrs.is_empty() {
        return None;
    }
    let (mut top, mut left, mut right) = (0, 0, 0);
    for (p, v) in &wrapper.style {
        let n = em4(v)?;
        match *p {
            "top" => top = n,
            "margin-left" => left = n,
            "margin-right" => right = n,
            _ => return None,
        }
    }
    let [Node::El(pstrut), content @ ..] = wrapper.kids.as_slice() else {
        return None;
    };
    if !(pstrut.has("pstrut")
        && pstrut.classes.len() == 1
        && pstrut.kids.is_empty()
        && pstrut.attrs.is_empty()
        && pstrut.style.len() == 1)
    {
        return None;
    }
    let down = em4(pstrut.get_style("height")?)? + top;
    if content.is_empty() || !content.iter().all(is_plain_inline) {
        return None;
    }
    let Some(Node::El(wrapper)) = x.kids.pop() else {
        return None;
    };
    let mut content = wrapper.kids;
    content.remove(0);
    // The element that takes the offsets: the size span, or the one span around the text.
    let sized = matches!(content.as_slice(), [Node::El(c)]
        if c.has("katex-sizing") && c.attrs.is_empty() && c.style.is_empty());
    let single = matches!(content.as_slice(), [Node::El(c)]
        if c.tag == Tag::Span && c.attrs.is_empty() && c.style.is_empty());
    let (mut host, ratio) = if sized || single {
        let Some(Node::El(mut c)) = content.pop() else {
            return None;
        };
        let ratio = if sized { font_ratio(&c.classes)? } else { 1.0 };
        // An italic correction that the text has is a margin of its own, in the same em: not to
        // be added to the list's (a sum is not laid out as two margins are).
        let inner_margin = matches!(c.kids.as_slice(), [Node::El(inner)]
            if inner.get_style("margin-right").is_some());
        if sized && levels & levels::MERGE_CONTENT != 0 && !(inner_margin && right != 0) {
            merge_content(&mut c);
        }
        (c, ratio)
    } else {
        (span(&[], Vec::new(), content), 1.0)
    };
    let scale = |v: i64| {
        if ratio == 1.0 {
            em_str(v)
        } else {
            em_scaled(v, ratio)
        }
    };
    let mut style = Vec::new();
    if down != 0 {
        style.push(("vertical-align", scale(-down)));
        style.push(("line-height", "0".to_owned()));
    }
    if left != 0 {
        style.push(("margin-left", scale(left)));
    }
    if right != 0 {
        style.push(("margin-right", scale(right)));
    }
    style.append(&mut host.style);
    host.style = style;
    Some(host)
}

/// Spacing spans that are empty with a margin are ordinary inline elements with the margin: an
/// empty inline box with a margin takes the margin's room as an empty inline block does, without
/// being a box of its own for layout.
fn inline_glue(el: &mut El) {
    if glue_of_el(el) {
        el.classes.clear();
    }
    for kid in &mut el.kids {
        if let Node::El(child) = kid {
            inline_glue(child);
        }
    }
}

fn glue_of_el(el: &El) -> bool {
    el.has("mspace")
        && el.classes.len() == 1
        && el.kids.is_empty()
        && el.attrs.is_empty()
        && el.style.len() == 1
        && el.style[0].0 == "margin-right"
}

/// A vertical list of one item: the `X` and the wrapper of its item become one element `Y`. The
/// wrapper is a zero-height block at the start of the `X`'s content, shifted by `top`, whose
/// margins set the item's place in the line. As the `X` itself, shifted by `top` and with those
/// margins, the item is drawn at the same place and takes the same room; the `X` gets an
/// explicit `height` since its content is no longer a zero-height block.
fn merge_item(x: &mut El) {
    let [Node::El(wrapper)] = x.kids.as_mut_slice() else {
        return;
    };
    if !(wrapper.tag == Tag::Span
        && wrapper.attrs.is_empty()
        && wrapper.classes.len() == 2
        && wrapper.classes[0] == "W"
        && wrapper.classes[1].starts_with('k')
        && wrapper
            .style
            .iter()
            .all(|(p, _)| matches!(*p, "top" | "margin-left" | "margin-right"))
        && x.attrs.is_empty()
        && x.style
            .iter()
            .all(|(p, _)| matches!(*p, "padding-top" | "height" | "vertical-align" | "width")))
    {
        return;
    }
    let mut wrapper = mem::replace(
        wrapper,
        El {
            tag: Tag::Span,
            classes: Vec::new(),
            style: Vec::new(),
            attrs: Vec::new(),
            kids: Vec::new(),
        },
    );
    if x.get_style("height").is_none() {
        x.style.push(("height", "0".to_owned()));
    }
    x.classes.retain(|c| c != "X");
    x.classes.insert(0, "Y".to_owned());
    // The wrapper's `W` is `Y` itself: `Y::before` is the pstrut.
    x.classes.extend(wrapper.classes.drain(1..));
    // The item's margins were inside the stack's width; on the merged element they are outside it.
    let margins: Option<f64> = wrapper
        .style
        .iter()
        .filter(|(p, _)| matches!(*p, "margin-left" | "margin-right"))
        .map(|(_, v)| v.strip_suffix("em").and_then(crate::numfmt::parse_f64))
        .sum::<Option<f64>>();
    if let Some(i) = x.style.iter().position(|(p, _)| *p == "width") {
        match (
            margins,
            x.style[i]
                .1
                .strip_suffix("em")
                .and_then(crate::numfmt::parse_f64),
        ) {
            (Some(m), Some(w)) => x.style[i].1 = ceil_em(w - m),
            _ => {
                x.style.remove(i);
            }
        }
    }
    x.style.append(&mut wrapper.style);
    x.kids = wrapper.kids;
}

/// A size span (an inline block in a vertical list) that holds one span of text, `<F63><N>i</N>`,
/// is that span with the size: the span's font only changes the glyphs, and the size span is
/// positioned by its baseline.
fn merge_content(content: &mut El) {
    if !(content.tag == Tag::Span
        && content.style.is_empty()
        && content.attrs.is_empty()
        && !content.classes.is_empty()
        && content
            .classes
            .iter()
            .all(|c| is_size_class(c) || DEAD.contains(&c.as_str())))
    {
        return;
    }
    let [Node::El(inner)] = content.kids.as_mut_slice() else {
        return;
    };
    if !(inner.tag == Tag::Span
        && text_only(inner)
        && inner.attrs.is_empty()
        && inner
            .classes
            .iter()
            .all(|c| FONT.contains(&c.as_str()) || DEAD.contains(&c.as_str()))
        && inner.style.iter().all(|(p, _)| *p == "margin-right"))
    {
        return;
    }
    let inner = mem::replace(
        inner,
        El {
            tag: Tag::Span,
            classes: Vec::new(),
            style: Vec::new(),
            attrs: Vec::new(),
            kids: Vec::new(),
        },
    );
    content.classes.extend(inner.classes);
    content.style = inner.style;
    content.kids = inner.kids;
}

fn merge_adjacent_text(kids: &mut Vec<Node>) {
    let mut out: Vec<Node> = Vec::with_capacity(kids.len());
    for kid in mem::take(kids) {
        match (out.last_mut(), kid) {
            (Some(Node::Text(a)), Node::Text(b)) => a.push_str(&b),
            (_, kid) => out.push(kid),
        }
    }
    *kids = out;
}

/// An empty `mspace` with only a `margin-right` is horizontal space: an empty inline-block of
/// that width. The same space as a margin on the box before it (or after it) is the same layout,
/// as long as that box is inline-level and has no margin of that side yet.
fn glue_of(node: &Node) -> Option<&str> {
    let Node::El(el) = node else { return None };
    (el.has("mspace")
        && el.classes.len() == 1
        && el.kids.is_empty()
        && el.attrs.is_empty()
        && el.style.len() == 1
        && el.style[0].0 == "margin-right")
        .then(|| el.style[0].1.as_str())
}

fn fold_glue(kids: &mut Vec<Node>, flat: bool) {
    let mut i = 0;
    while i < kids.len() {
        let Some(value) = glue_of(&kids[i]).map(str::to_owned) else {
            i += 1;
            continue;
        };
        if flat {
            if fold_glue_flat(kids, i, &value) {
                continue;
            }
            i += 1;
            continue;
        }
        let prev_ok = i > 0
            && matches!(&kids[i - 1], Node::El(p) if takes_margin(p) && p.get_style("margin-right").is_none());
        let next_ok = i + 1 < kids.len()
            && matches!(&kids[i + 1], Node::El(n) if takes_margin(n) && n.get_style("margin-left").is_none() && glue_of(&kids[i + 1]).is_none());
        if prev_ok {
            if let Node::El(p) = &mut kids[i - 1] {
                p.style.push(("margin-right", value));
            }
            kids.remove(i);
        } else if next_ok {
            if let Node::El(n) = &mut kids[i + 1] {
                n.style.insert(0, ("margin-left", value));
            }
            kids.remove(i);
        } else {
            i += 1;
        }
    }
}

/// The glue at `kids[i]` of a formula without bases: a margin of the span before it or after it
/// (a size span as well, with the margin in its em; margins already there are added to), of the
/// one after the line break opportunity that follows it (the break is moved into that span, after
/// the margin, so that the glue stays at the end of the line before the break), or, between two
/// glues that have text between them (an operator with its spaces), the margins of a span around
/// that text. Returns whether the glue went (and `kids[i]` is the next node).
fn fold_glue_flat(kids: &mut Vec<Node>, i: usize, value: &str) -> bool {
    if i > 0
        && let Node::El(p) = &mut kids[i - 1]
        && let Some(ratio) = margin_host(p, true)
        && add_margin(p, "margin-right", value, ratio)
    {
        kids.remove(i);
        return true;
    }
    if i + 1 < kids.len()
        && glue_of(&kids[i + 1]).is_none()
        && let Node::El(n) = &mut kids[i + 1]
        && let Some(ratio) = margin_host(n, true)
        && add_margin(n, "margin-left", value, ratio)
    {
        kids.remove(i);
        return true;
    }
    if let Some(host) = after_break(kids, i)
        && let Node::El(n) = &mut kids[host]
        && let Some(ratio) = margin_host(n, false)
        && add_margin(n, "margin-left", value, ratio)
    {
        n.kids.insert(0, Node::El(wbr()));
        kids.remove(i + 1);
        kids.remove(i);
        return true;
    }
    if let (Some(Node::Text(_)), Some(after)) = (kids.get(i + 1), kids.get(i + 2))
        && let Some(right) = glue_of(after).map(str::to_owned)
    {
        let text = kids.remove(i + 1);
        kids.remove(i + 1);
        kids[i] = Node::El(span(
            &[],
            vec![
                ("margin-left", value.to_owned()),
                ("margin-right", right),
            ],
            vec![text],
        ));
        return true;
    }
    false
}

/// How the em of `el`'s own margins relates to its parent's (1 for a span without a size), if
/// margins can be put on `el`. `atomic`: also an inline block.
fn margin_host(el: &El, atomic: bool) -> Option<f64> {
    if el.tag != Tag::Span || !el.attrs.is_empty() || glue_of_el(el) {
        return None;
    }
    let plain = el
        .classes
        .iter()
        .all(|c| DEAD.contains(&c.as_str()) || FONT.contains(&c.as_str()));
    if plain {
        return Some(1.0);
    }
    if atomic && takes_margin(el) {
        return Some(1.0);
    }
    if el.has("katex-sizing")
        && el
            .classes
            .iter()
            .all(|c| DEAD.contains(&c.as_str()) || FONT.contains(&c.as_str()) || is_size_class(c))
    {
        return font_ratio(&el.classes);
    }
    None
}

/// Puts `value` (in the parent's em) as `el`'s margin on one side, if it has none there. A sum
/// of margins is not laid out as the margins one by one: each is a number of layout units.
fn add_margin(el: &mut El, side: &'static str, value: &str, ratio: f64) -> bool {
    if el.get_style(side).is_some() {
        return false;
    }
    let Some(v) = em4(value) else { return false };
    let v = if ratio == 1.0 {
        value.to_owned()
    } else {
        em_scaled(v, ratio)
    };
    el.style.push((side, v));
    true
}

fn wbr() -> El {
    El {
        tag: Tag::Wbr,
        classes: Vec::new(),
        style: Vec::new(),
        attrs: Vec::new(),
        kids: Vec::new(),
    }
}

/// The element after the line break opportunity at `kids[i + 1]`, if it is a plain span (not an
/// inline block, in which a break would not be one of the line's) that can take a margin.
fn after_break(kids: &[Node], i: usize) -> Option<usize> {
    let (Some(Node::El(brk)), Some(Node::El(_))) = (kids.get(i + 1), kids.get(i + 2)) else {
        return None;
    };
    (brk.tag == Tag::Wbr).then_some(i + 2)
}

/// Adjacent spans with the same classes and style that hold only text are one run of text. The
/// KaTeX fonts have no kerning or ligatures, so a glyph is placed the same either way. A
/// `margin-right` may be on the last one only.
fn merge_text(kids: &mut Vec<Node>) {
    let mut out: Vec<Node> = Vec::with_capacity(kids.len());
    for kid in mem::take(kids) {
        let mergeable = match (out.last(), &kid) {
            (Some(Node::El(a)), Node::El(b)) => {
                text_only(a)
                    && text_only(b)
                    && a.tag == Tag::Span
                    && b.tag == Tag::Span
                    && a.classes == b.classes
                    && a.attrs.is_empty()
                    && b.attrs.is_empty()
                    && a.get_style("margin-right").is_none()
                    && a.get_style("margin-left").is_none()
                    && b.get_style("margin-left").is_none()
                    && a.style
                        .iter()
                        .filter(|(p, _)| *p != "margin-right")
                        .eq(b.style.iter().filter(|(p, _)| *p != "margin-right"))
            }
            _ => false,
        };
        match (mergeable, out.last_mut(), kid) {
            (true, Some(Node::El(a)), Node::El(b)) => {
                a.kids.extend(b.kids);
                if let Some(m) = b.style.into_iter().find(|(p, _)| *p == "margin-right") {
                    a.style.push(m);
                }
            }
            (_, _, kid) => out.push(kid),
        }
    }
    *kids = out;
}

fn text_only(el: &El) -> bool {
    !el.kids.is_empty() && el.kids.iter().all(|k| matches!(k, Node::Text(_)))
}

/// The table of a vertical list as one inline block.
///
/// In `katex.css` a `vlist-t` is an inline table whose first row holds the stacked items in a
/// cell of the full height above the baseline, bottom-aligned, so that the items' zero-height
/// blocks start at the baseline; a second row of the depth's height hangs below it. An inline
/// block with `padding-top` for the first row and `height` for the second starts the blocks at
/// the same place and is as tall; `contain: layout` puts its baseline at the bottom edge, as the
/// table's is, and `vertical-align` lowers it by the depth. The table's `vlist-s` cell and the `-2px` margin that compensates for it go.
fn to_inline_block(table: &mut El) {
    let rows: Vec<&El> = table
        .kids
        .iter()
        .filter_map(|k| if let Node::El(e) = k { Some(e) } else { None })
        .collect();
    if rows.len() != table.kids.len()
        || !(1..=2).contains(&rows.len())
        || !table.style.iter().all(|(p, _)| *p == "--sz")
        || !table.attrs.is_empty()
        || !table
            .classes
            .iter()
            .all(|c| matches!(c.as_str(), "vlist-t" | "vlist-t2"))
    {
        return;
    }
    let cell = |row: &El, nth: usize| -> Option<El> {
        match row.kids.get(nth) {
            Some(Node::El(e)) if e.has("vlist") && e.classes.len() == 1 => Some(e.clone()),
            _ => None,
        }
    };
    let first = rows[0];
    let two = rows.len() == 2;
    let (Some(top), true) = (
        cell(first, 0),
        first.has("vlist-r")
            && (if two {
                first.kids.len() == 2
                    && matches!(&first.kids[1], Node::El(s) if s.has("vlist-s"))
                    && rows[1].has("vlist-r")
                    && rows[1].kids.len() == 1
            } else {
                first.kids.len() == 1
            }),
    ) else {
        return;
    };
    let depth = if two { cell(rows[1], 0) } else { None };
    if two && depth.is_none() {
        return;
    }
    let (Some(h), true) = (
        top.get_style("height").map(str::to_owned),
        top.style.len() == 1 && table.has("vlist-t2") == two,
    ) else {
        return;
    };
    let d = match &depth {
        Some(cell) => match (cell.get_style("height"), cell.style.len()) {
            (Some(d), 1) => Some(d.to_owned()),
            _ => return,
        },
        None => None,
    };
    let mut style = vec![("padding-top", h)];
    if let Some(d) = d {
        // Lowered by its own height, so that the first row's bottom is on the baseline.
        let lowered = d
            .strip_prefix('-')
            .map_or_else(|| format!("-{d}"), str::to_owned);
        style.push(("height", d));
        style.push(("vertical-align", lowered));
    }
    if let Some((_, w)) = table.style.iter().find(|(p, _)| *p == "--sz") {
        style.push(("width", w.clone()));
    }
    table.classes = vec!["X".to_owned()];
    table.style = style;
    table.kids = top.kids;
}

/// A pstrut is an inline-block as tall as `height` and no wide, first in the wrapper: its
/// `::before` is the same box.
fn pstrut_to_pseudo(wrapper: &mut El) {
    let Some(Node::El(first)) = wrapper.kids.first() else {
        return;
    };
    if !(first.has("pstrut")
        && first.classes.len() == 1
        && first.kids.is_empty()
        && first.attrs.is_empty()
        && first.style.len() == 1)
    {
        return;
    }
    let Some(height) = first.get_style("height") else {
        return;
    };
    let Some(class) = pseudo_class(PSTRUTS, "k", height, "") else {
        return;
    };
    wrapper.classes.push("W".to_owned());
    wrapper.classes.push(class);
    wrapper.kids.remove(0);
}

/// A strut is an inline-block of the formula's height, first in its base: its `::before`.
fn strut_to_pseudo(base: &mut El) {
    let Some(Node::El(first)) = base.kids.first() else {
        return;
    };
    if !(first.has("katex-strut")
        && first.classes.len() == 1
        && first.kids.is_empty()
        && first.attrs.is_empty())
    {
        return;
    }
    let mut height = None;
    let mut align = "";
    for (p, v) in &first.style {
        match *p {
            "height" => height = Some(v.as_str()),
            "vertical-align" => align = v.as_str(),
            _ => return,
        }
    }
    let Some(height) = height else { return };
    let Some(class) = pseudo_class(STRUTS, "u", height, align) else {
        return;
    };
    base.classes.push(class);
    base.kids.remove(0);
}

/// The class of a `::before` whose height (and vertical alignment) the stylesheet has.
pub(crate) fn pseudo_class(table: &[(&str, &str)], prefix: &str, height: &str, align: &str) -> Option<String> {
    let height = trim_number(height);
    let align = trim_number(align);
    let i = table
        .iter()
        .position(|(h, a)| *h == height && *a == align)?;
    Some(format!("{prefix}{}", base36(i)))
}

pub(crate) fn base36(mut n: usize) -> String {
    let mut digits = Vec::new();
    loop {
        digits.push(char::from_digit((n % 36) as u32, 36).unwrap_or('0'));
        n /= 36;
        if n == 0 {
            break;
        }
    }
    digits.iter().rev().collect()
}

/// The `katex` element: the root, or the root's only child in display mode.
fn katex_mut(root: &mut El) -> Option<&mut El> {
    if root.has("katex") {
        return Some(root);
    }
    root.kids.iter_mut().find_map(|kid| match kid {
        Node::El(el) if el.has("katex") => Some(el),
        _ => None,
    })
}

/// `katex-html` is a plain wrapper: in inline mode it is an unstyled inline span, and in display
/// mode `.katex > .katex-html` is a block that `.katex` already is (both have `position:
/// relative`), so its children can sit directly in `.katex`. The exception is `fleqn`, whose
/// padding on `.katex` moves the left edge that `.katex-tag` is positioned from. Its
/// `aria-hidden` moves up when the formula has no label to be read instead.
fn unwrap_html(root: &mut El, labelled: bool) {
    let Some(katex) = katex_mut(root) else { return };
    let mut kids = Vec::with_capacity(katex.kids.len());
    let mut hidden = false;
    for kid in mem::take(&mut katex.kids) {
        match kid {
            Node::El(wrapper) if wrapper.classes == ["katex-html"] && wrapper.style.is_empty() => {
                hidden |= wrapper.attrs.iter().any(|(k, _)| k == "aria-hidden");
                kids.extend(wrapper.kids);
            }
            other => kids.push(other),
        }
    }
    katex.kids = kids;
    if hidden && !labelled {
        katex
            .attrs
            .push(("aria-hidden".to_owned(), "true".to_owned()));
    }
}

// ---------------------------------------------------------------------------------------------
// Serialization

/// Class names that no rule in `katex.css` mentions.
const DEAD: &[&str] = &[
    "mord",
    "mbin",
    "mrel",
    "mopen",
    "mclose",
    "mpunct",
    "minner",
    "mop",
    "mtight",
    "text",
    "allowbreak",
    "nobreak",
];

/// Short names for KaTeX's frequent classes; `katex.css` is aliased to them by the generator.
// SHORT-BEGIN
const SHORT: &[(&str, &str)] = &[
    ("katex-base", "B"),
    ("katex-strut", "U"),
    ("mspace", "S"),
    ("vlist-t", "T"),
    ("vlist-t2", "T2"),
    ("vlist-r", "R"),
    ("vlist", "V"),
    ("vlist-s", "Z"),
    ("pstrut", "P"),
    ("msupsub", "Q"),
    ("mathnormal", "N"),
    ("mathrm", "M"),
];
// SHORT-END

fn short(class: &str) -> Option<&'static str> {
    SHORT
        .iter()
        .find(|(long, _)| *long == class)
        .map(|(_, s)| *s)
}

/// `katex-sizing reset-sizeA sizeB` as one class, `F` followed by the two sizes in base 12.
fn sizing_class(classes: &[String]) -> Option<String> {
    let mut reset = None;
    let mut size = None;
    let mut sizing = false;
    for c in classes {
        if c == "katex-sizing" {
            sizing = true;
        } else if let Some(n) = c.strip_prefix("reset-size") {
            reset = n.parse::<u32>().ok().filter(|n| (1..=11).contains(n));
        } else if let Some(n) = c.strip_prefix("size") {
            size = n.parse::<u32>().ok().filter(|n| (1..=11).contains(n));
        }
    }
    let (true, Some(r), Some(s)) = (sizing, reset, size) else {
        return None;
    };
    let digit = |n: u32| char::from_digit(n, 12).unwrap_or('0');
    Some(format!("F{}{}", digit(r), digit(s)))
}

fn write_el(out: &mut String, el: &El, levels: u32, limit: usize) -> Result<(), ()> {
    out.push('<');
    out.push_str(el.tag.name());
    let mut classes: Vec<String> = Vec::new();
    let sizing = if levels & levels::SHORT_NAMES != 0 {
        sizing_class(&el.classes)
    } else {
        None
    };
    if let Some(c) = &sizing {
        classes.push(c.clone());
    }
    for c in &el.classes {
        if sizing.is_some() && is_size_class(c) {
            continue;
        }
        if levels & levels::DEAD_CLASSES != 0 && DEAD.contains(&c.as_str()) {
            continue;
        }
        if levels & levels::SHORT_NAMES != 0
            && let Some(s) = short(c)
        {
            classes.push(s.to_owned());
            continue;
        }
        classes.push(c.clone());
    }
    let mut decls: Vec<(&str, String)> = el
        .style
        .iter()
        .map(|(p, v)| {
            (
                *p,
                if levels & levels::NUM_TRIM != 0 {
                    trim_number(v)
                } else {
                    v.clone()
                },
            )
        })
        .collect();
    if levels & levels::COMBO != 0 {
        if let Some(class) = shift_class(&mut decls) {
            classes.push(class);
        }
        for (group, table, prefix) in [(GEOM_PROPS, GEOMS, 'g'), (ITEM_PROPS, ITEMS, 'i')] {
            if let Some(class) = combo_class(&mut decls, group, table, prefix) {
                classes.push(class);
            }
        }
    }
    let mut inline: Vec<(&str, String)> = Vec::new();
    for (prop, value) in decls {
        if levels & levels::VALUE_CLASSES != 0
            && let Some(c) = value_class(prop, &value)
        {
            classes.push(c);
            continue;
        }
        inline.push((prop, value));
    }
    if !classes.is_empty() {
        out.push_str(" class=\"");
        for (i, c) in classes.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            let _ = escape_into(out, c);
        }
        out.push('"');
    }
    for (k, v) in &el.attrs {
        let _ = write!(out, " {k}=\"");
        let _ = escape_into(out, v);
        out.push('"');
    }
    if !inline.is_empty() {
        out.push_str(" style=\"");
        for (i, (p, v)) in inline.iter().enumerate() {
            if i > 0 && levels & levels::NUM_TRIM != 0 {
                out.push(';');
            }
            out.push_str(p);
            out.push(':');
            let _ = escape_into(out, v);
            if levels & levels::NUM_TRIM == 0 {
                out.push(';');
            }
        }
        out.push('"');
    }
    if out.len() > limit {
        return Err(());
    }
    if el.tag == Tag::Wbr {
        out.push('>');
        return Ok(());
    }
    if el.tag.is_void() {
        out.push_str("/>");
        return Ok(());
    }
    out.push('>');
    for kid in &el.kids {
        match kid {
            Node::Text(t) => {
                let _ = escape_into(out, t);
            }
            Node::El(child) => write_el(out, child, levels, limit)?,
        }
    }
    out.push_str("</");
    out.push_str(el.tag.name());
    out.push('>');
    if out.len() > limit {
        return Err(());
    }
    Ok(())
}

/// `0.5em` as `.5em`, `-0.25em` as `-.25em`, `0em` as `0`.
pub(crate) fn trim_number(value: &str) -> String {
    if let Some(n) = value.strip_suffix("em") {
        let (sign, digits) = n.strip_prefix('-').map_or(("", n), |d| ("-", d));
        let plain = digits.bytes().all(|b| b.is_ascii_digit() || b == b'.')
            && !digits.is_empty()
            && digits.bytes().filter(|b| *b == b'.').count() <= 1;
        if plain {
            if let Some(frac) = digits.strip_prefix("0.") {
                return format!("{sign}.{frac}em");
            }
            if digits == "0" {
                return "0".to_owned();
            }
        }
    }
    value.to_owned()
}

/// Inline declarations that have a class of their own, by property. Generated from the corpus by
/// `tools/gen_compact_values.py`.
// VALUES-BEGIN
const VALUES: &[(&str, &[&str])] = &[
    (
        "height",
        &[
            ".15em", "0", ".2861em", ".143em", ".2083em", ".2819em", ".3473em", ".686em",
            ".3552em", ".011em", ".522em", ".2441em", "1.25em", "1.5689em", ".2501em", ".59em",
            ".2645em", ".5344em", "1.2777em", "1.2844em", "1.68em", "1.8941em", "5.6em", ".2025em",
            ".2918em", ".8804em", ".8889em", "1.3861em", "1.4138em", "1.516em", "1.7941em",
        ],
    ),
    (
        "vertical-align",
        &[
            "-.15em",
            "-.2861em",
            "-.2083em",
            "-.143em",
            "-.2819em",
            "-.3473em",
            "-.686em",
            "-.3552em",
            "-.011em",
            "-.2441em",
            "-.2501em",
            "-1.2777em",
            "-1.25em",
            "-.59em",
            "-.25em",
            "-5.6em",
            "-1.2844em",
            "-.5344em",
            "-1.4138em",
            "-1.3861em",
            "-.8804em",
            "-.2918em",
            "-.2645em",
            "-.1944em",
            "-.0391em",
            "-4.55em",
            "-1.4382em",
            "-.7693em",
            "-.69em",
            "-.4233em",
            "-.2527em",
            "-.0833em",
            "-1.516em",
            "-1.1em",
            "-.2025em",
            "-.136em",
        ],
    ),
    (
        "top",
        &[
            "-2.55em",
            "-3.063em",
            "-2.357em",
            "-3.113em",
            "0",
            "-3.05em",
            "-3.677em",
            "-2.314em",
            "-3.23em",
            "-2.5198em",
            "-3.322em",
            "-2.689em",
            "-2.4559em",
            "-1.9em",
            "-1.8723em",
            "-3em",
            "-2.989em",
            "-3.76em",
            "-3.2041em",
            "-2.26em",
            "-8.26em",
            "-6.76em",
            "-5.26em",
            "-3.9789em",
            "-3.5689em",
            "-3.0344em",
            "-2.931em",
            "-2.4355em",
            "-2.2844em",
            "-.76em",
            ".74em",
            "2.24em",
            "-3.7941em",
            "-3.09em",
            "-2.786em",
            "-2.453em",
            "-1.8479em",
            "-1.809em",
        ],
    ),
    (
        "margin-left",
        &[
            "0", ".1667em", "-.0359em", ".2222em", "-.0077em", "-.1389em", "-.0576em", "-.0278em",
            "-.0785em", ".2778em", "2em", "-.044em", "-.0715em", "-.0269em", "-.109em", "-.1076em",
            "-.0315em", ".0556em", "-.2222em", "-.0502em", "-.0037em",
        ],
    ),
    (
        "margin-right",
        &[
            ".2778em", ".05em", ".2222em", ".1667em", ".0359em", ".0278em", ".1389em", ".0576em",
            ".0714em", ".0077em", ".0715em", ".0315em", ".0785em", ".044em", ".0269em", ".109em",
            ".0572em", ".0139em", ".0556em", "1em", ".0502em", ".1076em", "-.1667em", "2em",
            ".3333em", ".0813em", ".0037em", ".0197em",
        ],
    ),
    ("min-width", &["1.469em"]),
    ("width", &[".5em"]),
    ("border-bottom-width", &[".04em"]),
    (
        "padding-top",
        &[
            ".3011em", ".1514em", ".3117em", ".8141em", ".3283em", ".3173em", ".3361em", "1.05em",
            ".8641em", ".3448em", ".8913em", ".2806em", "1.3214em", ".1645em", ".3281em", ".207em",
            "1.1003em", ".7401em", ".7519em", "1.2041em", ".8019em", ".8413em", ".8991em",
            ".3175em", ".8491em", ".938em", "1.0344em", "1.1081em", "1.3603em", "1.7844em",
            "6.1em", ".6828em", ".7144em", ".9191em", "1.0064em", "1.09em",
        ],
    ),
];
// VALUES-END

/// Struts (height, vertical-align) and pstruts' heights that have a pseudo-element rule of their
/// own. Generated from the corpus by `tools/gen_compact_values.py`; the class is the prefix and
/// the position in base 36.
// STRUTS-BEGIN
const STRUTS: &[(&str, &str)] = &[
    ("1em", "-.25em"),
    (".6444em", ""),
    (".8141em", ""),
    (".8389em", "-.1944em"),
    (".6833em", ""),
    (".625em", "-.1944em"),
    (".6944em", ""),
    (".5806em", "-.15em"),
    (".7278em", "-.0833em"),
    (".4306em", ""),
    (".8889em", "-.1944em"),
    (".8333em", "-.15em"),
    (".7333em", "-.15em"),
    (".8444em", "-.15em"),
    (".8778em", "-.1944em"),
    ("1.0361em", "-.2861em"),
    (".7778em", "-.1944em"),
    (".6667em", "-.0833em"),
    (".7719em", "-.136em"),
    (".7167em", "-.2861em"),
    (".6389em", "-.1944em"),
    (".7917em", "-.2083em"),
    (".9501em", "-.136em"),
    ("2.0074em", "-.686em"),
    (".9694em", "-.2861em"),
    (".4445em", ""),
    (".6111em", ""),
    (".6389em", "-.2083em"),
    ("1.2503em", "-.15em"),
    (".8641em", ""),
    (".8974em", "-.0833em"),
    (".854em", "-.1944em"),
    (".7804em", "-.136em"),
    (".8694em", "-.2861em"),
    (".8991em", ""),
    ("1.1413em", "-.25em"),
    (".8193em", "-.136em"),
    ("1.0858em", "-.1944em"),
    ("1.2581em", "-.15em"),
    (".7667em", "-.0833em"),
    (".8491em", ""),
    (".9385em", "-.3552em"),
    ("2.3em", "-1.25em"),
    ("2.566em", "-1.516em"),
    (".8019em", ""),
    ("2.4em", "-.95em"),
    (".938em", ""),
    (".6595em", ""),
    (".9028em", "-.2083em"),
    ("1.1113em", "-.011em"),
    (".8095em", "-.1944em"),
    ("2.0463em", "-.686em"),
    (".7335em", "-.1944em"),
    (".8413em", ""),
    ("1.2709em", "-.2645em"),
    (".5945em", "-.15em"),
    ("1.7941em", "-.59em"),
    ("1.0413em", "-.15em"),
    (".7858em", "-.3552em"),
    (".786em", "-.15em"),
];
// STRUTS-END
// PSTRUTS-BEGIN
const PSTRUTS: &[(&str, &str)] = &[
    ("2.7em", ""),
    ("3em", ""),
    ("2.5em", ""),
    ("3.05em", ""),
    ("3.5689em", ""),
    ("3.0344em", ""),
    ("3.7941em", ""),
    ("3.008em", ""),
    ("3.68em", ""),
    ("3.8941em", ""),
];
// PSTRUTS-END

/// The struts of a formula without bases (height, vertical-align): the class is `t` and the
/// position in base 36. Generated from the corpus by `tools/gen_compact_flat.py`.
// FSTRUTS-BEGIN
pub(crate) const FSTRUTS: &[(&str, &str)] = &[
    ("1em", "-.25em"),
    (".8141em", ""),
    (".8889em", "-.1944em"),
    (".625em", "-.1944em"),
    (".6833em", ""),
    (".8389em", "-.1944em"),
    (".8888em", "-.1944em"),
    (".6444em", ""),
    (".7278em", "-.0833em"),
    (".6944em", ""),
    (".8333em", "-.15em"),
    (".9501em", "-.136em"),
    (".5806em", "-.15em"),
    ("1.0361em", "-.2861em"),
    (".8444em", "-.15em"),
    (".8778em", "-.1944em"),
    (".8974em", "-.0833em"),
    (".8777em", "-.1944em"),
    (".8193em", "-.136em"),
    ("2.0074em", "-.686em"),
    ("1.1413em", "-.25em"),
    (".8491em", ""),
    ("2.566em", "-1.516em"),
    ("1.1052em", "-.3552em"),
    ("2.4em", "-.95em"),
    (".6595em", ""),
    (".9028em", "-.2083em"),
    (".4306em", ""),
    (".7944em", "-.15em"),
    (".7777em", "-.0833em"),
    (".8413em", ""),
    ("1.2709em", "-.2645em"),
    ("1.0858em", "-.1944em"),
    ("1.7941em", "-.59em"),
    (".7777em", "-.1944em"),
    (".854em", "-.1944em"),
    (".7778em", "-.1944em"),
    ("1.0641em", "-.25em"),
    (".7167em", "-.2861em"),
    (".7667em", "-.0833em"),
];
// FSTRUTS-END

/// The offsets of an item that is shifted in place (vertical-align, margin-left, margin-right; the
/// item has `line-height: 0`): the class is `z` and the position in base 36.
// SHIFTS-BEGIN
const SHIFTS: &[[&str; 3]] = &[
    ["-.214286em", "", ".071429em"],
    ["-.214286em", "-.051286em", ".071429em"],
    [".518571em", "", ".071429em"],
    [".59em", "", ".071429em"],
    ["-.214286em", "-.011em", ".071429em"],
    ["-.214286em", "-.198429em", ".071429em"],
    ["-.214286em", "-.082286em", ".071429em"],
    ["-.214286em", "-.112143em", ".071429em"],
    ["-.2002em", "-.03892em", ".09996em"],
    ["-.214286em", "-.062857em", ".071429em"],
    ["-.2002em", "", ".09996em"],
    ["-.257429em", "", ".071429em"],
    ["-.348714em", "", ".071429em"],
    ["-.2002em", "-.05026em", ".09996em"],
    ["-.214286em", "-.102143em", ".071429em"],
    ["-.214286em", "-.038429em", ".071429em"],
    [".412857em", "", ".071429em"],
    ["-.214286em", "-.039714em", ".071429em"],
    ["-.214286em", "-.155714em", ".071429em"],
    ["-.214286em", "-.153714em", ".071429em"],
    [".6034em", "", ".09996em"],
    [".4004em", "", ".09996em"],
    ["-.214286em", "-.005286em", ".071429em"],
    ["-.214286em", "-.045em", ".071429em"],
    ["-.214286em", "-.317429em", ".071429em"],
];
// SHIFTS-END

/// The properties of a vertical list's own geometry, and of its item's place.
const GEOM_PROPS: &[&str] = &["padding-top", "height", "vertical-align"];
const ITEM_PROPS: &[&str] = &["top", "margin-left", "margin-right"];
const SHIFT_PROPS: &[&str] = &["vertical-align", "margin-left", "margin-right"];

// GEOMS-BEGIN
const GEOMS: &[[&str; 3]] = &[
    [".3011em", ".15em", "-.15em"],
    [".8141em", "0", ""],
    [".3117em", ".15em", "-.15em"],
    [".3283em", ".15em", "-.15em"],
    [".1514em", ".15em", "-.15em"],
    [".1514em", ".2861em", "-.2861em"],
    [".1514em", ".3473em", "-.3473em"],
    [".3173em", ".2819em", "-.2819em"],
    [".3361em", ".15em", "-.15em"],
    [".3448em", ".3552em", "-.3552em"],
    [".8913em", "0", ""],
    [".3117em", ".2083em", "-.2083em"],
    [".2806em", ".15em", "-.15em"],
    ["1.3214em", ".686em", "-.686em"],
    [".8641em", "0", ""],
    ["", ".522em", ""],
    [".1645em", ".143em", "-.143em"],
    [".3361em", ".2861em", "-.2861em"],
    [".3011em", ".2083em", "-.2083em"],
    [".3281em", ".143em", "-.143em"],
    [".3117em", ".2861em", "-.2861em"],
    [".207em", ".2441em", "-.2441em"],
    ["1.1003em", ".011em", "-.011em"],
    [".3173em", ".143em", "-.143em"],
    ["1.05em", "1.25em", "-1.25em"],
    [".7401em", "0", ""],
    [".3011em", ".2861em", "-.2861em"],
    [".8019em", "0", ""],
    [".8991em", "0", ""],
    [".1514em", ".2501em", "-.2501em"],
    [".938em", "0", ""],
    [".7519em", "0", ""],
    [".3175em", ".15em", "-.15em"],
    [".8413em", "0", ""],
    ["1.1081em", ".011em", "-.011em"],
    ["1.0344em", ".5344em", "-.5344em"],
    ["", "1.5689em", ""],
    ["1.7844em", "1.2844em", "-1.2844em"],
    [".3283em", ".2083em", "-.2083em"],
    [".8491em", "0", ""],
    ["6.1em", "5.6em", "-5.6em"],
    ["1.05em", "1.3861em", "-1.3861em"],
    ["1.05em", "1.2777em", "-1.2777em"],
    ["1.05em", "1.516em", "-1.516em"],
    [".6828em", "0", ""],
    ["", ".8889em", "-.1944em"],
    ["1.05em", "1.4138em", "-1.4138em"],
    ["1.3603em", ".686em", "-.686em"],
    ["1.0064em", ".2645em", "-.2645em"],
    [".9191em", "0", ""],
    ["1.2041em", ".59em", "-.59em"],
    ["", "1.7941em", ""],
    [".7144em", "0", ""],
    [".1514em", ".2918em", "-.2918em"],
    [".3173em", ".2025em", "-.2025em"],
];
// GEOMS-END
// ITEMS-BEGIN
const ITEMS: &[[&str; 3]] = &[
    ["", "", ".2778em"],
    ["", "", ".2222em"],
    ["-2.55em", "0", ".05em"],
    ["", "", ".1667em"],
    ["", "", ".0359em"],
    ["", "", ".0278em"],
    ["-2.55em", "-.0359em", ".05em"],
    ["", "", ".1389em"],
    ["-3.063em", "", ".05em"],
    ["", ".1667em", ""],
    ["", "", ".0576em"],
    ["-3.113em", "", ".05em"],
    ["", "", ".0715em"],
    ["-2.55em", "-.0077em", ".05em"],
    ["", "", ".0315em"],
    ["0", "", ""],
    ["-2.55em", "-.1389em", ".05em"],
    ["-2.55em", "-.0576em", ".05em"],
    ["", "", ".0269em"],
    ["", ".2222em", ""],
    ["", "", ".044em"],
    ["", "", ".0077em"],
    ["-3.05em", "", ""],
    ["", "", ".109em"],
    ["-2.55em", "-.0785em", ".05em"],
    ["", "", ".0785em"],
    ["-2.357em", "-.0278em", ".0714em"],
    ["-2.314em", "", ""],
    ["-3.677em", "", ""],
    ["-2.55em", "-.044em", ".05em"],
    ["-3.23em", "", ""],
    ["", "", ".0572em"],
    ["", ".2778em", ""],
    ["", ".2222em", ".0077em"],
    ["", ".1667em", ".0278em"],
    ["", ".1667em", ".0359em"],
    ["-2.357em", "0", ".0714em"],
    ["", "", ".0139em"],
    ["-2.5198em", "0", ".05em"],
    ["-3.322em", "", ""],
    ["-2.689em", "", ""],
    ["", "", "1em"],
    ["-2.4559em", "", ".05em"],
    ["", "", ".0556em"],
    ["", ".2222em", ".0785em"],
    ["-2.357em", "-.0359em", ".0714em"],
    ["", "2em", ""],
    ["-2.55em", "-.0715em", ".05em"],
    ["", "", ".0502em"],
    ["-1.9em", "0", ""],
    ["", "", ".1076em"],
    ["-1.8723em", "0", ""],
    ["", ".2222em", ".0359em"],
    ["-2.55em", "", ".05em"],
    ["", ".2778em", ".0077em"],
    ["", ".1667em", ".0576em"],
    ["", "", "2em"],
    ["-2.55em", "-.0269em", ".05em"],
    ["", "2em", ".0359em"],
    ["-2.989em", "", ".05em"],
    ["-3em", "", ""],
    ["", ".1667em", ".109em"],
    ["", "", "-.1667em"],
    ["-3.2041em", "", ""],
    ["-2.55em", "-.0278em", ".05em"],
    ["", ".2222em", ".044em"],
    ["-3.76em", "", ""],
    ["-2.26em", "", ""],
    ["-2.55em", "-.109em", ".05em"],
    ["", "", ".3333em"],
    ["-2.55em", "-.1076em", ".05em"],
    ["", ".0556em", ".2222em"],
    ["-2.931em", "", ".0714em"],
    ["", ".1667em", ".1389em"],
    ["-2.4355em", "0", ".05em"],
    ["-3.5689em", "", ""],
    ["-3.0344em", "", ""],
    ["-3.9789em", "", ""],
    ["-2.2844em", "", ""],
    ["-8.26em", "", ""],
    ["-6.76em", "", ""],
    ["-5.26em", "", ""],
    ["-.76em", "", ""],
    [".74em", "", ""],
    ["2.24em", "", ""],
    ["", "", ".0813em"],
    ["", ".1667em", ".2778em"],
    ["", ".1667em", ".1076em"],
    ["-1.809em", "0", ""],
    ["-1.8479em", "0", ""],
    ["-2.786em", "", ".0714em"],
    ["", "", ".0037em"],
    ["-2.55em", "-.0037em", ".05em"],
    ["-3.09em", "", ""],
    ["-3.7941em", "", ""],
    ["", ".1667em", ".0715em"],
    ["-2.55em", "-.0315em", ".05em"],
    ["-2.55em", "-.2222em", ".05em"],
];
// ITEMS-END

/// If the declarations of `group` that `decls` holds are one of `table`'s rows (an empty string
/// for a property it lacks), takes them out and returns the row's class.
fn combo_class(
    decls: &mut Vec<(&str, String)>,
    group: &[&str],
    table: &[[&str; 3]],
    prefix: char,
) -> Option<String> {
    let mut row = [""; 3];
    let mut any = false;
    for (slot, prop) in row.iter_mut().zip(group) {
        if let Some((_, v)) = decls.iter().find(|(p, _)| p == prop) {
            *slot = v.as_str();
            any = true;
        }
    }
    if !any {
        return None;
    }
    let at = table.iter().position(|r| *r == row)?;
    decls.retain(|(p, _)| !group.contains(p));
    Some(format!("{prefix}{}", base36(at)))
}

/// If `decls` shift an item in place (`line-height: 0` and offsets) as a row of `SHIFTS` does,
/// takes them out and returns the row's class.
fn shift_class(decls: &mut Vec<(&str, String)>) -> Option<String> {
    if !decls.iter().any(|(p, v)| *p == "line-height" && v == "0") {
        return None;
    }
    let mut row = [""; 3];
    for (slot, prop) in row.iter_mut().zip(SHIFT_PROPS) {
        if let Some((_, v)) = decls.iter().find(|(p, _)| p == prop) {
            *slot = v.as_str();
        }
    }
    let at = SHIFTS.iter().position(|r| *r == row)?;
    decls.retain(|(p, _)| *p != "line-height" && !SHIFT_PROPS.contains(p));
    Some(format!("z{}", base36(at)))
}

/// The class standing for `prop: value`, if the stylesheet has one: the property's letter and
/// the value's position in the property's list in base 36 (the generator writes the same names).
fn value_class(prop: &str, value: &str) -> Option<String> {
    let value = trim_number(value);
    let (_, values) = VALUES.iter().find(|(p, _)| *p == prop)?;
    let at = values.iter().position(|v| *v == value)?;
    let letter = match prop {
        "height" => 'h',
        "vertical-align" => 'a',
        "top" => 'y',
        "margin-left" => 'l',
        "margin-right" => 'r',
        "min-width" => 'm',
        "width" => 'w',
        "border-bottom-width" => 'x',
        "padding-top" => 'q',
        _ => return None,
    };
    Some(format!("{letter}{}", base36(at)))
}

/// The stylesheet the compact output needs.
///
/// It is KaTeX 0.18.5's `katex.css` (its font files are loaded from `fonts/`, as there, but
/// only as WOFF2) with the rules for the compact output
/// added, so it also styles the KaTeX-compatible output. Generated by
/// `tools/gen_compact_css.py` from `katex.css` and the tables in this file.
#[must_use]
pub const fn stylesheet() -> &'static str {
    include_str!("../data/katex-compact.css")
}
