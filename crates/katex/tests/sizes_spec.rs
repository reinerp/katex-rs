//! Sizes computed from the tree and the font metrics (`Settings::compact_sizes`, `src/sizes.rs`).

use katex::compact::{self, levels};
use katex::sizes::{self, FormulaSizes};
use katex::types::OutputFormat;
use katex::{KatexContext, Settings, render_to_string};

const FORMULAS: &[&str] = &[
    r"a+b+c=d",
    r"x_i^2",
    r"\frac{a}{b}",
    r"\sqrt{x}+1",
    r"\sum_{i=0}^{n} i",
    r"\begin{pmatrix}a&b\\c&d\end{pmatrix}",
    r"\mathbf{x}\cdot\text{hello world} = \left(\frac{1}{2}\right)^2",
    r"\boxed{x}\xrightarrow{f}y",
];

fn render(tex: &str, display: bool, font: bool, sizes: bool) -> String {
    let lv = if font {
        levels::ALL | levels::FONT_RUN
    } else {
        levels::ALL
    };
    let settings = Settings::builder()
        .display_mode(display)
        .throw_on_error(false)
        .output(OutputFormat::CompactHtml)
        .compact_levels(lv)
        .compact_sizes(sizes)
        .build();
    render_to_string(&KatexContext::default(), tex, &settings).expect("render")
}

/// The `width` that the option adds to stacks (`X` and `Y`), removed (a width of exactly .5em is the class `w0`).
fn strip_widths(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(i) = rest.find("<span class=\"") {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = tail.find('>').expect("tag end");
        let mut tag = tail[..end].to_owned();
        let stack = tag.starts_with("<span class=\"X") || tag.starts_with("<span class=\"Y");
        if stack {
            while let Some(p) = tag.find(";width:") {
                let q = tag[p + 1..]
                    .find([';', '"'])
                    .map_or(tag.len(), |n| n + p + 1);
                tag.replace_range(p..q, "");
            }
            if let Some(p) = tag.find(" style=\"width:") {
                let q = tag[p + 8..].find('"').map_or(tag.len(), |n| n + p + 8) + 1;
                tag.replace_range(p..q, "");
            }
            tag = tag.replace(" w0\"", "\"").replace(" w0 ", " ");
        }
        out.push_str(&tag);
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn off_by_default() {
    assert!(!Settings::builder().build().compact_sizes);
    for tex in FORMULAS {
        for font in [false, true] {
            let out = render(tex, false, font, false);
            assert!(!out.contains("--sz"), "{tex}: {out}");
            assert!(!out.contains(";width:"), "{tex}: {out}");
            assert!(compact::take_sizes().is_none(), "{tex}");
        }
    }
}

/// With the option on, only widths are added: without them the markup is the markup without the option.
#[test]
fn only_widths_are_added() {
    for tex in FORMULAS {
        for display in [false, true] {
            for font in [false, true] {
                let off = render(tex, display, font, false);
                let on = render(tex, display, font, true);
                assert!(!on.contains("--sz"), "{tex}: {on}");
                assert_eq!(
                    strip_widths(&on),
                    off,
                    "{tex} display={display} font={font}"
                );
            }
        }
    }
}

fn sizes_of(tex: &str, display: bool, font: bool) -> FormulaSizes {
    let _ = render(tex, display, font, true);
    compact::take_sizes().expect("sizes")
}

#[test]
fn a_run_is_the_sum_of_its_advances() {
    // a, +, b, +, c, =, d: letters 0.5, 0.4306 (italic b: 0.4290), ... plus 3mu and 4mu around the operators: the total is the
    // advance of the whole run, which `mathrun` keeps as the width of the node
    let z = sizes_of("a+b+c=d", false, true);
    let w = z.width.expect("width");
    assert!((w - 5.6894).abs() < 0.001, "{w}");
    // KaTeX's height and depth of the strut
    assert!(
        (z.height - 0.6945).abs() < 1e-9 && (z.depth - 0.0833).abs() < 1e-9,
        "{z:?}"
    );
    // the same formula without the font: symbols and margins, the same sum
    let z2 = sizes_of("a+b+c=d", false, false);
    assert!((z2.width.expect("width") - w).abs() < 0.002, "{z2:?}");
}

#[test]
fn stacks_get_their_width() {
    let out = render(r"\frac{a}{b}", false, true, true);
    let i = out.find("class=\"X").expect("a stack");
    let tag = &out[i..out[i..].find('>').map(|n| n + i).expect("tag end")];
    let w = tag
        .split("width:")
        .nth(1)
        .and_then(|s| s.split(['"', ';']).next())
        .expect("width");
    let w: f64 = w.strip_suffix("em").expect("em").parse().expect("number");
    // the wider of a and b at 0.7 of the font (0.7 * 0.5, with the italic correction of b)
    assert!((0.3..0.45).contains(&w), "{tag}");
    let z = sizes_of(r"\frac{a}{b}", false, true);
    assert_eq!((z.stacks, z.stacks_known), (1, 1));
}

#[test]
fn a_stack_with_a_merged_item_has_the_width_without_its_margins() {
    // `x_i` after a symbol that is not in the font: a stack of one item that is not plain, merged into one element (`Y`),
    // whose margins are outside it
    for tex in [r"\sum_{r' \ne r} a", r"\mathbf{x}_{\mathbf{i}}"] {
        let on = render(tex, false, true, true);
        let off = render(tex, false, true, false);
        assert_eq!(strip_widths(&on), off, "{tex}");
    }
}

#[test]
fn what_cannot_be_derived_is_unknown() {
    // a vertical separator of an array is 1px wide at least: not a length in em, so the width of the formula is unknown (the
    // columns are stacks of their own, and known)
    let z = sizes_of(r"\begin{array}{c|c} a & b \end{array}", false, true);
    assert_eq!(z.width, None, "{z:?}");
    assert!(
        z.unknown.iter().any(|u| u.contains("vertical-separator")),
        "{z:?}"
    );
}

#[test]
fn extents_follow_the_line_box() {
    // At 18.15 px (a 15 px text, the formula's font is 1.21 times) Blink's line box of the formula's font is 16 px above the
    // baseline and 5.78125 below.
    let (above, below) = sizes::line_box(18.15);
    assert_eq!((above, below), (16.0, 5.78125));
    let z = sizes_of("a+b", false, true);
    let (a, b, la) = z.extents(18.15).expect("extents");
    assert!(
        (a * 18.15 - 16.0).abs() < 1e-9
            && (b * 18.15 - 5.78125).abs() < 1e-9
            && (la * 18.15 - 16.0).abs() < 1e-9
    );
    // a tall formula: the strut sets the extent
    let z = sizes_of(r"\frac{a}{b}", false, true);
    let (a, b, _) = z.extents(18.15).expect("extents");
    assert!((b - 0.345).abs() < 1e-9 && a * 18.15 >= 16.0, "{a} {b}");
}
