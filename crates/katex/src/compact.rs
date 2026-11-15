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

use crate::ParseError;
use crate::dom_tree::{DomSpan, HtmlDomNode, SvgChildNode};
use crate::types::{ParseErrorKind, Settings};
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
    /// Shorten numbers (`0.5em` to `.5em`).
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
    /// Also remove wrappers that hold only text, which then sits in its parent's text run. Glyphs
    /// are then placed by the sum of unrounded advances and not by sums of layout units that
    /// are rounded per element, so one in a few hundred glyphs lands a pixel away.
    pub const UNWRAP_TEXT: u32 = 1 << 11;
    /// Every step.
    pub const ALL: u32 = (1 << 12) - 1;
    /// Every step that keeps the pixels exactly as KaTeX's.
    pub const EXACT: u32 = ALL & !(UNWRAP_TEXT | TEXT);
}

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
fn convert(node: &HtmlDomNode, depth: usize) -> Option<Node> {
    if depth > MAX_DEPTH {
        return None;
    }
    Some(match node {
        HtmlDomNode::DomSpan(span) => {
            let mut kids = Vec::with_capacity(span.children.len());
            for child in &span.children {
                push_node(&mut kids, convert(child, depth + 1)?);
            }
            Node::El(El {
                tag: Tag::Span,
                classes: span.classes.iter().map(str::to_owned).collect(),
                style: span
                    .style
                    .iter()
                    .map(|(p, v)| (p.as_str(), v.to_owned()))
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
                push_node(&mut kids, convert(child, depth + 1)?);
            }
            Node::El(El {
                tag: Tag::A,
                classes: anchor.classes.iter().map(str::to_owned).collect(),
                style: anchor
                    .style
                    .iter()
                    .map(|(p, v)| (p.as_str(), v.to_owned()))
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
                .map(|(p, v)| (p.as_str(), v.to_owned()))
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
                style.push(("margin-right", crate::units::make_em(symbol.italic)));
            }
            style.extend(symbol.style.iter().map(|(p, v)| (p.as_str(), v.to_owned())));
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
                            crate::svg_geometry::PATH_MAP
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
            let mut attrs = vec![(
                "xmlns".to_owned(),
                "http://www.w3.org/2000/svg".to_owned(),
            )];
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
                push_node(&mut kids, convert(child, depth + 1)?);
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
    let Node::El(mut el) = convert(&HtmlDomNode::DomSpan(root.clone()), 0)? else {
        return None;
    };
    rewrite(&mut el, settings);
    let mut out = String::new();
    write_el(&mut out, &el, settings.compact_levels, settings.max_output_size)
        .map(|()| out)
        .map_or_else(
            |()| Some(Err(ParseErrorKind::OutputTooLarge { max: settings.max_output_size }.into())),
            |out| Some(Ok(out)),
        )
}

// ---------------------------------------------------------------------------------------------
// Rewrite steps

/// Class names that only set (inherited) font properties.
const FONT: &[&str] = &[
    "mathnormal", "mathit", "mathrm", "mathbf", "boldsymbol", "amsrm", "mathbb", "textbb", "mathcal",
    "mathfrak", "textfrak", "mathboldfrak", "textboldfrak", "mathtt", "mathscr", "textscr", "mathsf",
    "textsf", "mathboldsf", "textboldsf", "mathsfit", "mathitsf", "textitsf", "mainrm", "textbf",
    "textit", "textrm", "texttt",
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
                || matches!(c.as_str(), "msupsub" | "X")
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
    if levels & (levels::UNWRAP | levels::VLIST | levels::PSEUDO | levels::GLUE | levels::MERGE | levels::TEXT) != 0 {
        walk(root, Role::Normal, levels);
    }
}

fn strip_dead(el: &mut El) {
    el.classes.retain(|c| !DEAD.contains(&c.as_str()) || c == "katex-error");
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
            if let Node::El(child) = kid {
                if child.has("vlist-t") {
                    to_inline_block(child);
                }
            }
        }
    }
    for kid in &mut el.kids {
        if let Node::El(child) = kid {
            walk(child, kid_role, levels);
        }
    }
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
    let plain = kid_role == Role::Normal && plain_parent(el);
    if plain && levels & levels::UNWRAP != 0 {
        let kids = core::mem::take(&mut el.kids);
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
            if let Node::El(child) = kid {
                if child.has("msupsub")
                    && child.style.is_empty()
                    && child.kids.len() == 1
                    && matches!(&child.kids[0], Node::El(c) if c.has("X"))
                {
                    let Some(Node::El(mut inner)) = child.kids.pop() else { continue };
                    inner.classes.push("msupsub".to_owned());
                    child.classes.clear();
                    *kid = Node::El(inner);
                }
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

fn merge_adjacent_text(kids: &mut Vec<Node>) {
    let mut out: Vec<Node> = Vec::with_capacity(kids.len());
    for kid in core::mem::take(kids) {
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
    for kid in core::mem::take(kids) {
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
                    && a.style.iter().filter(|(p, _)| *p != "margin-right").eq(b
                        .style
                        .iter()
                        .filter(|(p, _)| *p != "margin-right"))
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
        || !table.classes.iter().all(|c| matches!(c.as_str(), "vlist-t" | "vlist-t2"))
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
        let lowered = d.strip_prefix('-').map_or_else(|| format!("-{d}"), str::to_owned);
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
    let Some(Node::El(first)) = wrapper.kids.first() else { return };
    if !(first.has("pstrut") && first.classes.len() == 1 && first.kids.is_empty() && first.attrs.is_empty() && first.style.len() == 1) {
        return;
    }
    let Some(height) = first.get_style("height") else { return };
    let Some(class) = pseudo_class(PSTRUTS, "k", height, "") else { return };
    wrapper.classes.push("W".to_owned());
    wrapper.classes.push(class);
    wrapper.kids.remove(0);
}

/// A strut is an inline-block of the formula's height, first in its base: its `::before`.
fn strut_to_pseudo(base: &mut El) {
    let Some(Node::El(first)) = base.kids.first() else { return };
    if !(first.has("katex-strut") && first.classes.len() == 1 && first.kids.is_empty() && first.attrs.is_empty()) {
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
    let Some(class) = pseudo_class(STRUTS, "u", height, align) else { return };
    base.classes.push(class);
    base.kids.remove(0);
}

/// The class of a `::before` whose height (and vertical alignment) the stylesheet has.
fn pseudo_class(table: &[(&str, &str)], prefix: &str, height: &str, align: &str) -> Option<String> {
    let height = trim_number(height);
    let align = trim_number(align);
    let i = table.iter().position(|(h, a)| *h == height && *a == align)?;
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
    for kid in core::mem::take(&mut katex.kids) {
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
        katex.attrs.push(("aria-hidden".to_owned(), "true".to_owned()));
    }
}

// ---------------------------------------------------------------------------------------------
// Serialization

/// Class names that no rule in `katex.css` mentions.
const DEAD: &[&str] = &[
    "mord", "mbin", "mrel", "mopen", "mclose", "mpunct", "minner", "mop", "mtight", "text",
    "allowbreak", "nobreak",
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
    SHORT.iter().find(|(long, _)| *long == class).map(|(_, s)| *s)
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
        } else if !DEAD.contains(&c.as_str()) {
            return None;
        }
    }
    let (true, Some(r), Some(s)) = (sizing, reset, size) else { return None };
    let digit = |n: u32| char::from_digit(n, 12).unwrap_or('0');
    Some(format!("F{}{}", digit(r), digit(s)))
}

fn write_el(out: &mut String, el: &El, levels: u32, limit: usize) -> Result<(), ()> {
    out.push('<');
    out.push_str(el.tag.name());
    let mut classes: Vec<String> = Vec::new();
    if levels & levels::SHORT_NAMES != 0 {
        if let Some(c) = sizing_class(&el.classes) {
            classes.push(c);
        }
    }
    if classes.is_empty() {
        for c in &el.classes {
            if levels & levels::DEAD_CLASSES != 0 && DEAD.contains(&c.as_str()) {
                continue;
            }
            if levels & levels::SHORT_NAMES != 0 {
                if let Some(s) = short(c) {
                    classes.push(s.to_owned());
                    continue;
                }
            }
            classes.push(c.clone());
        }
    }
    let mut inline: Vec<(&str, String)> = Vec::new();
    for (prop, value) in &el.style {
        let value = if levels & levels::NUM_TRIM != 0 { trim_number(value) } else { value.clone() };
        if levels & levels::VALUE_CLASSES != 0 {
            if let Some(c) = value_class(prop, &value) {
                classes.push(c);
                continue;
            }
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
            if i > 0 {
                out.push(';');
            }
            out.push_str(p);
            out.push(':');
            let _ = escape_into(out, v);
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
    out.push_str(el.tag.name());
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
            ".011em", ".143em", ".15em", ".2025em", ".2083em", ".2441em", ".2501em", ".2645em",
            ".2819em", ".2861em", ".2918em", ".3473em", ".3552em", ".4306em", ".4445em",
            ".522em", ".5344em", ".5806em", ".59em", ".5945em", ".6111em", ".625em", ".6389em",
            ".6444em", ".6595em", ".6667em", ".6833em", ".686em", ".6944em", ".7167em",
            ".7278em", ".7333em", ".7335em", ".7667em", ".7719em", ".7778em", ".7804em",
            ".7858em", ".786em", ".7917em", ".8019em", ".8095em", ".8141em", ".8193em",
            ".8333em", ".8389em", ".8413em", ".8444em", ".8491em", ".854em", ".8641em",
            ".8694em", ".8778em", ".8804em", ".8889em", ".8974em", ".8991em", ".9028em",
            ".938em", ".9385em", ".9501em", ".9694em", "1em", "1.0361em", "1.0413em",
            "1.0858em", "1.1113em", "1.1413em", "1.25em", "1.2503em", "1.2581em", "1.2709em",
            "1.2777em", "1.2844em", "1.3861em", "1.4138em", "1.516em", "1.5689em", "1.68em",
            "1.7941em", "1.8941em", "2.0074em", "2.0463em", "2.3em", "2.4em", "2.5em",
            "2.566em", "2.7em", "3em", "3.008em", "3.0344em", "3.05em", "3.5689em", "3.68em",
            "3.7941em", "3.8941em", "5.6em",
        ],
    ),
    (
        "vertical-align",
        &[
            "-5.6em", "-4.55em", "-1.516em", "-1.4382em", "-1.4138em", "-1.3861em",
            "-1.2844em", "-1.2777em", "-1.25em", "-1.1em", "-.95em", "-.8804em", "-.7693em",
            "-.69em", "-.686em", "-.59em", "-.5344em", "-.4233em", "-.3552em", "-.3473em",
            "-.2918em", "-.2861em", "-.2819em", "-.2645em", "-.2527em", "-.2501em", "-.25em",
            "-.2441em", "-.2083em", "-.2025em", "-.1944em", "-.15em", "-.143em", "-.136em",
            "-.0833em", "-.0391em", "-.011em",
        ],
    ),
    (
        "top",
        &[
            "-8.26em", "-6.76em", "-5.26em", "-3.9789em", "-3.7941em", "-3.76em", "-3.677em",
            "-3.5689em", "-3.322em", "-3.23em", "-3.2041em", "-3.113em", "-3.09em", "-3.063em",
            "-3.05em", "-3.0344em", "-3em", "-2.989em", "-2.931em", "-2.786em", "-2.689em",
            "-2.55em", "-2.5198em", "-2.4559em", "-2.453em", "-2.4355em", "-2.357em",
            "-2.314em", "-2.2844em", "-2.26em", "-1.9em", "-1.8723em", "-1.8479em", "-1.809em",
            "-.76em", "0", ".74em", "2.24em",
        ],
    ),
    (
        "margin-left",
        &[
            "-.2222em", "-.1389em", "-.109em", "-.1076em", "-.0785em", "-.0715em", "-.0576em",
            "-.0502em", "-.044em", "-.0359em", "-.0315em", "-.0278em", "-.0269em", "-.0077em",
            "-.0037em", "0", ".0556em", ".1667em", ".2222em", ".2778em", "2em",
        ],
    ),
    (
        "margin-right",
        &[
            "-.1667em", ".0037em", ".0077em", ".0139em", ".0197em", ".0269em", ".0278em",
            ".0315em", ".0359em", ".044em", ".05em", ".0502em", ".0556em", ".0572em",
            ".0576em", ".0714em", ".0715em", ".0785em", ".0813em", ".1076em", ".109em",
            ".1389em", ".1667em", ".2222em", ".2778em", ".3333em", "1em", "2em",
        ],
    ),
    (
        "min-width",
        &[
            "1.469em",
        ],
    ),
    (
        "width",
        &[
            ".5em",
        ],
    ),
    (
        "border-bottom-width",
        &[
            ".04em",
        ],
    ),
    (
        "padding-top",
        &[
            ".1514em", ".1645em", ".207em", ".2806em", ".3011em", ".3117em", ".3173em",
            ".3175em", ".3281em", ".3283em", ".3361em", ".3448em", ".6828em", ".7144em",
            ".7401em", ".7519em", ".8019em", ".8141em", ".8413em", ".8491em", ".8641em",
            ".8913em", ".8991em", ".9191em", ".938em", "1.0064em", "1.0344em", "1.05em",
            "1.09em", "1.1003em", "1.1081em", "1.2041em", "1.3214em", "1.3603em", "1.7844em",
            "6.1em",
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

/// Orders values as the generator writes them: by number, then by text.
fn cmp_values(a: &str, b: &str) -> core::cmp::Ordering {
    let num = |v: &str| v.strip_suffix("em").unwrap_or(v).parse::<f64>().unwrap_or(f64::NAN);
    num(a)
        .partial_cmp(&num(b))
        .unwrap_or(core::cmp::Ordering::Equal)
        .then_with(|| a.cmp(b))
}

/// The class standing for `prop: value`, if the stylesheet has one: the property's letter and
/// the number (`margin-right: .2778em` is `r_2778`, `top: -2.55em` is `yn2_55`). The generator
/// writes the same names.
fn value_class(prop: &str, value: &str) -> Option<String> {
    let value = trim_number(value);
    let (_, values) = VALUES.iter().find(|(p, _)| *p == prop)?;
    values.binary_search_by(|v| cmp_values(v, &value)).ok()?;
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
    let mut name = String::with_capacity(value.len() + 2);
    name.push(letter);
    let number = value.strip_suffix("em").unwrap_or(&value);
    let number = number.strip_prefix('-').map_or(number, |n| {
        name.push('n');
        n
    });
    name.extend(number.chars().map(|c| if c == '.' { '_' } else { c }));
    Some(name)
}

/// The stylesheet the compact output needs.
#[must_use]
pub fn stylesheet() -> String {
    String::new()
}
