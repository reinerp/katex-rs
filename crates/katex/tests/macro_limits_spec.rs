//! Defenses against macro definitions in untrusted input.
#![allow(clippy::expect_used, clippy::unwrap_in_result)]

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use katex::macro_expander::MacroMap;
use katex::macros::MacroDefinition;
use katex::types::ParseErrorKind;
use katex::{KatexContext, ParseError, Settings, render_to_string};

fn no_definitions() -> Settings {
    Settings::builder()
        .display_mode(true)
        .allow_macro_definitions(false)
        .build()
}

fn disallowed_command(result: &Result<String, ParseError>) -> Option<String> {
    match result {
        Err(e) => match &*e.kind {
            ParseErrorKind::MacroDefinitionsDisallowed { command } => Some(command.clone()),
            _ => None,
        },
        Ok(_) => None,
    }
}

#[test]
fn definition_commands_are_rejected_when_disabled() {
    let ctx = KatexContext::default();
    let cases = [
        (r"\def\a{x}\a", r"\def"),
        (r"\gdef\a{x}\a", r"\gdef"),
        (r"\edef\a{x}\a", r"\edef"),
        (r"\xdef\a{x}\a", r"\xdef"),
        (r"\let\a=x\a", r"\let"),
        (r"\futurelet\a\b\c", r"\futurelet"),
        (r"\global\def\a{x}\a", r"\global"),
        (r"\long\def\a{x}\a", r"\long"),
        (r"\newcommand\a{x}\a", r"\newcommand"),
        (r"\renewcommand\alpha{x}\alpha", r"\renewcommand"),
        (r"\providecommand\a{x}\a", r"\providecommand"),
        // Passed through built-in macros, the tokens are still the input's.
        (r"\TextOrMath{\def\a{x}}{\def\a{x}}\a", r"\def"),
        (r"\@firstoftwo{\newcommand\a{x}}{}\a", r"\newcommand"),
        (r"\expandafter\def\alpha{x}\alpha", r"\def"),
        (r"\Braket{\newcommand\a{x}}", r"\newcommand"),
        (r"x+\text{\gdef\a{x}}", r"\gdef"),
    ];
    for (tex, command) in cases {
        let result = render_to_string(&ctx, tex, &no_definitions());
        assert_eq!(
            disallowed_command(&result).as_deref(),
            Some(command),
            "{tex}: {result:?}"
        );
        // Allowed by default, as in KaTeX.
        let default = Settings::builder().display_mode(true).build();
        assert!(
            render_to_string(&ctx, tex, &default).is_ok() || command == r"\futurelet",
            "{tex}"
        );
        // Not throwing renders the usual error span.
        let quiet = Settings::builder()
            .allow_macro_definitions(false)
            .throw_on_error(false)
            .build();
        let html = render_to_string(&ctx, tex, &quiet).expect("renders");
        assert!(
            html.contains("katex-error") && html.contains("Macro definitions are disabled"),
            "{html}"
        );
    }
}

#[test]
fn katex_and_host_macros_still_work_when_disabled() {
    let ctx = KatexContext::default();
    // \tag and \nonumber are implemented with \gdef.
    for tex in [
        r"x\tag{1}",
        r"x\tag*{a}",
        r"\begin{align}a&=b\nonumber\\c&=d\end{align}",
        r"\begin{equation}x\end{equation}",
        r"\Set{x|x>0}",
        r"\char`a\neq\not\in",
    ] {
        let allowed = render_to_string(&ctx, tex, &Settings::builder().display_mode(true).build());
        let disabled = render_to_string(&ctx, tex, &no_definitions());
        assert!(disabled.is_ok(), "{tex}: {disabled:?}");
        assert_eq!(disabled.ok(), allowed.ok(), "{tex}");
    }
    // Host-supplied macros are trusted, even if they define macros.
    let mut macros = MacroMap::default();
    macros.insert(
        r"\host".to_owned(),
        MacroDefinition::String(r"\def\inner{y}\inner".to_owned()),
    );
    let settings = Settings::builder()
        .allow_macro_definitions(false)
        .macros(macros)
        .build();
    assert!(render_to_string(&ctx, r"x+\host", &settings).is_ok());
}

/// Pathological macro inputs from the evaluation (patho_misc.jsonl and
/// patho_amp2.jsonl): with definitions disabled each fails at once.
#[test]
fn pathological_macros_fail_fast_when_disabled() {
    let ctx = KatexContext::default();
    let big = "x".repeat(15_000);
    let names: Vec<String> = (0..31)
        .map(|i| {
            format!(
                r"\m{}{}",
                char::from(b'a' + i / 26),
                char::from(b'a' + i % 26)
            )
        })
        .collect();
    let mut doubling = format!(r"\def{}{{x}}", names[0]);
    for i in 1..31 {
        write!(
            doubling,
            r"\def{}{{{}{}}}",
            names[i],
            names[i - 1],
            names[i - 1]
        )
        .expect("write to String");
    }
    doubling.push_str(&names[30]);
    let cases = [
        r"\def\a{\a}\a".to_owned(),
        r"\def\a{{\a}}\a".to_owned(),
        r"\def\a{\frac{\a}{x}}\a".to_owned(),
        doubling,
        format!(r"\def\d#1{{#1#1#1#1#1#1#1#1}}\d{{\d{{\d{{\d{{{big}}}}}}}}}"),
        format!(r"\def\d#1{{#1#1#1#1#1#1#1#1}}\d{{\d{{\d{{{big}}}}}}}"),
        format!(r"\def\a{{{}\a{}}}\a", "{".repeat(4000), "}".repeat(4000)),
        format!(
            r"\def\a{{{}\a{}}}\a",
            r"\frac{".repeat(1000),
            "}{x}".repeat(1000)
        ),
    ];
    for tex in &cases {
        let start = Instant::now();
        let result = render_to_string(&ctx, tex, &no_definitions());
        assert_eq!(
            disallowed_command(&result).as_deref(),
            Some(r"\def"),
            "{tex:.60}"
        );
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "{tex:.60}: {:?}",
            start.elapsed()
        );
    }
}

fn too_many_tokens(result: &Result<String, ParseError>) -> bool {
    matches!(result, Err(e) if matches!(*e.kind, ParseErrorKind::TooManyExpandedTokens { .. }))
}

/// With definitions allowed (the default), argument-copying macros hit the
/// expanded-token budget instead of multiplying the input: in the evaluation,
/// three levels took 13 s and produced 84 MB, and four ran out of memory.
#[test]
fn argument_copying_fails_fast_with_definitions_allowed() {
    let ctx = KatexContext::default();
    let settings = Settings::default();
    let big = "x".repeat(15_000);
    let cases = [
        format!(r"\def\d#1{{#1#1#1#1#1#1#1#1}}\d{{\d{{\d{{{big}}}}}}}"),
        format!(r"\def\d#1{{#1#1#1#1#1#1#1#1}}\d{{\d{{\d{{\d{{{big}}}}}}}}}"),
        // One expansion copying a long argument 5,000 times must fail before
        // building the 75-million-token result.
        format!(r"\def\d#1{{{}}}\d{{{big}}}", "#1".repeat(5_000)),
        format!(
            r"\newcommand\d[1]{{#1#1#1#1}}\d{{\d{{\d{{\d{{\d{{\d{{\d{{\d{{\d{{{big}}}}}}}}}}}}}}}}}}}"
        ),
    ];
    for tex in &cases {
        let start = Instant::now();
        let result = render_to_string(&ctx, tex, &settings);
        assert!(too_many_tokens(&result), "{tex:.60}: {result:?}");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{tex:.60}: {:?}",
            start.elapsed()
        );
    }
}

#[test]
fn max_expanded_tokens_is_configurable() {
    let ctx = KatexContext::default();
    let with = |n: usize| {
        render_to_string(
            &ctx,
            r"\def\a{abcdefghijk}\a",
            &Settings::builder().max_expanded_tokens(n).build(),
        )
    };
    assert!(with(11).is_ok());
    assert!(too_many_tokens(&with(10)));
    // Built-in macros count too.
    let builtin = Settings::builder().max_expanded_tokens(0).build();
    assert!(too_many_tokens(&render_to_string(&ctx, r"a\;b", &builtin)));
    assert!(render_to_string(&ctx, "a+b", &builtin).is_ok());
}

#[test]
fn bad_parameter_numbers_are_errors() {
    let ctx = KatexContext::default();
    // These used to index past the arguments and panic.
    for tex in [
        r"\def\a#1{#2}\a x",
        r"\def\a#1{#0}\a x",
        r"\def\a#1#2{#1#3}\a xy",
    ] {
        let result = render_to_string(&ctx, tex, &Settings::default());
        assert!(
            matches!(&result, Err(e) if matches!(*e.kind, ParseErrorKind::InvalidMacroArgumentNumber { .. })),
            "{tex}: {result:?}"
        );
    }
    assert!(render_to_string(&ctx, r"\def\a#1#2{#2#1}\a xy", &Settings::default()).is_ok());
}
