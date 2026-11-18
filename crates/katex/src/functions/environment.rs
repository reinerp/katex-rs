//! Environment function implementations for KaTeX Rust
//!
//! This module handles the `\begin` and `\end` commands for LaTeX environments,
//! providing the core functionality for structured content like matrices,
//! arrays, and other mathematical constructs.
//!
//! Migrated from KaTeX's functions/environment.js.

use crate::KatexContext;
use crate::define_environment::EnvContext;
use crate::define_function::{FunctionDefSpec, FunctionPropSpec};
use crate::parser::parse_node::{AnyParseNode, NodeType, ParseNode, ParseNodeEnvironment};
use crate::types::{ArgType, Mode, ParseError, ParseErrorKind};

/// The `Debug` text of a node or token for an error message; without the `diagnostics` feature a placeholder (the `Debug` code of every parse node stays out of the binary).
#[cfg(feature = "diagnostics")]
fn debug_text<T: core::fmt::Debug>(value: &T) -> String {
    format!("{value:?}")
}

/// See above.
#[cfg(not(feature = "diagnostics"))]
fn debug_text<T>(_value: &T) -> String {
    "?".to_owned()
}

/// Environment delimiters. HTML/MathML rendering is defined in the
/// corresponding defineEnvironment definitions.
pub fn define_environment(ctx: &mut KatexContext) {
    let spec = FunctionDefSpec {
        node_type: Some(NodeType::Environment),
        names: &["\\begin", "\\end"],
        props: FunctionPropSpec {
            num_args: 1,
            arg_types: Some(vec![ArgType::Mode(Mode::Text)]),
            ..Default::default()
        },
        handler: Some(|context, args, _opt_args| {
            let func_name = context.func_name;
            let parser = context.parser;
            let ParseNode::OrdGroup(name_group) = &args[0] else {
                return Err(ParseError::new(ParseErrorKind::InvalidEnvironmentName {
                    value: debug_text(&args[0]),
                }));
            };
            let mut env_name = String::new();
            for item in &name_group.body {
                if let AnyParseNode::TextOrd(text_ord) = item {
                    env_name.push_str(&text_ord.text);
                } else if matches!(item, AnyParseNode::Spacing(spacing) if spacing.text == " ") {
                    env_name.push(' ');
                } else {
                    return Err(ParseError::new(ParseErrorKind::InvalidEnvironmentName {
                        value: debug_text(&args[0]),
                    }));
                }
            }

            // Handles \\end
            if func_name != "\\begin" {
                let env = ParseNodeEnvironment {
                    mode: parser.mode,
                    loc: None,
                    name: env_name.clone(),
                    // right_delim: None,
                    // size: None,
                    // bar_size: None,
                    name_group: args[0].clone().into(),
                };
                return Ok(env.into());
            }

            // begin...end is similar to left...right
            let Some(env_spec) = parser.ctx.environments.get(&env_name) else {
                return Err(ParseError::new(ParseErrorKind::NoSuchEnvironment {
                    name: env_name.clone(),
                }));
            };

            // Build the environment object. Arguments and other information
            // will be made available to the begin and end methods
            // using properties.
            let (args, opt_args) =
                parser.parse_arguments(&format!("\\begin{{{env_name}}}"), env_spec.as_ref())?;
            let env_context = EnvContext {
                mode: parser.mode,
                parser,
                env_name: env_name.clone(),
            };
            let result = (env_spec.handler)(env_context, args, opt_args)?;
            parser.expect("\\end", false)?;
            let end_name_token = parser.next_token.clone();
            let Some(ParseNode::Environment(end_node)) = parser.parse_function(None, None)? else {
                return Err(ParseError::new(
                    ParseErrorKind::ExpectedEnvironmentAfterEnd {
                        found: debug_text(&end_name_token),
                    },
                ));
            };
            if end_node.name != env_name {
                return Err(ParseError::new(ParseErrorKind::MismatchedEnvironmentEnd {
                    begin: env_name,
                    end: end_node.name,
                }));
            }
            Ok(result)
        }),
        html_builder: None, // Environment-specific builders are handled by individual environments
        mathml_builder: None,
    };

    ctx.define_function(spec);
}
