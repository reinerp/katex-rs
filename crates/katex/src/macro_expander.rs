//! MacroExpander – the “gullet” that expands macros to tokens
//!
//! Ported from KaTeX/src/MacroExpander.js with adjustments to fit the Rust
//! codebase.

use crate::context::KatexContext;
use crate::lexer::Lexer;
use crate::macros::builtins::BUILTIN_MACROS;
use crate::namespace::{KeyMap, Namespace};
use crate::types::TokenText;
use crate::types::{
    ErrorLocationProvider, LexerInterface as _, Mode, ParseError, ParseErrorKind, Settings,
    SourceLocation, Token,
};
use alloc::sync::Arc;
use core::ptr;

use crate::macros::{
    MacroArg, MacroContextInterface, MacroDefinition, MacroExpansion, MacroExpansionResult,
};

/// Map of macro definitions.
pub type MacroMap = KeyMap<String, MacroDefinition>;

/// Commands that act like macros but aren't defined as a macro, function, or
/// symbol
pub const IMPLICIT_COMMANDS: phf::Set<&str> = phf::phf_set! {
    "^",
    "_",
    "\\limits",
    "\\nolimits",
};

/// Commands that define or alias macros, rejected in the input when
/// [`Settings::allow_macro_definitions`] is `false`. The names starting with
/// two backslashes are internal forms of `\global` combinations.
pub const MACRO_DEFINITION_COMMANDS: phf::Set<&str> = phf::phf_set! {
    "\\def",
    "\\gdef",
    "\\edef",
    "\\xdef",
    "\\let",
    "\\futurelet",
    "\\global",
    "\\long",
    "\\newcommand",
    "\\renewcommand",
    "\\providecommand",
    "\\\\globallong",
    "\\\\globallet",
    "\\\\globalfuture",
};

/// MacroExpander: expands macros until only non-macro tokens remain
pub struct MacroExpander<'a> {
    settings: &'a Settings,
    expansion_count: usize,
    /// Tokens produced by expansion so far; see [`Settings::max_expanded_tokens`].
    expanded_tokens: usize,
    /// Current nesting depth, shared with the parser; see [`Self::enter_nesting`].
    nesting_depth: usize,
    lexer: Lexer<'a>,
    macros: Namespace<'a, MacroDefinition>,
    stack: Vec<Token>, // tokens in REVERSE order
    mode: Mode,
    /// No global object in Rust; pass context reference around
    ctx: &'a KatexContext,
}

impl<'a> MacroExpander<'a> {
    /// Create a new MacroExpander (also creates a new Lexer)
    #[must_use]
    pub fn new(input: &str, settings: &'a Settings, mode: Mode, ctx: &'a KatexContext) -> Self {
        // Build macros namespace: builtins from context, globals from
        // settings.macros
        let globals = settings.macros.borrow_mut();
        let macros = Namespace::new(&BUILTIN_MACROS, globals);

        Self {
            lexer: Lexer::new(Arc::from(input), settings),
            settings,
            expansion_count: 0,
            expanded_tokens: 0,
            nesting_depth: 0,
            macros,
            mode,
            stack: Vec::new(),

            ctx,
        }
    }

    /// Feed a new input string to the same MacroExpander (with existing macros
    /// etc.).
    pub fn feed(&mut self, input: &str) {
        self.lexer = Lexer::new(Arc::from(input), self.settings);
    }

    /// Switches between text and math modes
    pub const fn switch_mode(&mut self, new_mode: Mode) {
        self.mode = new_mode;
    }

    /// Ends all currently nested groups (if any)
    pub fn end_groups(&mut self) {
        self.macros.end_groups();
    }

    /// Sets the category code for a character in the lexer
    pub fn set_catcode(&mut self, char: char, code: u8) {
        self.lexer.set_catcode(char, code);
    }

    /// Add a token to the stack
    pub fn push_token(&mut self, token: Token) {
        self.stack.push(token);
    }

    /// Append multiple tokens to the stack
    pub fn push_tokens(&mut self, tokens: Vec<Token>) {
        self.stack.extend(tokens);
    }

    /// Find a macro argument without expanding tokens and append the array of
    /// tokens to the token stack Returns a Token representing the argument
    /// range, or None for missing optional arg
    pub fn scan_argument(&mut self, is_optional: bool) -> Result<Option<Token>, ParseError> {
        let (start_tok, end_tok, tokens);
        if is_optional {
            self.consume_spaces()?;
            if self.future_mut()?.text != "[" {
                return Ok(None);
            }
            let start = self.pop_token()?; // drop [
            let arg = self.consume_arg(Some(&vec!["]".to_owned()]))?;
            start_tok = start;
            end_tok = arg.end;
            tokens = arg.tokens;
        } else {
            let arg = self.consume_arg(None)?;
            start_tok = arg.start;
            end_tok = arg.end;
            tokens = arg.tokens;
        }

        // indicate the end of an argument
        self.push_token(Token::new("EOF", None));
        self.push_tokens(tokens);

        // compute range token with empty text
        let loc = SourceLocation::range(start_tok.loc, end_tok.loc);
        Ok(Some(Token {
            text: TokenText::from(String::new()),
            loc,
            noexpand: None,
            treat_as_relax: None,
        }))
    }

    /// Consume specified number of arguments with optional delimiters
    fn consume_args_with_delims(
        &mut self,
        num_args: usize,
        delimiters: Option<&Vec<Vec<String>>>,
    ) -> Result<Vec<Vec<Token>>, ParseError> {
        if let Some(d) = delimiters {
            if d.len() != num_args + 1 {
                return Err(ParseError::new(
                    ParseErrorKind::MacroDelimiterLengthMismatch,
                ));
            }
            for expected in &d[0] {
                let tok = self.pop_token()?;
                if expected != &tok.text {
                    return Err(ParseError::with_token(
                        ParseErrorKind::MacroDefinitionMismatch,
                        &tok,
                    ));
                }
            }
        }

        let mut args: Vec<Vec<Token>> = Vec::new();
        for i in 0..num_args {
            let delims_for_arg = delimiters.as_ref().map(|v| &v[i + 1]);
            let arg = self.consume_arg(delims_for_arg)?;
            args.push(arg.tokens);
        }
        Ok(args)
    }

    /// Enters one level of nesting: a group, command or environment being
    /// parsed, or a call of a function macro (which may expand further macros
    /// recursively). Fails, blaming `at`, if that would exceed
    /// [`Settings::max_nesting`]. Each successful call must be paired with
    /// [`Self::exit_nesting`].
    pub fn enter_nesting(&mut self, at: &dyn ErrorLocationProvider) -> Result<(), ParseError> {
        if self.nesting_depth >= self.settings.max_nesting {
            return Err(ParseError::with_token(
                ParseErrorKind::TooDeeplyNested {
                    max: self.settings.max_nesting,
                },
                at,
            ));
        }
        self.nesting_depth += 1;
        Ok(())
    }

    /// Leaves a level entered with [`Self::enter_nesting`].
    pub const fn exit_nesting(&mut self) {
        self.nesting_depth = self.nesting_depth.saturating_sub(1);
    }

    /// Fails if `token` is a macro-definition command written in the input
    /// while [`Settings::allow_macro_definitions`] is `false`. Definitions
    /// produced by KaTeX's own macros (`\tag` uses `\gdef`) or by
    /// [`Settings::macros`] come from their own strings and are allowed.
    pub fn check_macro_definition(&self, token: &Token) -> Result<(), ParseError> {
        let name = token.text.as_str();
        if self.settings.allow_macro_definitions
            || !MACRO_DEFINITION_COMMANDS.contains(name)
            || !token
                .loc
                .as_ref()
                .is_some_and(|loc| ptr::eq(loc.input.as_ptr(), self.lexer.input().as_ptr()))
        {
            return Ok(());
        }
        Err(ParseError::with_token(
            ParseErrorKind::MacroDefinitionsDisallowed {
                command: name.to_owned(),
            },
            token,
        ))
    }

    /// Adds `count` to the tokens produced by expansion, failing (blaming
    /// `macro_token`) if that exceeds [`Settings::max_expanded_tokens`].
    fn count_expanded_tokens(
        &mut self,
        count: usize,
        macro_token: &Token,
    ) -> Result<(), ParseError> {
        self.expanded_tokens = self.expanded_tokens.saturating_add(count);
        if self.expanded_tokens > self.settings.max_expanded_tokens {
            return Err(ParseError::with_token(
                ParseErrorKind::TooManyExpandedTokens {
                    max: self.settings.max_expanded_tokens,
                },
                macro_token,
            ));
        }
        Ok(())
    }

    /// Replaces the placeholders `#1`..`#9` in a macro body with the
    /// arguments, and `##` with `#`. The body, the arguments and the result
    /// are in reverse order, like the token stack. Builds the result in one
    /// pass and counts it against [`Settings::max_expanded_tokens`] before
    /// copying each argument, so an expansion that copies a long argument
    /// many times fails before allocating it.
    fn substitute_args(
        &mut self,
        body: &[Token],
        args: &[Vec<Token>],
        macro_token: &Token,
    ) -> Result<Vec<Token>, ParseError> {
        let mut out: Vec<Token> = Vec::with_capacity(body.len());
        // Walk the body in reading order (from the end), emitting in reading
        // order, then reverse.
        let mut i = body.len();
        while i > 0 {
            i -= 1;
            let tok = &body[i];
            if tok.text != "#" {
                self.count_expanded_tokens(1, macro_token)?;
                out.push(tok.clone());
                continue;
            }
            if i == 0 {
                return Err(ParseError::with_token(
                    ParseErrorKind::MacroIncompletePlaceholder,
                    tok,
                ));
            }
            i -= 1;
            let next = &body[i];
            if next.text == "#" {
                // ## -> #
                self.count_expanded_tokens(1, macro_token)?;
                out.push(next.clone());
                continue;
            }
            let arg = (next.text.len() == 1)
                .then(|| next.text.as_str().parse::<usize>().ok())
                .flatten()
                .and_then(|n| n.checked_sub(1))
                .and_then(|index| args.get(index));
            let Some(arg) = arg else {
                return Err(ParseError::with_token(
                    ParseErrorKind::InvalidMacroArgumentNumber {
                        value: next.text.to_owned_string(),
                    },
                    next,
                ));
            };
            self.count_expanded_tokens(arg.len(), macro_token)?;
            out.extend(arg.iter().rev().cloned());
        }
        out.reverse();
        Ok(out)
    }

    /// Increment expansion counter and check against max_expand
    fn count_expansion(&mut self, amount: usize) -> Result<(), ParseError> {
        self.expansion_count += amount;
        if self.expansion_count > self.settings.max_expand {
            return Err(ParseError::new(ParseErrorKind::MacroTooManyExpansions));
        }
        Ok(())
    }

    /// Expand the next token only once if possible
    fn expand_once_internal(&mut self, expandable_only: bool) -> Result<Option<isize>, ParseError> {
        let top_token = self.pop_token()?;
        let name = top_token.text.as_str();
        let expansion = if top_token.noexpand == Some(true) {
            None
        } else {
            self.get_expansion(&top_token)?
        };

        let expansion = match expansion {
            Some(exp) if !(expandable_only && exp.unexpandable == Some(true)) => exp,
            _ => {
                if expandable_only
                    && expansion.is_none()
                    && name.starts_with('\\')
                    && !self.is_defined(name)
                {
                    return Err(ParseError::with_token(
                        ParseErrorKind::UndefinedControlSequence {
                            name: name.to_owned(),
                        },
                        &top_token,
                    ));
                }
                self.push_token(top_token);
                return Ok(None);
            }
        };

        self.count_expansion(1)?;
        let tokens = expansion.tokens;
        let args =
            self.consume_args_with_delims(expansion.num_args, expansion.delimiters.as_ref())?;
        let tokens = if expansion.num_args > 0 {
            self.substitute_args(&tokens, &args, &top_token)?
        } else {
            self.count_expanded_tokens(tokens.len(), &top_token)?;
            tokens
        };
        let count = tokens.len();
        self.push_tokens(tokens);
        Ok(Some(count as isize))
    }

    /// Fully expand the given token stream to forward-order tokens
    fn expand_tokens_internal(&mut self, tokens: Vec<Token>) -> Result<Vec<Token>, ParseError> {
        let mut output: Vec<Token> = Vec::new();
        let old_len = self.stack.len();
        self.push_tokens(tokens);
        while self.stack.len() > old_len {
            if self.expand_once_internal(true)?.is_none() {
                let mut token = self
                    .stack
                    .pop()
                    .ok_or_else(|| ParseError::new(ParseErrorKind::MacroStackUnexpectedlyEmpty))?;
                if token.treat_as_relax == Some(true) {
                    // the expansion of \noexpand is the token itself
                    token.noexpand = Some(false);
                    token.treat_as_relax = Some(false);
                }
                output.push(token);
            }
        }
        self.count_expansion(output.len())?;
        Ok(output)
    }

    /// Compute expansion for a name. Errors raised by function macros (for
    /// example `\newcommand` redefining a command) propagate, as in KaTeX.
    fn get_expansion(&mut self, token: &Token) -> Result<Option<MacroExpansion>, ParseError> {
        let name = token.text.as_str();
        self.check_macro_definition(token)?;
        // If single character has a catcode other than 13 (active), don't
        // expand it
        if name.chars().count() == 1
            && let Some(ch) = name.chars().next()
            && let Some(catcode) = self.lexer.get_catcode(ch)
            && catcode != 13
        {
            return Ok(None);
        }

        let Some(definition) = self.macros.get(name).cloned() else {
            return Ok(None);
        };

        // Function macros may expand further macros (for example
        // \expandafter), recursing, so each call counts as a nesting level.
        let result = match definition {
            MacroDefinition::Function(f) => {
                self.enter_nesting(token)?;
                let result = f(self as &mut dyn MacroContextInterface);
                self.exit_nesting();
                result?
            }
            MacroDefinition::StaticFunction(f) => {
                self.enter_nesting(token)?;
                let result = f(self as &mut dyn MacroContextInterface);
                self.exit_nesting();
                result?
            }
            MacroDefinition::StaticStr(s) => return Ok(Some(self.string_to_expansion(s))),
            MacroDefinition::String(s) => return Ok(Some(self.string_to_expansion(&s))),
            MacroDefinition::Expansion(e) => return Ok(Some(e)),
        };
        Ok(Some(match result {
            MacroExpansionResult::String(s) => self.string_to_expansion(&s),
            MacroExpansionResult::Expansion(e) => e,
            MacroExpansionResult::Empty => MacroExpansion::default(),
        }))
    }

    fn string_to_expansion(&self, expansion: &str) -> MacroExpansion {
        let mut num_args = 0usize;
        if expansion.contains('#') {
            let stripped = expansion.replace("##", "");
            while stripped.contains(&format!("#{}", num_args + 1)) {
                num_args += 1;
            }
        }

        let mut body_lexer = Lexer::new(Arc::from(expansion), self.settings);
        let mut tokens: Vec<Token> = Vec::new();
        while let Ok(tok) = body_lexer.lex() {
            if tok.text == "EOF" {
                break;
            }
            tokens.push(tok);
        }
        tokens.reverse();
        MacroExpansion {
            tokens,
            num_args,
            delimiters: None,
            unexpandable: None,
        }
    }
}

impl<'a> MacroContextInterface<'a> for MacroExpander<'a> {
    fn mode(&self) -> Mode {
        self.mode
    }

    fn context(&self) -> &KatexContext {
        self.ctx
    }

    fn macros<'s>(&'s self) -> &'s Namespace<'a, MacroDefinition> {
        &self.macros
    }

    fn macros_mut<'s>(&'s mut self) -> &'s mut Namespace<'a, MacroDefinition> {
        &mut self.macros
    }

    fn future_mut(&mut self) -> Result<Token, ParseError> {
        if self.stack.is_empty() {
            let tok = self.lexer.lex()?;
            self.push_token(tok);
        }
        self.stack
            .last()
            .cloned()
            .ok_or_else(|| ParseError::new(ParseErrorKind::EmptyMacroExpanderStack))
    }

    fn pop_token(&mut self) -> Result<Token, ParseError> {
        // No lookahead clone is needed when the token is immediately consumed.
        match self.stack.pop() {
            Some(token) => Ok(token),
            None => self.lexer.lex(),
        }
    }

    fn consume_spaces(&mut self) -> Result<(), ParseError> {
        loop {
            let token = self.future_mut()?;
            if token.text == " " {
                self.stack.pop();
            } else {
                break;
            }
        }
        Ok(())
    }

    fn expand_once(&mut self, expandable_only: Option<bool>) -> Result<Option<isize>, ParseError> {
        self.expand_once_internal(expandable_only.unwrap_or(false))
    }

    fn expand_after_future(&mut self) -> Result<Token, ParseError> {
        self.expand_once_internal(false)?;
        self.future_mut()
    }

    fn expand_next_token(&mut self) -> Result<Token, ParseError> {
        loop {
            if self.expand_once_internal(false)?.is_none() {
                let mut token = self
                    .stack
                    .pop()
                    .ok_or_else(|| ParseError::new(ParseErrorKind::MacroStackUnexpectedlyEmpty))?;
                if token.treat_as_relax == Some(true) {
                    token.set_text("\\relax");
                }
                return Ok(token);
            }
        }
    }

    fn expand_macro(&mut self, name: &str) -> Result<Option<Vec<Token>>, ParseError> {
        if self.macros.has(name) {
            let toks = self.expand_tokens_internal(vec![Token::new(name.to_owned(), None)])?;
            Ok(Some(toks))
        } else {
            Ok(None)
        }
    }

    fn expand_macro_as_text(&mut self, name: &str) -> Result<Option<String>, ParseError> {
        self.expand_macro(name)?.map_or(Ok(None), |tokens| {
            let mut s = String::new();
            for t in tokens {
                s.push_str(t.text.as_str());
            }
            Ok(Some(s))
        })
    }

    fn expand_tokens(&mut self, tokens: Vec<Token>) -> Result<Vec<Token>, ParseError> {
        let toks: Vec<Token> = tokens.into_iter().collect();
        self.expand_tokens_internal(toks)
    }

    fn consume_arg(&mut self, delims: Option<&Vec<String>>) -> Result<MacroArg, ParseError> {
        let mut tokens: Vec<Token> = Vec::new();
        let is_delimited = delims.as_ref().is_some_and(|d| !d.is_empty());
        if !is_delimited {
            self.consume_spaces()?;
        }
        let start = self.future_mut()?;
        let mut tok;
        let mut depth: isize = 0;
        let mut match_idx: usize = 0;
        loop {
            tok = self.pop_token()?;
            tokens.push(tok.clone());
            if tok.text == "{" {
                depth += 1;
            } else if tok.text == "}" {
                depth -= 1;
                if depth == -1 {
                    return Err(ParseError::with_token(
                        ParseErrorKind::ExtraCloseBrace,
                        &tok,
                    ));
                }
            } else if tok.text == "EOF" {
                let expected = delims.as_ref().map_or("}", |d| {
                    if is_delimited {
                        d.get(match_idx).map_or("}", String::as_str)
                    } else {
                        "}"
                    }
                });
                return Err(ParseError::with_token(
                    ParseErrorKind::UnexpectedEndOfMacroArgument {
                        expected: expected.to_owned(),
                    },
                    &tok,
                ));
            }
            if let Some(d) = &delims
                && is_delimited
            {
                if (depth == 0 || (depth == 1 && d[match_idx] == "{")) && tok.text == d[match_idx] {
                    match_idx += 1;
                    if match_idx == d.len() {
                        // don't include delimiters
                        let keep = tokens.len() - match_idx;
                        tokens.truncate(keep);
                        break;
                    }
                } else {
                    match_idx = 0;
                }
            }
            if depth == 0 && !is_delimited {
                // undelimited arg: stop after a single token or a {...} group
                if start.text != "{" || tok.text == "}" {
                    break;
                }
            }
            if depth == 0 && is_delimited {
                // keep going until delimiters matched
            }
        }

        // Remove outermost braces if present
        if start.text == "{" && tokens.last().map(|t| t.text.as_str()) == Some("}") {
            tokens.pop();
            if !tokens.is_empty() {
                tokens.remove(0);
            }
        }
        tokens.reverse();
        Ok(MacroArg {
            tokens,
            start,
            end: tok,
        })
    }

    fn consume_args(&mut self, num_args: usize) -> Result<Vec<Vec<Token>>, ParseError> {
        self.consume_args_with_delims(num_args, None)
    }

    fn is_defined(&self, name: &str) -> bool {
        if self.macros.has(name) {
            return true;
        }

        if self.ctx.functions.contains_key(name) {
            return true;
        }

        if IMPLICIT_COMMANDS.contains(name) {
            return true;
        }

        let symbols = &self.ctx.symbols;
        symbols.contains(Mode::Math, name) || symbols.contains(Mode::Text, name)
    }

    fn is_expandable(&self, name: &str) -> bool {
        if let Some(def) = self.macros.get(name) {
            match def {
                MacroDefinition::Expansion(e) => e.unexpandable != Some(true),
                _ => true,
            }
        } else {
            if let Some(md) = self.macros.get(name) {
                return match md {
                    MacroDefinition::Expansion(e) => !e.unexpandable.unwrap_or(false),
                    _ => true,
                };
            }

            self.ctx.functions.contains_key(name) && !self.ctx.functions[name].primitive
        }
    }

    fn begin_group(&mut self) {
        self.macros.begin_group();
    }

    fn end_group(&mut self) -> Result<(), ParseError> {
        self.macros.end_group()
    }
}

#[cfg(test)]
mod tests {
    use crate::KatexContext;
    use crate::parse_tree::parse_tree;
    use crate::types::{ParseErrorKind, Settings};

    #[test]
    fn errors_from_function_macros_propagate() {
        let ctx = KatexContext::default();
        let settings = Settings::default();
        let err = parse_tree(&ctx, r"\newcommand\lambda{x}\lambda", &settings)
            .expect_err("redefinition must fail");
        assert!(
            matches!(&*err.kind, ParseErrorKind::NewcommandRedefinition { name } if name == r"\lambda"),
            "{err}"
        );
        let err = parse_tree(
            &ctx,
            r"\tag{1}\tag{2}",
            &Settings::builder().display_mode(true).build(),
        )
        .expect_err("two tags must fail");
        assert!(matches!(&*err.kind, ParseErrorKind::MultipleTag), "{err}");
    }
}
