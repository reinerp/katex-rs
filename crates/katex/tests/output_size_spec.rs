//! The output-size cap of `render_to_string`.
#![allow(clippy::expect_used)]

use katex::types::ParseErrorKind;
use katex::{KatexContext, Settings, render_to_string};

#[test]
fn output_is_capped_at_max_output_size() {
    let ctx = KatexContext::default();
    let tex = "x+".repeat(500) + "x";
    let full = render_to_string(&ctx, &tex, &Settings::default()).expect("renders");
    let with = |max: usize, throw_on_error: bool| {
        let settings = Settings::builder()
            .max_output_size(max)
            .throw_on_error(throw_on_error)
            .build();
        render_to_string(&ctx, &tex, &settings)
    };
    assert_eq!(with(full.len(), true).expect("fits"), full);
    let err = with(full.len() - 1, true).expect_err("too large");
    assert!(matches!(*err.kind, ParseErrorKind::OutputTooLarge { max } if max == full.len() - 1));
    let span = with(1000, false).expect("error span");
    assert!(
        span.contains("katex-error") && span.contains("Output too large"),
        "{span}"
    );
    assert!(span.len() < full.len());
}

/// From the evaluation, smaller: a 100x100 matrix (69 KB of input) renders to
/// 8 MB, and this 50x50 one to about 2 MB.
#[test]
fn large_outputs_fail_under_a_cap() {
    let ctx = KatexContext::default();
    let row = (0..50)
        .map(|j| format!("a_{{{j}}}"))
        .collect::<Vec<_>>()
        .join("&");
    let tex = format!(
        r"\begin{{matrix}}{}\end{{matrix}}",
        vec![row; 50].join(r"\\")
    );
    let settings = Settings::builder().max_output_size(1 << 20).build();
    let err = render_to_string(&ctx, &tex, &settings).expect_err("too large");
    assert!(matches!(*err.kind, ParseErrorKind::OutputTooLarge { .. }));
}
