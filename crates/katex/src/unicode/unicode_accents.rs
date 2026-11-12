use crate::static_map::{CharMap, Map};

/// Represents the LaTeX equivalents for a Unicode accent character
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccentMapping {
    /// LaTeX command for text mode
    pub text: &'static str,
    /// LaTeX command for math mode (optional)
    pub math: Option<&'static str>,
}

/// Mapping of Unicode accent characters to their LaTeX equivalents
pub const UNICODE_ACCENTS: Map<char, AccentMapping> = CharMap::new(&[
    // Combining grave accent
    (
        '\u{0300}',
        AccentMapping {
            text: "\\`",
            math: Some("\\grave"),
        },
    ),
    // Combining acute accent
    (
        '\u{0301}',
        AccentMapping {
            text: "\\'",
            math: Some("\\acute"),
        },
    ),
    // Combining circumflex accent
    (
        '\u{0302}',
        AccentMapping {
            text: "\\^",
            math: Some("\\hat"),
        },
    ),
    // Combining tilde
    (
        '\u{0303}',
        AccentMapping {
            text: "\\~",
            math: Some("\\tilde"),
        },
    ),
    // Combining macron (overline)
    (
        '\u{0304}',
        AccentMapping {
            text: "\\=",
            math: Some("\\bar"),
        },
    ),
    // Combining breve
    (
        '\u{0306}',
        AccentMapping {
            text: "\\u",
            math: Some("\\breve"),
        },
    ),
    // Combining dot above
    (
        '\u{0307}',
        AccentMapping {
            text: "\\.",
            math: Some("\\dot"),
        },
    ),
    // Combining diaeresis (umlaut)
    (
        '\u{0308}',
        AccentMapping {
            text: "\\\"",
            math: Some("\\ddot"),
        },
    ),
    // Combining ring above
    (
        '\u{030a}',
        AccentMapping {
            text: "\\r",
            math: Some("\\mathring"),
        },
    ),
    // Combining double acute accent
    (
        '\u{030b}',
        AccentMapping {
            text: "\\H",
            math: None,
        },
    ),
    // Combining caron (háček)
    (
        '\u{030c}',
        AccentMapping {
            text: "\\v",
            math: Some("\\check"),
        },
    ),
    // Combining cedilla
    (
        '\u{0327}',
        AccentMapping {
            text: "\\c",
            math: None,
        },
    ),
]);
