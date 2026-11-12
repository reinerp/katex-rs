//! Options module for KaTeX
//!
//! This module contains the Options struct which holds information about the
//! current parsing context including style, size, color, and font. Options
//! objects are immutable and provide methods for creating new Options with
//! different properties when recursing through the parsing process.

use crate::builder::{Unset, optional_setters};
use crate::style::TEXT;
use crate::types::{BUILD_LEVELS_PER_NESTING_LEVEL, ParseError, ParseErrorKind};
use crate::{
    font_metrics::{FONT_METRICS, FontMetrics},
    style::Style,
};
use alloc::borrow::Cow;
use core::cmp;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};
use strum::EnumString;

/// Bounds the recursion of the HTML and MathML builders while they build one
/// tree: at most [`BUILD_LEVELS_PER_NESTING_LEVEL`] times
/// ([`Settings::max_nesting`](crate::Settings::max_nesting) + 1) group
/// builders may be active at once.
///
/// A clone starts at the original's current depth, so options derived for a
/// child keep counting from their parent. The counter is atomic only so that
/// [`Options`] stays `Sync`; a render runs on one thread.
#[derive(Debug)]
pub struct BuildDepth {
    depth: AtomicUsize,
    max_nesting: usize,
}

impl BuildDepth {
    /// A counter at depth 0 for the given [`Settings::max_nesting`](crate::Settings::max_nesting).
    #[must_use]
    pub const fn new(max_nesting: usize) -> Self {
        Self {
            depth: AtomicUsize::new(0),
            max_nesting,
        }
    }

    /// Enters one builder level, failing if that exceeds the limit. The level
    /// is left when the returned guard is dropped.
    pub fn enter(&self) -> Result<BuildDepthGuard<'_>, ParseError> {
        let depth = self.depth.load(Ordering::Relaxed);
        // The top level counts as a level too.
        let limit = self
            .max_nesting
            .saturating_add(1)
            .saturating_mul(BUILD_LEVELS_PER_NESTING_LEVEL);
        if depth >= limit {
            return Err(ParseError::new(ParseErrorKind::TooDeeplyNested {
                max: self.max_nesting,
            }));
        }
        self.depth.store(depth + 1, Ordering::Relaxed);
        Ok(BuildDepthGuard(self))
    }
}

impl Default for BuildDepth {
    /// No limit.
    fn default() -> Self {
        Self::new(usize::MAX)
    }
}

impl Clone for BuildDepth {
    fn clone(&self) -> Self {
        Self {
            depth: AtomicUsize::new(self.depth.load(Ordering::Relaxed)),
            max_nesting: self.max_nesting,
        }
    }
}

impl PartialEq for BuildDepth {
    /// Depth bookkeeping is not part of the value of [`Options`].
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

/// Leaves a level entered with [`BuildDepth::enter`] when dropped.
#[derive(Debug)]
pub struct BuildDepthGuard<'a>(&'a BuildDepth);

impl Drop for BuildDepthGuard<'_> {
    fn drop(&mut self) {
        self.0.depth.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Font weight options
#[derive(Debug, Clone, PartialEq, Eq, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum FontWeight {
    /// Bold font weight
    TextBf, // "textbf"
    /// Medium font weight
    TextMd, // "textmd"
    /// Means no change
    #[strum(serialize = "")]
    Empty, // ""
}

impl FontWeight {
    /// Convert FontWeight to string representation
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::TextBf => "textbf",
            Self::TextMd => "textmd",
            Self::Empty => "",
        }
    }
}

/// Font shape options
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontShape {
    /// Italic font shape
    TextIt,
    /// Upright font shape
    TextUp,
    /// Means no change
    Empty, // ""
}

impl FontShape {
    /// Convert FontShape to string representation
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::TextIt => "textit",
            Self::TextUp => "textup",
            Self::Empty => "",
        }
    }
}

impl From<&str> for FontShape {
    fn from(s: &str) -> Self {
        match s {
            "textit" => Self::TextIt,
            "textup" => Self::TextUp,
            _ => Self::Empty,
        }
    }
}

/// Size style mapping table
/// Each element contains [textsize, scriptsize, scriptscriptsize].
/// The size mappings are taken from TeX with \normalsize=10pt.
const SIZE_STYLE_MAP: [[usize; 3]; 11] = [
    [1, 1, 1],   // size1: [5, 5, 5]              \tiny
    [2, 1, 1],   // size2: [6, 5, 5]
    [3, 1, 1],   // size3: [7, 5, 5]              \scriptsize
    [4, 2, 1],   // size4: [8, 6, 5]              \footnotesize
    [5, 2, 1],   // size5: [9, 6, 5]              \small
    [6, 3, 1],   // size6: [10, 7, 5]             \normalsize
    [7, 4, 2],   // size7: [12, 8, 6]             \large
    [8, 6, 3],   // size8: [14.4, 10, 7]          \Large
    [9, 7, 6],   // size9: [17.28, 12, 10]        \LARGE
    [10, 8, 7],  // size10: [20.74, 14.4, 12]     \huge
    [11, 10, 9], // size11: [24.88, 20.74, 17.28] \HUGE
];

/// Size multipliers corresponding to different sizes
const SIZE_MULTIPLIERS: [f64; 11] = [0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.2, 1.44, 1.728, 2.074, 2.488];

/// Calculate the size at a given style
const fn size_at_style(size: usize, style: &Style) -> usize {
    if style.size < 2 {
        size
    } else {
        SIZE_STYLE_MAP[size - 1][style.size - 1]
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Options for text rendering
pub struct Options {
    /// Current style
    pub style: &'static Style,
    /// Current color
    pub color: Option<String>,
    /// Current size
    pub size: usize,
    /// Text size
    pub text_size: usize,
    /// Phantom flag
    pub phantom: bool,
    /// Font (specific font like "Main-Bold")
    pub font: String,
    /// Font family (like "SansSerif")
    pub font_family: String,
    /// Font weight
    pub font_weight: FontWeight,
    /// Font shape
    pub font_shape: FontShape,
    /// Size multiplier
    pub size_multiplier: f64,
    /// Maximum size
    pub max_size: f64,
    /// Minimum rule thickness
    pub min_rule_thickness: f64,
    /// Recursion depth of the group builders
    pub build_depth: BuildDepth,
}

/// Builder for [`Options`], returned by [`Options::builder`]. `style`,
/// `max_size` and `min_rule_thickness` are required.
#[must_use]
pub struct OptionsBuilder<S = Unset, M = Unset, R = Unset> {
    style: S,
    max_size: M,
    min_rule_thickness: R,
    opt: OptionsOptional,
}

#[derive(Default)]
struct OptionsOptional {
    color: Option<String>,
    size: Option<usize>,
    text_size: Option<usize>,
    phantom: Option<bool>,
    font: Option<String>,
    font_family: Option<String>,
    font_weight: Option<FontWeight>,
    font_shape: Option<FontShape>,
}

impl<S, M, R> OptionsBuilder<S, M, R> {
    /// Math style
    pub fn style(self, style: &'static Style) -> OptionsBuilder<&'static Style, M, R> {
        OptionsBuilder {
            style,
            max_size: self.max_size,
            min_rule_thickness: self.min_rule_thickness,
            opt: self.opt,
        }
    }

    /// Maximum size
    pub fn max_size(self, max_size: f64) -> OptionsBuilder<S, f64, R> {
        OptionsBuilder {
            style: self.style,
            max_size,
            min_rule_thickness: self.min_rule_thickness,
            opt: self.opt,
        }
    }

    /// Minimum rule thickness
    pub fn min_rule_thickness(self, min_rule_thickness: f64) -> OptionsBuilder<S, M, f64> {
        OptionsBuilder {
            style: self.style,
            max_size: self.max_size,
            min_rule_thickness,
            opt: self.opt,
        }
    }

    optional_setters! {
        /// Text color
        color / maybe_color: String;
        /// Size index
        size / maybe_size: usize;
        /// Text size index
        text_size / maybe_text_size: usize;
        /// Whether the content is a phantom
        phantom / maybe_phantom: bool;
        /// Font
        font / maybe_font: String;
        /// Font family
        font_family / maybe_font_family: String;
        /// Font weight
        font_weight / maybe_font_weight: FontWeight;
        /// Font shape
        font_shape / maybe_font_shape: FontShape;
    }
}

impl OptionsBuilder<&'static Style, f64, f64> {
    /// Builds the options.
    #[must_use]
    pub fn build(self) -> Options {
        let opt = self.opt;
        let size = opt.size.unwrap_or(Options::BASESIZE);
        let multiplier_idx = cmp::min(size, SIZE_MULTIPLIERS.len());
        let size_multiplier = SIZE_MULTIPLIERS[multiplier_idx - 1];
        Options {
            style: self.style,
            color: opt.color,
            size,
            text_size: opt.text_size.unwrap_or(size),
            phantom: opt.phantom.unwrap_or(false),
            font: opt.font.unwrap_or_default(),
            font_family: opt.font_family.unwrap_or_default(),
            font_weight: opt.font_weight.unwrap_or(FontWeight::Empty),
            font_shape: opt.font_shape.unwrap_or(FontShape::Empty),
            size_multiplier,
            max_size: self.max_size,
            min_rule_thickness: self.min_rule_thickness,
            build_depth: BuildDepth::default(),
        }
    }
}

impl Options {
    /// Create a new Options instance with builder
    pub fn builder() -> OptionsBuilder {
        OptionsBuilder {
            style: Unset,
            max_size: Unset,
            min_rule_thickness: Unset,
            opt: OptionsOptional::default(),
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            style: TEXT,
            color: None,
            size: Self::BASESIZE,
            text_size: Self::BASESIZE,
            phantom: false,
            font: String::new(),
            font_family: String::new(),
            font_weight: FontWeight::Empty,
            font_shape: FontShape::Empty,
            size_multiplier: SIZE_MULTIPLIERS[Self::BASESIZE - 1],
            max_size: 1000.0,
            min_rule_thickness: 0.04,
            build_depth: BuildDepth::default(),
        }
    }
}

impl Options {
    /// The base size index
    pub const BASESIZE: usize = 6;

    /// Return an options object with the given style. If `self.style ===
    /// style`, returns `self`.
    #[must_use]
    pub fn having_style(&self, style: &'static Style) -> Self {
        if ptr::eq(self.style, style) {
            self.clone()
        } else {
            let size = size_at_style(self.text_size, style);
            let mut new_options = self.clone();

            // In Javascript there is a method called 'extend'
            // which copies properties from one object to another.
            // In Rust we can use the `clone` method for this purpose.
            new_options.style = style;
            new_options.size = size;
            new_options.size_multiplier = SIZE_MULTIPLIERS[size - 1];
            new_options
        }
    }

    /// Return an options object with a cramped version of the current style. If
    /// the current style is cramped, returns `self`.
    #[must_use]
    pub fn having_cramped_style(&self) -> Self {
        self.having_style(self.style.cramp())
    }

    /// Return an options object with the given size and in at least
    /// `\textstyle`. Returns `self` if appropriate.
    #[must_use]
    pub fn having_size(&self, size: usize) -> Self {
        if self.size == size && self.text_size == size {
            self.clone()
        } else {
            let mut new_options = self.clone();
            new_options.style = TEXT;
            new_options.size = size;
            new_options.text_size = size;
            new_options.size_multiplier = SIZE_MULTIPLIERS[size - 1];
            new_options
        }
    }

    /// Like `self.having_size(BASESIZE).having_style(style)`. If `style` is
    /// omitted, changes to at least `\textstyle`.
    #[must_use]
    pub fn having_base_style(&self, style: Option<&'static Style>) -> Self {
        let style = style.unwrap_or_else(|| self.style.text());
        let want_size = size_at_style(Self::BASESIZE, style);

        if self.size == want_size && self.text_size == Self::BASESIZE && self.style == style {
            self.clone()
        } else {
            let mut new_options = self.clone();
            new_options.style = style;
            new_options.size = want_size;
            new_options.size_multiplier = SIZE_MULTIPLIERS[want_size - 1];
            new_options
        }
    }

    /// Remove the effect of sizing changes such as \Huge.
    /// Keep the effect of the current style, such as \scriptstyle.
    #[must_use]
    pub fn having_base_sizing(&self) -> Self {
        let size = match self.style.id {
            4 | 5 => 3, // normalsize in scriptstyle
            6 | 7 => 1, // normalsize in scriptscriptstyle
            _ => 6,     // normalsize in textstyle or displaystyle
        };

        let mut new_options = self.clone();
        new_options.style = self.style.text();
        new_options.size = size;
        new_options.size_multiplier = SIZE_MULTIPLIERS[size - 1];
        new_options
    }

    /// Create a new options object with the given color.
    #[must_use]
    pub fn with_color(&self, color: &str) -> Self {
        let mut new_options = self.clone();
        new_options.color = Some(color.to_owned());
        new_options
    }

    /// Create a new options object with "phantom" set to true.
    #[must_use]
    pub fn with_phantom(&self) -> Self {
        let mut new_options = self.clone();
        new_options.phantom = true;
        new_options
    }

    /// Creates a new options object with the given math font or old text font.
    #[must_use]
    pub fn with_font(&self, font: String) -> Self {
        let mut new_options = self.clone();
        new_options.font = font;
        new_options
    }

    /// Create a new options objects with the given fontFamily.
    #[must_use]
    pub fn with_text_font_family(&self, font_family: String) -> Self {
        let mut new_options = self.clone();
        new_options.font_family = font_family;
        new_options.font = String::new();
        new_options
    }

    /// Creates a new options object with the given font weight
    #[must_use]
    pub fn with_text_font_weight(&self, font_weight: FontWeight) -> Self {
        let mut new_options = self.clone();
        new_options.font_weight = font_weight;
        new_options.font.clear();
        new_options
    }

    /// Creates a new options object with the given font shape
    #[must_use]
    pub fn with_text_font_shape(&self, font_shape: FontShape) -> Self {
        let mut new_options = self.clone();
        new_options.font_shape = font_shape;
        new_options.font.clear();
        new_options
    }

    /// Return the CSS sizing classes required to switch from enclosing options
    /// `old_options` to `self`. Returns an array of classes.
    #[must_use]
    pub fn sizing_classes(&self, old_options: &Self) -> Vec<Cow<'static, str>> {
        if old_options.size == self.size {
            vec![]
        } else {
            vec![
                Cow::Borrowed("katex-sizing"),
                Cow::Owned(format!("reset-size{}", old_options.size)),
                Cow::Owned(format!("size{}", self.size)),
            ]
        }
    }

    /// Return the CSS sizing classes required to switch to the base size. Like
    /// `self.having_size(BASESIZE).sizing_classes(self)`.
    #[must_use]
    pub fn base_sizing_classes(&self) -> Vec<Cow<'static, str>> {
        if self.size == Self::BASESIZE {
            vec![]
        } else {
            vec![
                Cow::Borrowed("katex-sizing"),
                Cow::Owned(format!("reset-size{}", self.size)),
                Cow::Owned(format!("size{}", Self::BASESIZE)),
            ]
        }
    }

    /// Gets the CSS color of the current options object
    #[must_use]
    pub fn get_color(&self) -> Option<&str> {
        if self.phantom {
            Some("transparent")
        } else {
            self.color.as_deref()
        }
    }

    /// Get font metrics for the current options
    #[must_use]
    pub const fn font_metrics(&self) -> &FontMetrics {
        use crate::font_metrics::FontSizeIndex;

        // Determine size index based on size
        let size_index: FontSizeIndex = if self.size >= 5 {
            0
        } else if self.size >= 3 {
            1
        } else {
            2
        };

        &FONT_METRICS[size_index as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{DISPLAY, SCRIPT, SCRIPTSCRIPT, TEXT};

    #[test]
    fn test_font_shape_conversion() {
        assert_eq!(FontShape::from("textit"), FontShape::TextIt);
        assert_eq!(FontShape::from("textup"), FontShape::TextUp);
        assert_eq!(FontShape::from("unknown"), FontShape::Empty);

        assert_eq!(FontShape::TextIt.as_str(), "textit");
        assert_eq!(FontShape::TextUp.as_str(), "textup");
        assert_eq!(FontShape::Empty.as_str(), "");
    }

    #[test]
    fn test_size_at_style() {
        // Test with display style (size = 0)
        assert_eq!(size_at_style(6, DISPLAY), 6);

        // Test with script style (size = 2)
        assert_eq!(size_at_style(6, SCRIPT), 3);

        // Test with scriptscript style (size = 3)
        assert_eq!(size_at_style(6, SCRIPTSCRIPT), 1);
    }

    #[test]
    fn test_options_creation() {
        let options = Options::builder()
            .style(TEXT)
            .color("red".to_owned())
            .size(5)
            .text_size(5)
            .font("Main-Regular".to_owned())
            .font_family("Main".to_owned())
            .font_weight(FontWeight::TextBf)
            .font_shape(FontShape::TextIt)
            .max_size(1000.0)
            .min_rule_thickness(0.04)
            .build();

        assert_eq!(options.style, TEXT);
        assert_eq!(options.color, Some("red".to_owned()));
        assert_eq!(options.size, 5);
        assert_eq!(options.text_size, 5);
        assert_eq!(options.font, "Main-Regular");
        assert_eq!(options.font_family, "Main");
        assert_eq!(options.font_weight, FontWeight::TextBf);
        assert_eq!(options.font_shape, FontShape::TextIt);
        assert_eq!(options.size_multiplier, SIZE_MULTIPLIERS[4]); // index 4 for size 5
        assert_eq!(options.max_size, 1000.0);
        assert_eq!(options.min_rule_thickness, 0.04);
    }
}
