//! Glyph runs in the generated font (`compact::levels::FONT_RUN`, `src/mathrun.rs`).

use katex::compact::{self, levels};
use katex::mathrun;
use katex::types::OutputFormat;
use katex::{KatexContext, Settings, render_to_string};

fn render(tex: &str, display: bool, levels: u32) -> String {
    let settings = Settings::builder()
        .display_mode(display)
        .throw_on_error(false)
        .output(OutputFormat::CompactHtml)
        .compact_levels(levels)
        .build();
    render_to_string(&KatexContext::default(), tex, &settings).expect("render")
}

fn font_run(tex: &str, display: bool) -> String {
    render(tex, display, levels::ALL | levels::FONT_RUN)
}

/// Whether `c` is one of the generated font's private-use code points.
fn private(c: char) -> bool {
    matches!(c, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{FFFFD}')
}

#[test]
fn flag_is_off_by_default() {
    assert_eq!(levels::ALL & levels::FONT_RUN, 0);
    for tex in [r"x_i^2 + a = b", r"\frac{a}{b}", r"\mathbf{x} + y"] {
        let out = render(tex, false, levels::ALL);
        assert!(!out.chars().any(private), "{tex}: {out}");
        assert!(!out.contains(" mf"), "{tex}: {out}");
    }
}

/// A formula of glyphs only is one element with one text node (and the element of the strut where it wraps).
#[test]
fn a_formula_of_glyphs_is_one_run() {
    for tex in [
        r"a+b+c=d",
        r"x_i",
        r"x^2",
        r"f(x)=\alpha x^2+\beta",
        r"e^{i\pi}+1=0",
    ] {
        let out = font_run(tex, false);
        assert!(
            out.starts_with(r#"<span class="katex m"#) && out.contains(" mf"),
            "{tex}: {out}"
        );
        // one element, or two for a formula that wraps: the element of its strut, after its first segment
        let spans = out.matches("<span").count();
        assert!(
            spans == 1 || (spans == 2 && out.contains(r#"<span class="mu"></span>"#)),
            "{tex}: {out}"
        );
        assert!(out.chars().any(private), "{tex}: {out}");
    }
}

/// Spacing is in the glyphs: `a+b+c=d` is its seven glyphs and no spacer between them.
#[test]
fn spacing_is_baked_into_the_glyphs() {
    let out = font_run(r"a+b+c=d", false);
    let text: Vec<char> = out.chars().filter(|c| private(*c)).collect();
    assert_eq!(text.len(), 7, "{out}");
    // a script is its glyphs and the script space is one spacer at most
    let out = font_run(r"x^2", false);
    assert!(out.chars().filter(|c| private(*c)).count() <= 2, "{out}");
}

/// What the font cannot write stays markup, and the glyphs around it are runs.
#[test]
fn mixed_formulas_have_text_and_elements() {
    let out = font_run(r"a + \frac{x^2}{y} = \mathbf{b} + c", false);
    assert!(out.contains(r#"class="mathbf""#), "{out}");
    assert!(out.contains("mfrac"), "{out}");
    assert!(out.chars().filter(|c| private(*c)).count() >= 4, "{out}");
    // bold b is the stock character, in a span of its own
    assert!(out.contains(">b</span>"), "{out}");
}

/// `\text` stays stock text (selectable, readable), with runs around it.
#[test]
fn text_mode_stays_text() {
    let out = font_run(r"a + \text{hello world} = b", false);
    assert!(out.contains("hello world"), "{out}");
    assert!(out.chars().filter(|c| private(*c)).count() >= 2, "{out}");
}

#[test]
fn display_keeps_its_wrapper() {
    let out = font_run(r"a + b", true);
    assert!(
        out.starts_with(r#"<span class="katex-display"><span class="katex mf"#),
        "{out}"
    );
}

#[test]
fn label_and_strut_settings_are_those_of_the_compact_output() {
    let tex = r"x_i^2 + a = b";
    let out = font_run(tex, false);
    assert!(
        out.contains(r#"role="math" aria-label="x_i^2 + a = b""#),
        "{out}"
    );
    assert!(
        out.contains(r#" class="katex mf "#)
            || out.contains(r#"class="katex mf t"#)
            || out.contains(r#"class="katex mf f""#),
        "{out}"
    );
    // a host line with room for the formula: no strut class
    let settings = Settings::builder()
        .throw_on_error(false)
        .output(OutputFormat::CompactHtml)
        .compact_levels(levels::ALL | levels::FONT_RUN)
        .compact_line((2.0, 1.0))
        .compact_label(false)
        .build();
    let out = render_to_string(&KatexContext::default(), tex, &settings).expect("render");
    assert!(
        out.starts_with(r#"<span class="katex mf w" aria-hidden="true">"#),
        "{out}"
    );
}

/// Short formulas have no line break opportunities. A long one that is text and inline spans has a zero-width space between its
/// bases and wraps there (`white-space: normal`, the class `mw`); one with a stack, or stock text a browser may break, keeps the
/// `<wbr>` of the compact output and `white-space: nowrap`.
#[test]
fn line_breaks_are_those_of_the_compact_output() {
    let breaks = |out: &str| {
        (
            out.matches('\u{200b}').count(),
            out.matches("<wbr>").count(),
        )
    };
    assert_eq!(breaks(&font_run(r"a+b=c", false)), (0, 0));
    let long = font_run(r"a+b=c+d=e+f=g", false);
    assert!(
        long.contains(r#"class="katex mw mf"#) || long.contains(" mw "),
        "{long}"
    );
    assert!(breaks(&long).0 >= 2 && breaks(&long).1 == 0, "{long}");
    assert_eq!(breaks(&font_run(r"a+b=c+d=e+f=g", true)), (0, 0));
    // the same number of break opportunities as the compact output, and none on the other side of a space: no `<wbr>` is a loss
    let flat = render(r"a+b=c+d=e+f=g", false, levels::ALL);
    assert_eq!(breaks(&long).0, flat.matches("<wbr>").count());
    // a stack: the inline block would allow breaks of its own, so `<wbr>` and `nowrap`
    let stack = font_run(r"a+b=c+\frac{d}{e}+f=g", false);
    assert_eq!(breaks(&stack).0, 0, "{stack}");
    assert!(breaks(&stack).1 >= 2 && !stack.contains(" mw"), "{stack}");
    // a hyphen in stock text: a browser may break after it
    let text = font_run(r"a+b=c+\text{non-zero}+d=e+f", false);
    assert_eq!(breaks(&text).0, 0, "{text}");
    assert!(breaks(&text).1 >= 1, "{text}");
    // spaces of stock text are no-break spaces in a formula that wraps at zero-width spaces
    let words = font_run(r"a+b=c+\text{two words}+d=e+f", false);
    assert!(words.contains("two\u{a0}words"), "{words}");
}

#[test]
fn the_text_of_a_run_has_only_the_font_and_spacers() {
    let out = font_run(r"x_i^2 + a = b", false);
    let body = out
        .rsplit_once(r#"">"#)
        .map(|(_, b)| b)
        .or_else(|| out.rsplit_once("\">").map(|(_, b)| b));
    let body = body.expect("text");
    let text = body.trim_end_matches("</span>");
    assert!(
        text.chars().all(private) || text.contains("<wbr>"),
        "{text:?}"
    );
}

#[test]
fn accessors() {
    assert!(mathrun::FONT_WOFF2.starts_with(b"wOF2"));
    assert!(mathrun::FONT_FILE_NAME.ends_with(".woff2"));
    assert!(mathrun::FONT_FILE_NAME.contains(mathrun::FONT_VERSION));
    assert!(mathrun::FONT_FAMILY.contains(mathrun::FONT_VERSION));
    let css = mathrun::stylesheet("fonts/x.woff2", None);
    assert!(css.starts_with(compact::stylesheet()));
    assert!(css.contains(&format!("font-family:{}", mathrun::FONT_FAMILY)));
    assert!(css.contains(".katex.mf{font-family:"));
    assert!(css.contains("text-rendering:geometricPrecision"));
    // the classes that set a font family keep the generated font first
    assert!(css.contains(&format!(
        ".katex.mf .mathbf{{font-family:{},KaTeX_Main",
        mathrun::FONT_FAMILY
    )));
    let with_blank = mathrun::stylesheet("a.woff2", Some("b.woff2"));
    assert!(with_blank.contains("unicode-range:U+E000-F8FF"));
}
