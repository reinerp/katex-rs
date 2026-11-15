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
    /// Every step.
    pub const ALL: u32 = (1 << 11) - 1;
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

fn rewrite(root: &mut El, settings: &Settings) {
    let levels = settings.compact_levels;
    if levels & levels::UNWRAP_HTML != 0 && !settings.fleqn {
        unwrap_html(root, settings.compact_label);
    }
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
            ".011em", ".143em", ".15em", ".1514em", ".1645em", ".2025em", ".207em", ".2083em",
            ".2441em", ".2501em", ".2645em", ".2806em", ".2819em", ".2861em", ".2918em",
            ".3011em", ".3117em", ".3173em", ".3175em", ".3281em", ".3283em", ".3361em",
            ".3448em", ".3473em", ".3552em", ".4306em", ".4445em", ".522em", ".5344em",
            ".5806em", ".59em", ".5945em", ".6111em", ".625em", ".6389em", ".6444em",
            ".6595em", ".6667em", ".6828em", ".6833em", ".686em", ".6887em", ".6944em",
            ".7144em", ".7167em", ".7278em", ".7333em", ".7335em", ".7401em", ".7519em",
            ".7667em", ".7719em", ".7778em", ".7804em", ".7858em", ".786em", ".7917em",
            ".8019em", ".8095em", ".8141em", ".8193em", ".8333em", ".8389em", ".8413em",
            ".8444em", ".8491em", ".854em", ".8641em", ".8694em", ".8778em", ".8804em",
            ".8889em", ".8913em", ".8974em", ".8991em", ".9028em", ".9191em", ".938em",
            ".9385em", ".9501em", ".9694em", "1em", "1.0064em", "1.0344em", "1.0361em",
            "1.0413em", "1.05em", "1.0858em", "1.09em", "1.1003em", "1.1081em", "1.1113em",
            "1.1413em", "1.2041em", "1.25em", "1.2503em", "1.2581em", "1.2709em", "1.2777em",
            "1.2844em", "1.3214em", "1.3603em", "1.3861em", "1.4138em", "1.516em", "1.5689em",
            "1.68em", "1.7844em", "1.7941em", "1.8941em", "2.0074em", "2.0463em", "2.3em",
            "2.4em", "2.5em", "2.566em", "2.7em", "3em", "3.008em", "3.0344em", "3.05em",
            "3.5689em", "3.68em", "3.7941em", "3.8941em", "5.6em", "6.1em",
        ],
    ),
    (
        "vertical-align",
        &[
            "-1.516em", "-1.2777em", "-1.25em", "-.95em", "-.686em", "-.59em", "-.3552em",
            "-.2861em", "-.2645em", "-.2501em", "-.25em", "-.2083em", "-.1944em", "-.15em",
            "-.136em", "-.0833em", "-.0391em", "-.011em",
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
            "-.0037em", "0",
        ],
    ),
    (
        "margin-right",
        &[
            "-.1667em", ".0037em", ".0077em", ".0139em", ".0197em", ".0269em", ".0278em",
            ".0315em", ".0359em", ".044em", ".05em", ".0502em", ".0556em", ".0572em",
            ".0576em", ".0714em", ".0715em", ".0785em", ".0813em", ".1076em", ".109em",
            ".1389em", ".1667em", ".1952em", ".2222em", ".2778em", ".3333em", "1em", "2em",
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
];
// VALUES-END

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
