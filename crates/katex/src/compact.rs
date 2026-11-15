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
    /// Every step.
    pub const ALL: u32 = (1 << 15) - 1;
    /// Write `span` as `q`, which is as short as an HTML tag that is neither a formatting
    /// element, which parsers handle slowly, nor a block gets. The stylesheet takes the quotation
    /// marks of `q` away. Not part of [`ALL`]: a page that styles `q`, or rich text copied out of
    /// a formula, would see it.
    pub const SHORT_TAGS: u32 = 1 << 16;
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
    pub const EXACT: u32 = ALL & !(UNWRAP_TEXT | TEXT | UNWRAP | GLUE | MERGE | MERGE_CONTENT);
}

/// The default [`Settings::compact_precision`].
pub const DEFAULT_PRECISION: u32 = 0;

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
        }
    }

    const fn is_void(self) -> bool {
        matches!(self, Self::Img | Self::Path | Self::Line)
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
            Node::El(El {
                tag: Tag::Span,
                classes: span.classes.iter().map(str::to_owned).collect(),
                style: span
                    .style
                    .iter()
                    .map(|(p, v)| (p.as_str(), sf.apply(p.as_str(), v)))
                    .collect(),
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
        HtmlDomNode::Symbol(symbol) => {
            let needs_span =
                symbol.italic > 0.0 || !symbol.classes.is_empty() || !symbol.style.is_empty();
            if !needs_span {
                return Some(Node::Text(symbol.text.clone()));
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
    if sf == 0 {
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

/// Renders `root` (the tree `build_tree` made for [`OutputFormat::CompactHtml`]) as compact
/// markup, or `None` if the tree is not one this module handles.
pub(crate) fn render(root: &DomSpan, settings: &Settings) -> Option<Result<String, ParseError>> {
    let is_katex = root.classes.contains("katex") || root.classes.contains("katex-display");
    if !is_katex {
        return None;
    }
    let Node::El(mut el) = convert(
        &HtmlDomNode::DomSpan(root.clone()),
        0,
        Rounding {
            sf: settings.compact_precision,
            extents: settings.compact_levels & levels::ROUND_EXTENTS != 0,
        },
    )?
    else {
        return None;
    };
    rewrite(&mut el, settings);
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

fn rewrite(root: &mut El, settings: &Settings) {
    let levels = settings.compact_levels;
    if levels & levels::UNWRAP_HTML != 0 && !settings.fleqn {
        unwrap_html(root, settings.compact_label);
    }
    if levels & levels::DEAD_CLASSES != 0 {
        strip_dead(root);
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
        fold_glue(&mut el.kids);
    }
    if plain && levels & levels::TEXT != 0 {
        merge_text(&mut el.kids);
    }
    merge_adjacent_text(&mut el.kids);
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
            .all(|(p, _)| matches!(*p, "padding-top" | "height" | "vertical-align")))
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

fn fold_glue(kids: &mut Vec<Node>) {
    let mut i = 0;
    while i < kids.len() {
        let Some(value) = glue_of(&kids[i]).map(str::to_owned) else {
            i += 1;
            continue;
        };
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
        || !table.style.is_empty()
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
fn pseudo_class(table: &[(&str, &str)], prefix: &str, height: &str, align: &str) -> Option<String> {
    let height = trim_number(height);
    let align = trim_number(align);
    let i = table
        .iter()
        .position(|(h, a)| *h == height && *a == align)?;
    Some(format!("{prefix}{}", base36(i)))
}

fn base36(mut n: usize) -> String {
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

fn tag_name(tag: Tag, levels: u32) -> &'static str {
    if tag == Tag::Span && levels & levels::SHORT_TAGS != 0 {
        "q"
    } else {
        tag.name()
    }
}

fn write_el(out: &mut String, el: &El, levels: u32, limit: usize) -> Result<(), ()> {
    out.push('<');
    out.push_str(tag_name(el.tag, levels));
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
    out.push_str(tag_name(el.tag, levels));
    out.push('>');
    if out.len() > limit {
        return Err(());
    }
    Ok(())
}

/// `0.5em` as `.5em`, `-0.25em` as `-.25em`, `0em` as `0`.
fn trim_number(value: &str) -> String {
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

/// The properties of a vertical list's own geometry, and of its item's place.
const GEOM_PROPS: &[&str] = &["padding-top", "height", "vertical-align"];
const ITEM_PROPS: &[&str] = &["top", "margin-left", "margin-right"];

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
