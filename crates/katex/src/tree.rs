//! Tree structure definitions for virtual DOM nodes
//!
//! This module contains the foundational types for KaTeX's virtual DOM system,
//! including the base VirtualNode trait and DocumentFragment structure.

use core::cell::RefCell;
use core::fmt::{self, Write as _};

use crate::ClassList;
use crate::ParseError;
use crate::types::{CssStyle, ParseErrorKind};
#[cfg(feature = "wasm")]
use crate::web_context::WebContext;
#[cfg(feature = "wasm")]
use wasm_bindgen::UnwrapThrowExt as _;
#[cfg(feature = "wasm")]
use web_sys;

/// Base virtual DOM node interface used in both DOM tree and MathML tree
/// implementations
pub trait VirtualNode {
    /// Convert into HTML markup by writing into the provided formatter.
    fn write_markup(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), ParseError>;

    /// Convenience helper that renders the node into a [`String`].
    fn to_markup(&self) -> Result<String, ParseError>
    where
        Self: Sized,
    {
        markup_to_string(self)
    }

    /// Convert into a DOM node
    #[cfg(feature = "wasm")]
    fn to_node(&self, ctx: &WebContext) -> web_sys::Node;
}

/// Document fragment containing elements without DOM representation
#[derive(Debug)]
pub struct DocumentFragment<ChildType: VirtualNode> {
    /// The child nodes contained within this document fragment. These represent
    /// the nested elements in the virtual DOM tree structure used for
    /// mathematical rendering in KaTeX. Each child is a VirtualNode that
    /// can be rendered into HTML or MathML markup.
    ///
    /// # See Also
    /// - [`VirtualNode`]: The trait implemented by child nodes
    /// - [`DocumentFragment::new`]: Constructor for creating fragments with
    ///   children
    pub children: Vec<ChildType>,

    /// CSS class names applied to this document fragment. These classes control
    /// styling and layout in the rendered mathematical output, following
    /// KaTeX's CSS conventions.
    ///
    /// # See Also
    /// - [`DocumentFragment::has_class`]: Method to check for specific classes
    pub classes: ClassList,

    /// The height of this document fragment in em units, representing the
    /// vertical extent above the baseline in mathematical rendering. Used
    /// for proper alignment and spacing in LaTeX expressions.
    ///
    /// # Mathematical Context
    /// In KaTeX, height is crucial for aligning superscripts, fractions, and
    /// other vertical elements.
    pub height: f64,

    /// The depth of this document fragment in em units, representing the
    /// vertical extent below the baseline in mathematical rendering. Used
    /// for proper alignment and spacing in LaTeX expressions.
    ///
    /// # Mathematical Context
    /// In KaTeX, depth is crucial for aligning subscripts, fractions, and other
    /// vertical elements.
    pub depth: f64,

    /// The maximum font size used within this document fragment, in points.
    /// This value determines the scaling factor for the entire fragment in
    /// mathematical rendering.
    ///
    /// # See Also
    /// - Font metrics in KaTeX for size calculations
    pub max_font_size: f64,

    /// Inline CSS styles applied to this document fragment. These styles
    /// override default KaTeX styling and allow for custom rendering
    /// adjustments in mathematical expressions.
    ///
    /// # See Also
    /// - [`CssStyle`]: The type used for style properties
    /// - KaTeX CSS documentation for available style properties
    pub style: CssStyle,
}

impl<ChildType: VirtualNode + Clone> Clone for DocumentFragment<ChildType> {
    fn clone(&self) -> Self {
        Self {
            children: self.children.clone(),
            classes: self.classes.clone(),
            height: self.height,
            depth: self.depth,
            max_font_size: self.max_font_size,
            style: self.style.clone(),
        }
    }
}

impl<ChildType: VirtualNode> DocumentFragment<ChildType> {
    /// Create a new document fragment with the given children
    #[must_use]
    pub fn new(children: Vec<ChildType>) -> Self {
        Self {
            children,
            classes: ClassList::Empty,
            height: 0.0,
            depth: 0.0,
            max_font_size: 0.0,
            style: CssStyle::default(),
        }
    }

    /// Check if the fragment has a specific class
    #[must_use]
    pub fn has_class(&self, class_name: &str) -> bool {
        self.classes.contains(class_name)
    }
}

impl<ChildType: VirtualNode + Clone + 'static> VirtualNode for DocumentFragment<ChildType> {
    fn write_markup(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), ParseError> {
        for child in &self.children {
            child.write_markup(fmt)?;
        }
        Ok(())
    }

    #[cfg(feature = "wasm")]
    fn to_node(&self, ctx: &WebContext) -> web_sys::Node {
        use wasm_bindgen::JsCast as _;

        let fragment = ctx.document.create_document_fragment();

        for child in &self.children {
            let node = child.to_node(ctx);
            fragment.append_child(&node).unwrap_throw();
        }

        fragment.dyn_into::<web_sys::Node>().unwrap_throw()
    }
}

pub use crate::dom_tree::HtmlDomNode;
pub use crate::mathml_tree::MathDomNode;

use crate::dom_tree::write_open_tag;
use crate::mathml_tree::MathNode;

/// A pending step of [`write_tree_markup`].
pub(crate) enum MarkupStep<'a> {
    /// Write an HTML node and its subtree.
    Html(&'a HtmlDomNode),
    /// Write a MathML node and its subtree.
    Math(&'a MathDomNode),
    /// Write a MathML element and its subtree.
    MathNode(&'a MathNode),
    /// Write the closing tag `</name>`.
    Close(&'a str),
}

/// Serializes an HTML or MathML tree with an explicit stack instead of
/// recursion, so the native stack used does not grow with the tree's depth.
/// The output is identical to a recursive pre-order walk.
pub(crate) fn write_tree_markup(
    root: MarkupStep<'_>,
    fmt: &mut fmt::Formatter<'_>,
) -> Result<(), ParseError> {
    let mut stack = vec![root];
    while let Some(step) = stack.pop() {
        match step {
            MarkupStep::Html(node) => match node {
                HtmlDomNode::DomSpan(span) => {
                    write_open_tag(fmt, "span", &span.classes, &span.style, &span.attributes)?;
                    stack.push(MarkupStep::Close("span"));
                    stack.extend(span.children.iter().rev().map(MarkupStep::Html));
                }
                HtmlDomNode::Anchor(anchor) => {
                    write_open_tag(fmt, "a", &anchor.classes, &anchor.style, &anchor.attributes)?;
                    stack.push(MarkupStep::Close("a"));
                    stack.extend(anchor.children.iter().rev().map(MarkupStep::Html));
                }
                HtmlDomNode::Fragment(fragment) => {
                    stack.extend(fragment.children.iter().rev().map(MarkupStep::Html));
                }
                HtmlDomNode::MathML(math_node) => stack.push(MarkupStep::MathNode(math_node)),
                // Leaves (an SVG node's children are paths and lines).
                HtmlDomNode::Img(img) => img.write_markup(fmt)?,
                HtmlDomNode::Symbol(symbol) => symbol.write_markup(fmt)?,
                HtmlDomNode::SvgNode(svg_node) => svg_node.write_markup(fmt)?,
            },
            MarkupStep::Math(node) => match node {
                MathDomNode::Math(math_node) => stack.push(MarkupStep::MathNode(math_node)),
                MathDomNode::Fragment(fragment) => {
                    stack.extend(fragment.children.iter().rev().map(MarkupStep::Math));
                }
                MathDomNode::Text(text) => text.write_markup(fmt)?,
                MathDomNode::Space(space) => space.write_markup(fmt)?,
            },
            MarkupStep::MathNode(math_node) => {
                math_node.write_open_tag(fmt)?;
                stack.push(MarkupStep::Close(math_node.node_type.as_ref()));
                stack.extend(math_node.children.iter().rev().map(MarkupStep::Math));
            }
            MarkupStep::Close(name) => {
                fmt.write_str("</")
                    .and_then(|()| fmt.write_str(name))
                    .and_then(|()| fmt.write_char('>'))
                    .map_err(ParseError::from)?;
            }
        }
    }
    Ok(())
}

/// Renders the provided [`VirtualNode`] into a [`String`] buffer.
pub fn markup_to_string<T: VirtualNode + ?Sized>(node: &T) -> Result<String, ParseError> {
    struct FormatterWriter<'a> {
        buf: &'a mut String,
    }

    impl fmt::Write for FormatterWriter<'_> {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            self.buf.push_str(s);
            Ok(())
        }
    }

    struct DisplayAdapter<'a, T: VirtualNode + ?Sized> {
        node: &'a T,
        error: &'a RefCell<Option<ParseError>>,
    }

    impl<T: VirtualNode + ?Sized> fmt::Display for DisplayAdapter<'_, T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self.node.write_markup(f) {
                Ok(()) => Ok(()),
                Err(err) => {
                    self.error.replace(Some(err));
                    Err(fmt::Error)
                }
            }
        }
    }

    let mut buffer = String::new();
    let error = RefCell::new(None);
    let mut writer = FormatterWriter { buf: &mut buffer };
    let adapter = DisplayAdapter {
        node,
        error: &error,
    };

    if fmt::write(&mut writer, format_args!("{adapter}")).is_err() {
        if let Some(err) = error.into_inner() {
            return Err(err);
        }
        return Err(ParseErrorKind::MarkupWriteFailure.into());
    }

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::forget;
    use std::thread::Builder;

    use crate::build_common::make_span;
    use crate::mathml_tree::{MathNodeType, TextNode};

    const DEPTH: usize = 50_000;

    /// Runs `f` on a thread whose stack is far smaller than a recursive walk of
    /// a `DEPTH`-deep tree would need.
    fn on_small_stack(f: impl FnOnce() + Send + 'static) {
        Builder::new()
            .stack_size(256 << 10)
            .spawn(f)
            .expect("spawn")
            .join()
            .expect("serialization overflowed or panicked");
    }

    #[test]
    fn html_markup_is_written_without_recursion() {
        on_small_stack(|| {
            let mut node: HtmlDomNode = make_span("x", vec![], None, None).into();
            for _ in 0..DEPTH {
                node = make_span("x", vec![node], None, None).into();
            }
            let markup = node.to_markup().expect("markup");
            assert_eq!(
                markup.len(),
                (DEPTH + 1) * r#"<span class="x"></span>"#.len()
            );
            assert!(markup.starts_with(r#"<span class="x"><span class="x">"#));
            // Dropping is recursive; this test is only about serialization.
            forget(node);
        });
    }

    #[test]
    fn mathml_markup_and_text_are_written_without_recursion() {
        on_small_stack(|| {
            let mut node = MathDomNode::Text(TextNode {
                text: "a<".to_owned(),
            });
            for _ in 0..DEPTH {
                node = MathNode::with_children(MathNodeType::Mrow, vec![node]).into();
            }
            let markup = node.to_markup().expect("markup");
            assert_eq!(markup.len(), DEPTH * "<mrow></mrow>".len() + "a&lt;".len());
            assert_eq!(node.to_text(), "a<");
            forget(node);
        });
    }
}
