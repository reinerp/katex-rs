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

/// A formula of glyphs only is one element with one text node.
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
        assert!(out.starts_with(r#"<span class="katex mf"#), "{tex}: {out}");
        assert_eq!(out.matches("<span").count(), 1, "{tex}: {out}");
        assert!(
            out.chars().filter(|c| private(*c)).count() >= tex.len() / 2,
            "{tex}: {out}"
        );
    }
}

/// What the font cannot write stays markup, and the glyphs around it are runs.
#[test]
fn mixed_formulas_have_text_and_elements() {
    let out = font_run(r"a + \frac{x^2}{y} = \mathbf{b} + c", false);
    assert!(out.contains(r#"class="mathbf""#), "{out}");
    assert!(out.contains("mfrac"), "{out}");
    assert!(out.chars().filter(|c| private(*c)).count() > 8, "{out}");
    // bold b is the stock character, in a span of its own
    assert!(out.contains(">b</span>"), "{out}");
}

/// `\text` stays stock text (selectable, readable), with runs around it.
#[test]
fn text_mode_stays_text() {
    let out = font_run(r"a + \text{hello world} = b", false);
    assert!(out.contains("hello world"), "{out}");
    assert!(out.chars().filter(|c| private(*c)).count() > 4, "{out}");
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

/// Short formulas have no line break opportunities, long ones have a `<wbr>` between bases, as in the compact output.
#[test]
fn line_breaks_are_those_of_the_compact_output() {
    assert!(!font_run(r"a+b=c", false).contains("<wbr>"));
    let long = font_run(r"a+b=c+d=e+f=g", false);
    assert!(long.contains("<wbr>"), "{long}");
    assert!(!font_run(r"a+b=c+d=e+f=g", true).contains("<wbr>"));
    // the number of break opportunities does not depend on the glyphs having become code points
    assert_eq!(
        long.matches("<wbr>").count(),
        render(r"a+b=c+d=e+f=g", false, levels::ALL)
            .matches("<wbr>")
            .count()
    );
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
