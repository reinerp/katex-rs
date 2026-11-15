//! The compact output (`OutputFormat::CompactHtml`, `src/compact.rs`).

use katex::compact::{self, levels};
use katex::types::{OutputFormat, TrustSetting};
use katex::{KatexContext, Settings, render_to_string};
use serde::Deserialize;

const FORMULAS: &[(&str, bool)] = &[
    (r"s_{i+1} = f_i(s_i)", false),
    (r"x^2 + y^2 = z^2", false),
    (r"\frac{a+b}{c} + \sqrt{x^2+1} + \sqrt[3]{y}", false),
    (r"\sum_{i=0}^{n} \frac{x_i}{i!}", true),
    (r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}^{-1}", true),
    (
        r"\begin{aligned} f(x) &= (x+1)^2 \\ &= x^2 + 2x + 1 \end{aligned}",
        true,
    ),
    (r"\boxed{c=\prod_{i=1}^{n}c_i}", false),
    (r"\text{if } x \ge 0,\ \mathbf{x}_k \mathrm{d}t", false),
    (
        r"\color{blue}{\hat{x}} + \vec{v} \cdot \overline{AB}",
        false,
    ),
    (r"\left( \frac{1}{2} \right)^2 \bar{\alpha}_{i,j}", false),
    (
        r"\xrightarrow[under]{over} \underbrace{a+b}_{c} \cancel{x}",
        true,
    ),
    (
        r"\bigcup_{k} A_k \qquad \forall k,\, \widetilde{F}(y)",
        true,
    ),
    (r"\newline \tag{1} a=b", true),
    (r"\foo", false),
];

fn settings(display: bool, output: OutputFormat) -> Settings {
    Settings::builder()
        .display_mode(display)
        .throw_on_error(false)
        .trust(TrustSetting::Bool(false))
        .max_expand(1000)
        .max_size(100.0)
        .output(output)
        .build()
}

fn compact(tex: &str, display: bool, levels: u32, label: bool) -> String {
    let settings = Settings::builder()
        .display_mode(display)
        .throw_on_error(false)
        .output(OutputFormat::CompactHtml)
        .compact_levels(levels)
        .compact_label(label)
        .build();
    render_to_string(&KatexContext::default(), tex, &settings).expect("render")
}

#[derive(Deserialize)]
struct Golden {
    tex: String,
    display: bool,
    html: String,
}

/// The KaTeX-compatible output is what it was before the compact one was added.
#[test]
fn katex_compatible_output_is_unchanged() {
    let ctx = KatexContext::default();
    let golden: Vec<Golden> =
        serde_json::from_str(include_str!("fixtures/legacy_html_golden.json"))
            .expect("golden file");
    assert!(golden.len() > 10);
    for g in golden {
        let got = render_to_string(
            &ctx,
            &g.tex,
            &settings(g.display, OutputFormat::HtmlAndMathml),
        )
        .expect("render");
        assert_eq!(got, g.html, "{}", g.tex);
    }
}

/// With every step off the compact output is the HTML output of KaTeX, so the conversion into
/// the compact tree and back loses nothing.
#[test]
fn no_steps_is_html_output() {
    let ctx = KatexContext::default();
    for &(tex, display) in FORMULAS {
        let html =
            render_to_string(&ctx, tex, &settings(display, OutputFormat::Html)).expect("html");
        assert_eq!(compact(tex, display, 0, false), html, "{tex}");
    }
}

fn classes(markup: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = markup;
    while let Some(at) = rest.find(" class=\"") {
        rest = &rest[at + 8..];
        let end = rest.find('"').expect("closing quote");
        out.extend(rest[..end].split(' ').map(str::to_owned));
        rest = &rest[end..];
    }
    out
}

/// Every class the compact output uses has a rule in the stylesheet that ships with it.
#[test]
fn every_class_has_a_rule() {
    let css = compact::stylesheet();
    let known = |class: &str| {
        let needle = format!(".{class}");
        css.match_indices(&needle).any(|(at, _)| {
            !css[at + needle.len()..]
                .starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
    };
    // Classes KaTeX's own CSS has no rule for, which consumers may style.
    let free = [
        "katex-error",
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
    for &(tex, display) in FORMULAS {
        for levels in [levels::ALL, levels::EXACT, 0] {
            for class in classes(&compact(tex, display, levels, true)) {
                assert!(
                    free.contains(&class.as_str()) || known(&class),
                    "{tex}: .{class} ({levels})"
                );
            }
        }
    }
}

fn text_of(markup: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut in_attr = None;
    for c in markup.chars() {
        match (in_tag, in_attr, c) {
            (false, _, '<') => in_tag = true,
            (true, None, '>') => in_tag = false,
            (true, None, '"') => in_attr = Some('"'),
            (true, Some(q), c) if c == q => in_attr = None,
            (false, _, c) => out.push(c),
            _ => {}
        }
    }
    out.replace('\u{200b}', "")
}

/// The text of a formula (what selecting and copying it gives) is that of KaTeX's HTML branch.
#[test]
fn text_is_that_of_the_html_branch() {
    let ctx = KatexContext::default();
    for &(tex, display) in FORMULAS {
        let html =
            render_to_string(&ctx, tex, &settings(display, OutputFormat::Html)).expect("html");
        let want = text_of(&html);
        for levels in [levels::ALL, levels::EXACT] {
            assert_eq!(
                text_of(&compact(tex, display, levels, false)),
                want,
                "{tex} ({levels})"
            );
        }
    }
}

#[test]
fn label_and_roles() {
    let with = compact(r"a<b", false, levels::ALL, true);
    assert!(
        with.starts_with(r#"<span class="katex" role="math" aria-label="a&lt;b">"#),
        "{with}"
    );
    assert!(!with.contains("<math"));
    let without = compact(r"a<b", false, levels::ALL, false);
    assert!(
        without.starts_with(r#"<span class="katex" aria-hidden="true">"#),
        "{without}"
    );
    let display = compact(r"a", true, levels::ALL, true);
    assert!(display.starts_with(r#"<span class="katex-display"><span class="katex" role="math""#));
}

#[test]
fn compact_is_much_smaller() {
    let ctx = KatexContext::default();
    let mut before = 0;
    let mut after = 0;
    for &(tex, display) in FORMULAS {
        before += render_to_string(&ctx, tex, &settings(display, OutputFormat::HtmlAndMathml))
            .expect("render")
            .len();
        after += compact(tex, display, levels::ALL, true).len();
    }
    assert!(after * 2 < before, "{after} bytes against {before}");
}

#[test]
fn levels_are_deterministic_and_total() {
    // Every single step alone, and all but one, renders every formula.
    for &(tex, display) in FORMULAS {
        let all = compact(tex, display, levels::ALL, true);
        assert_eq!(all, compact(tex, display, levels::ALL, true));
        for bit in 0..17 {
            let one = compact(tex, display, 1 << bit, true);
            let but = compact(tex, display, levels::ALL & !(1 << bit), true);
            assert!(!one.is_empty() && !but.is_empty());
        }
    }
}

#[test]
fn precision_keeps_extents_and_rounds_offsets() {
    let render = |sf, levels| {
        let settings = Settings::builder()
            .output(OutputFormat::CompactHtml)
            .compact_levels(levels)
            .compact_precision(sf)
            .compact_label(false)
            .build();
        render_to_string(
            &KatexContext::default(),
            r"x_{ab}^{2} \frac{1}{3} = y",
            &settings,
        )
        .expect("render")
    };
    let exact = render(0, levels::ALL & !levels::VALUE_CLASSES & !levels::COMBO);
    let rounded = render(2, levels::ALL & !levels::VALUE_CLASSES & !levels::COMBO);
    assert_ne!(exact, rounded);
    // The line-height-related extents are left alone unless asked for.
    let heights = |s: &str| {
        s.match_indices("height:")
            .map(|(i, _)| s[i..i + 14].to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(heights(&exact), heights(&rounded));
    let all = render(
        2,
        (levels::ALL | levels::ROUND_EXTENTS) & !levels::VALUE_CLASSES & !levels::COMBO,
    );
    assert_ne!(heights(&exact), heights(&all));
    // A small non-zero value stays non-zero.
    assert!(!rounded.contains("margin-right:0em") && !rounded.contains("margin-right:0;"));
}

#[test]
fn short_tags_are_an_option() {
    let plain = compact(r"x_i + \frac{a}{b}", false, levels::ALL, true);
    assert!(plain.contains("<span") && !plain.contains("<q"));
    let short = compact(
        r"x_i + \frac{a}{b}",
        false,
        levels::ALL | levels::SHORT_TAGS,
        true,
    );
    assert!(short.starts_with("<q class=\"katex\""), "{short}");
    assert!(!short.contains("<span") && !short.contains("</span>"));
    assert_eq!(
        short.replace("<q", "<span").replace("</q>", "</span>"),
        plain
    );
}

#[test]
fn output_size_limit_applies() {
    let settings = Settings::builder()
        .output(OutputFormat::CompactHtml)
        .max_output_size(300)
        .throw_on_error(true)
        .build();
    let long = "x_{a}".repeat(40);
    assert!(render_to_string(&KatexContext::default(), &long, &settings).is_err());
    let settings = Settings::builder()
        .output(OutputFormat::CompactHtml)
        .max_output_size(300)
        .throw_on_error(false)
        .build();
    let out = render_to_string(&KatexContext::default(), &long, &settings).expect("render");
    assert!(out.contains("katex-error"), "{out}");
}

#[test]
fn deeply_nested_input_renders() {
    let nested = format!("{}x{}", r"\sqrt{".repeat(12), "}".repeat(12));
    let out = compact(&nested, false, levels::ALL, true);
    assert!(out.starts_with("<span class=\"katex\""), "{out}");
}

/// Rewriting the tree is recursive but bounded: the deepest formula the nesting limit allows
/// renders on a stack of a size the hardened renderer is known to need, also in compact form.
#[test]
fn deepest_allowed_formula_fits_the_workers_stack() {
    let stack = if cfg!(debug_assertions) {
        16 << 20
    } else {
        1 << 20
    };
    let result = std::thread::Builder::new()
        .stack_size(stack)
        .spawn(|| {
            let ctx = KatexContext::default();
            let tex = format!("{}x{}", r"\overset{a}{".repeat(15), "}".repeat(15));
            let settings = |output| {
                Settings::builder()
                    .output(output)
                    .throw_on_error(false)
                    .build()
            };
            let html =
                render_to_string(&ctx, &tex, &settings(OutputFormat::HtmlAndMathml)).expect("html");
            let compact = render_to_string(&ctx, &tex, &settings(OutputFormat::CompactHtml))
                .expect("compact");
            (html.len(), compact.len())
        })
        .expect("spawn")
        .join();
    let (html, compact) = result.expect("rendered without overflowing the stack");
    assert!(compact < html);
}
