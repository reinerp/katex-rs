//! Nesting-depth limits: no input may exhaust the native stack.
//!
//! Every test renders on a thread with [`STACK`] bytes of stack. A stack
//! overflow aborts the whole test process, so these tests fail loudly if any
//! input recurses past the `max_nesting` budget. The 2 MiB figure applies to
//! optimized builds (`cargo test --release`), which need at most about
//! 0.6 MiB at the default limit; unoptimized builds use about five times as
//! much stack per level (2.9 MiB), so they get a 6 MiB thread.
#![allow(clippy::expect_used, clippy::unwrap_in_result)]

use std::thread;
use std::time::{Duration, Instant};

use katex::types::{ArgType, DEFAULT_MAX_NESTING, ParseErrorKind, TrustSetting};
use katex::{KatexContext, ParseError, Settings, render_to_string};

const STACK: usize = if cfg!(debug_assertions) {
    6 << 20
} else {
    2 << 20
};

/// Placeholder in nesting templates.
const SLOT: &str = "@@";

fn on_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    thread::Builder::new()
        .stack_size(STACK)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("render thread panicked")
}

fn settings(throw_on_error: bool) -> Settings {
    Settings::builder()
        .display_mode(true)
        .throw_on_error(throw_on_error)
        .trust(TrustSetting::Bool(true))
        .build()
}

fn nest(template: &str, depth: usize) -> String {
    let (open, close) = template.split_once(SLOT).expect("template has a slot");
    let mut s = open.repeat(depth);
    s.push('x');
    s.push_str(&close.repeat(depth));
    s
}

fn is_too_deep(result: &Result<String, ParseError>) -> bool {
    matches!(result, Err(e) if matches!(*e.kind, ParseErrorKind::TooDeeplyNested { .. }))
}

/// Renders `tex` both throwing and not throwing; returns the throwing result.
fn render_both(ctx: &KatexContext, tex: &str) -> Result<String, ParseError> {
    let thrown = render_to_string(ctx, tex, &settings(true));
    let html = render_to_string(ctx, tex, &settings(false)).expect("throw_on_error=false renders");
    // Without throwing, unknown commands render as text instead of failing,
    // but a depth error still becomes an error span.
    if is_too_deep(&thrown) {
        assert!(
            html.contains("katex-error"),
            "error span expected for {tex:.80}"
        );
    }
    thrown
}

/// The constructs from the stack-overflow evaluation, which aborted at
/// 160-470 levels on a 2 MiB thread before the limit existed.
const CONSTRUCTS: &[(&str, &str)] = &[
    ("braces", "{@@}"),
    ("frac", r"\frac{@@}{y}"),
    ("sqrt", r"\sqrt{@@}"),
    ("sup", "x^{@@}"),
    ("left", r"\left(@@\right)"),
    ("matrix", r"\begin{matrix}@@\end{matrix}"),
];

/// More templates for the random generator: mode switches, environments,
/// prefixes and commands whose handlers parse the rest of the group.
const TEMPLATES: &[&str] = &[
    "{@@}",
    r"\begingroup @@\endgroup ",
    r"\frac{@@}{y}",
    r"\sqrt{@@}",
    r"\sqrt[@@]{y}",
    "x^{@@}",
    "x_{@@}^{y}",
    r"\left(@@\right)",
    r"\left(@@\middle|y\right)",
    r"\begin{matrix}@@\end{matrix}",
    r"\begin{array}{cc}@@&y\\z&w\end{array}",
    r"\begin{aligned}@@&=y\end{aligned}",
    r"\text{a\(@@\)}",
    r"\text{\textbf{@@}}",
    r"\color{red}@@",
    r"\displaystyle @@",
    r"\Huge @@",
    r"\rm @@",
    r"\Braket{a|@@}",
    r"\overbrace{@@}^{y}",
    r"\stackrel{@@}{=}",
    r"\xrightarrow[@@]{y}",
    r"\operatorname{@@}",
    r"\mathchoice{@@}{y}{z}{w}",
    r"\href{a}{@@}",
    r"\fbox{@@}",
    r"\cancel{@@}",
    r"\phantom{@@}",
    r"\binom{@@}{y}",
    r"{@@\over y}",
    r"\widehat{@@}",
    r"\htmlClass{a}{@@}",
    r"\def\m{@@}\m",
    r"\expandafter{@@}",
    r"\noexpand\relax{@@}",
    r"\TextOrMath{@@}{@@}",
];

#[test]
fn constructs_render_below_the_limit_and_fail_above_it() {
    on_stack(|| {
        let ctx = KatexContext::default();
        for (name, template) in CONSTRUCTS {
            // Some constructs count two levels per nesting step (a command and
            // its braced argument), so half the limit always fits.
            let shallow = nest(template, DEFAULT_MAX_NESTING / 2);
            assert!(
                render_both(&ctx, &shallow).is_ok(),
                "{name} at half the limit"
            );
            for depth in [DEFAULT_MAX_NESTING + 1, 2_000] {
                let deep = nest(template, depth);
                let start = Instant::now();
                let result = render_both(&ctx, &deep);
                assert!(is_too_deep(&result), "{name} at {depth}: {result:?}");
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "{name} at {depth} is slow"
                );
            }
        }
    });
}

#[test]
fn max_nesting_is_configurable() {
    on_stack(|| {
        let ctx = KatexContext::default();
        let tex = nest("{@@}", 100);
        let with = |n: usize| {
            let s = Settings::builder().max_nesting(n).build();
            render_to_string(&ctx, &tex, &s)
        };
        assert!(with(100).is_ok());
        assert!(is_too_deep(&with(99)));
        assert!(is_too_deep(&with(0)));
        let flat = Settings::builder().max_nesting(0).build();
        assert!(render_to_string(&ctx, r"x+y=\alpha", &flat).is_ok());
    });
}

#[test]
fn macro_built_nesting_fails_cleanly() {
    on_stack(|| {
        let ctx = KatexContext::default();
        let amplified = |open: &str, close: &str, k: usize| {
            format!(r"\def\a{{{}\a{}}}\a", open.repeat(k), close.repeat(k))
        };
        let cases = [
            r"\def\a{{\a}}\a".to_owned(),
            r"\def\a{\frac{\a}{x}}\a".to_owned(),
            r"\newcommand\a{{\a}}\a".to_owned(),
            r"\def\a{\left(\a\right)}\a".to_owned(),
            r"\def\a{\expandafter\a\a}\a".to_owned(),
            r"\def\a{\edef\b{\a}}\a".to_owned(),
            amplified("{", "}", 10),
            amplified("{", "}", 100),
            amplified("{", "}", 4000),
            amplified(r"\frac{", "}{x}", 1000),
            format!(r"{}\def\a{{x}}\a", r"\global".repeat(10_000)),
            format!("{}x", r"\expandafter".repeat(10_000)),
        ];
        for tex in &cases {
            let start = Instant::now();
            let result = render_both(&ctx, tex);
            assert!(result.is_err(), "{tex:.60} should fail");
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "{tex:.60} is slow"
            );
        }
        // Without macro help the chains above recurse once per token.
        assert!(is_too_deep(&render_both(&ctx, &cases[10])));
        assert!(is_too_deep(&render_both(&ctx, &cases[11])));
        assert!(is_too_deep(&render_both(&ctx, &cases[8])));
    });
}

/// A nesting template for every command and environment: the slot goes in the
/// first argument that takes math or text, or after the command if it has
/// none (switches like `\bf` and `\color` that parse the rest of the group).
fn every_command_and_environment(ctx: &KatexContext) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut names: Vec<&String> = ctx.functions.keys().collect();
    names.sort();
    for name in names {
        let spec = &ctx.functions[name];
        if spec.infix {
            continue;
        }
        let mut args = String::new();
        let mut slot = false;
        for i in spec.num_optional_args..spec.num_optional_args + spec.num_args {
            let arg_type = spec.arg_types.as_ref().and_then(|v| v.get(i)).copied();
            args.push_str(match arg_type {
                Some(ArgType::Color) => "{red}",
                Some(ArgType::Size) => "{1em}",
                Some(ArgType::Url | ArgType::Raw) => "{a}",
                _ if !slot => {
                    slot = true;
                    "{@@}"
                }
                _ => "{y}",
            });
        }
        let template = if slot {
            format!("{name}{args}")
        } else {
            format!("{name}{args} @@")
        };
        let template = if spec.allowed_in_math {
            template
        } else {
            format!(r"\text{{{template}}}")
        };
        out.push((name.clone(), template));
    }
    let mut envs: Vec<&String> = ctx.environments.keys().collect();
    envs.sort();
    for env in envs {
        let args = "{c}".repeat(ctx.environments[env].num_args);
        out.push((
            env.clone(),
            format!(r"\begin{{{env}}}{args}@@\end{{{env}}}"),
        ));
    }
    out
}

#[test]
fn every_command_and_environment_nests_safely() {
    on_stack(|| {
        let ctx = KatexContext::default();
        let all = every_command_and_environment(&ctx);
        assert!(all.len() > 300, "only {} templates", all.len());
        let mut too_deep = 0;
        for (name, template) in &all {
            for depth in [DEFAULT_MAX_NESTING, DEFAULT_MAX_NESTING + 8] {
                let result = render_both(&ctx, &nest(template, depth));
                too_deep += usize::from(is_too_deep(&result));
                if depth > DEFAULT_MAX_NESTING
                    && let Ok(html) = &result
                {
                    // Accepted beyond the limit only if a level costs nothing,
                    // i.e. the command does not nest at all.
                    assert!(html.len() < 1 << 20, "{name}: {} bytes", html.len());
                }
            }
        }
        assert!(too_deep > all.len() / 2, "{too_deep} of {}", 2 * all.len());
    });
}

/// xorshift64*: a fixed-seed generator so failures reproduce.
struct Rng(u64);

impl Rng {
    const fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    const fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Random nesting of random constructs, sometimes with siblings around each
/// level and sometimes with a recursive macro, up to three times the limit.
fn random_input(rng: &mut Rng) -> String {
    let depth = rng.below(3 * DEFAULT_MAX_NESTING + 1);
    let mut s = String::from("x");
    for _ in 0..depth {
        let template = TEMPLATES[rng.below(TEMPLATES.len())];
        let (open, close) = template.split_once(SLOT).expect("slot");
        let close = close.replace(SLOT, "y");
        s = match rng.below(4) {
            0 => format!("a+{open}{s}{close}+b"),
            _ => format!("{open}{s}{close}"),
        };
    }
    if rng.below(8) == 0 {
        s = format!(r"\def\r{{{{{s}\r}}}}\r");
    }
    s
}

#[test]
fn random_nesting_never_overflows() {
    on_stack(|| {
        let ctx = KatexContext::default();
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let (mut ok, mut deep) = (0, 0);
        for _ in 0..1500 {
            let tex = random_input(&mut rng);
            let result = render_both(&ctx, &tex);
            ok += usize::from(result.is_ok());
            deep += usize::from(is_too_deep(&result));
        }
        // The generator must exercise both sides of the limit.
        assert!(ok > 100 && deep > 50, "ok {ok}, too deep {deep}");
    });
}
