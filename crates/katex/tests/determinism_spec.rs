//! Output must not depend on hash-map iteration order.
#![allow(clippy::expect_used)]

use katex::types::TrustSetting;
use katex::{KatexContext, Settings, render_to_string};

/// Formulas whose output has elements with several attributes: SVG
/// delimiters and stretchy arrows, links, data attributes, images, MathML
/// operators, tables and rules.
const FORMULAS: &[&str] = &[
    r"\sqrt{x}",
    r"\sqrt[3]{\frac{a}{b}}",
    r"\left(\frac{a}{b}\right)",
    r"\Bigg\{x\Bigg\}",
    r"\overrightarrow{AB}+\widehat{abc}",
    r"\xrightarrow[under]{over}",
    r"\overbrace{a+b}^{c}\underbrace{d}_{e}",
    r"\cancel{x}\bcancel{y}\xcancel{z}",
    r"\boxed{x}\fbox{y}\colorbox{red}{z}\fcolorbox{red}{blue}{w}",
    r"\href{https://example.com}{x}\url{https://example.com}",
    r"\htmlData{a=1, b=2, c=3, d=4}{x}\htmlId{i}{y}\htmlClass{c}{z}\htmlStyle{color:red}{w}",
    r"\includegraphics[height=1em, totalheight=2em, width=3em, alt=a]{https://example.com/a.png}",
    r"\rule{1em}{2em}\rule[-1em]{1em}{2em}",
    r"\begin{array}{c|c}a&b\\\hline c&d\end{array}",
    r"\begin{pmatrix}1&2\\3&4\end{pmatrix}",
    r"\sum_{i=1}^{n}\int_0^1 \prod_k \lim_{x\to0}",
    r"\operatorname*{argmax}_x f(x)",
    r"\stackrel{a}{=}\overset{b}{\to}\underset{c}{\leftarrow}",
    r"\not= \neq \notin \mathrlap{x}\mathllap{y}",
    r"\big( \Big[ \bigg\langle \Bigg| x",
    r"\mathbb{R}\mathcal{A}\mathfrak{g}\vec{v}\dot{x}",
    r"\text{\textbf{a}\textit{b}}\mbox{c}",
    r"\phantom{x}\hphantom{y}\vphantom{z}\smash{w}",
    r"\color{blue} x \textcolor{red}{y}",
    r"\begin{CD}A @>a>> B\\@VbVV @VVcV\\C @>>d> D\end{CD}",
    r"\begin{aligned}a&=b\tag{1}\end{aligned}",
    r"x\tag{2}",
    r"\angl{n}\phase{x}\underline{a}\overline{b}",
];

#[test]
fn rendering_is_deterministic() {
    let settings = Settings::builder()
        .display_mode(true)
        .throw_on_error(false)
        .trust(TrustSetting::Bool(true))
        .build();
    let render_all = || {
        let ctx = KatexContext::default();
        FORMULAS
            .iter()
            .map(|tex| render_to_string(&ctx, tex, &settings).expect("renders"))
            .collect::<Vec<_>>()
    };
    let first = render_all();
    for _ in 0..20 {
        assert_eq!(render_all(), first);
    }
}

#[test]
fn attributes_keep_katex_order() {
    let ctx = KatexContext::default();
    let html =
        render_to_string(&ctx, r"\rule[-1em]{1em}{2em}", &Settings::default()).expect("renders");
    // As in KaTeX's rule.js mathmlBuilder.
    assert!(
        html.contains(r#"<mpadded height="-1em" depth="1em" voffset="-1em"><mspace mathbackground="black" width="1em" height="2em">"#),
        "{html}"
    );
}
