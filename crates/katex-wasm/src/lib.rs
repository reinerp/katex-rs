//! Plain C ABI around the katex-rs fork, for `wasm32-unknown-unknown` without wasm-bindgen.
//! Exports: memory, alloc, dealloc, init, render, render_batch, out_ptr, out_len, out_status.
//! One KatexContext and one Settings per (mode, display) are kept alive across calls.
use katex::types::{OutputFormat, TrustSetting};
use katex::{render_to_string, KatexContext, Settings};
use std::cell::RefCell;

/// Options of the shipped (hybrid) output that a page may change: `set_label`, `set_wrap_min`, `set_line`.
#[derive(Clone, Copy)]
struct Config {
    label: bool,
    wrap_min: usize,
    line: Option<(f64, f64)>,
}

/// Settings of a mode: with the `full` feature mode 0 legacy (HTML+MathML), 1 stock HTML only, 2 flat (ALL), 3 hybrid font (ALL | FONT_RUN),
/// 4 hybrid + `compact_line((0.8815, 0.3581))`, 5 flat + the same `compact_line` (those are the modes of the native harness `ch_font.rs`);
/// without it every mode is the hybrid output, 4 with the line extents. The `Config` options apply to the compact modes.
fn settings(display: bool, mode: u32, cfg: Config) -> Settings {
    let mut b = Settings::builder()
        .display_mode(display)
        .throw_on_error(false)
        .trust(TrustSetting::Bool(false))
        .max_expand(1000)
        .max_size(100.0);
    #[cfg(feature = "full")]
    if mode == 0 {
        return b.output(OutputFormat::HtmlAndMathml).build();
    } else if mode == 1 {
        return b.output(OutputFormat::Html).build();
    }
    #[cfg(not(feature = "full"))]
    let mode = if mode == 4 { 4 } else { 3 };
    let mut lv = katex::compact::levels::ALL;
    if mode == 3 || mode == 4 {
        lv |= katex::compact::levels::FONT_RUN;
    }
    b = b
        .output(OutputFormat::CompactHtml)
        .compact_levels(lv)
        .compact_label(cfg.label)
        .compact_precision(0)
        .compact_wrap_min(cfg.wrap_min);
    let line = if mode == 4 || mode == 5 { Some(cfg.line.unwrap_or((0.8815, 0.3581))) } else { None };
    if let Some(l) = line {
        b = b.compact_line(l);
    }
    b.build()
}

struct State {
    ctx: KatexContext,
    cfg: Config,
    settings: Vec<Option<Settings>>, // index mode * 2 + display
    out: Vec<u8>,
    status: u32,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert_with(|| State {
            ctx: KatexContext::default(),
            cfg: Config { label: true, wrap_min: katex::compact::DEFAULT_WRAP_MIN, line: None },
            settings: (0..12).map(|_| None).collect(),
            out: Vec::new(),
            status: 0,
        });
        f(st)
    })
}

fn render_one(st: &mut State, tex: &str, display: bool, mode: u32, out: &mut Vec<u8>) -> u32 {
    let idx = (mode * 2 + u32::from(display)) as usize;
    if st.settings[idx].is_none() {
        st.settings[idx] = Some(settings(display, mode, st.cfg));
    }
    match render_to_string(&st.ctx, tex, st.settings[idx].as_ref().unwrap()) {
        Ok(h) => {
            out.extend_from_slice(h.as_bytes());
            0
        }
        Err(e) => {
            out.extend_from_slice(e.to_string().as_bytes());
            1
        }
    }
}

fn reconfigure(f: impl FnOnce(&mut Config)) {
    with_state(|st| {
        f(&mut st.cfg);
        st.settings.iter_mut().for_each(|s| *s = None);
    });
}

/// Whether the hybrid output carries the TeX as an `aria-label` (default 1).
#[unsafe(no_mangle)]
pub extern "C" fn set_label(on: u32) {
    reconfigure(|c| c.label = on != 0);
}

/// The shortest formula (in characters) that may wrap (default 8).
#[unsafe(no_mangle)]
pub extern "C" fn set_wrap_min(n: u32) {
    reconfigure(|c| c.wrap_min = n as usize);
}

/// The room of the host's line above and below its baseline, in em of the formula, for mode 4 (default 0.8815, 0.3581).
#[unsafe(no_mangle)]
pub extern "C" fn set_line(ascent: f64, descent: f64) {
    reconfigure(|c| c.line = Some((ascent, descent)));
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(n: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(n.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `p` and `n` must be a pointer and capacity from `alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(p: *mut u8, n: usize) {
    drop(unsafe { Vec::from_raw_parts(p, 0, n.max(1)) });
}

/// Build the context and the settings of a mode now (otherwise the first `render` does it).
#[unsafe(no_mangle)]
pub extern "C" fn init(mode: u32) {
    with_state(|st| {
        for d in 0..2 {
            let idx = (mode * 2 + d) as usize;
            if st.settings[idx].is_none() {
                st.settings[idx] = Some(settings(d == 1, mode, st.cfg));
            }
        }
    });
}

/// Render one formula (UTF-8 TeX at `ptr`, `len` bytes). flags: bit 0 display, bits 1.. mode.
/// Returns a pointer to the HTML, valid until the next call; its length is `out_len()`, `out_status()` is 1 for an error text.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn render(ptr: *const u8, len: usize, flags: u32) -> *const u8 {
    let tex = unsafe { std::slice::from_raw_parts(ptr, len) };
    let tex = String::from_utf8_lossy(tex);
    with_state(|st| {
        let mut out = std::mem::take(&mut st.out);
        out.clear();
        st.status = render_one(st, &tex, flags & 1 == 1, flags >> 1, &mut out);
        st.out = out;
        st.out.as_ptr()
    })
}

/// Render many formulas. Input at `ptr`: for each, u8 display, u32 LE byte length, the TeX; `len` is the whole input's length.
/// Output: for each, u32 LE length of the HTML then the HTML (error texts too: status of the last one in `out_status`, a NUL byte first for each error).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn render_batch(ptr: *const u8, len: usize, flags: u32) -> *const u8 {
    let inp = unsafe { std::slice::from_raw_parts(ptr, len) };
    with_state(|st| {
        let mut out = std::mem::take(&mut st.out);
        out.clear();
        let mode = flags >> 1;
        let mut i = 0;
        let mut status = 0;
        while i + 5 <= inp.len() {
            let display = inp[i] == 1;
            let n = u32::from_le_bytes([inp[i + 1], inp[i + 2], inp[i + 3], inp[i + 4]]) as usize;
            let tex = String::from_utf8_lossy(&inp[i + 5..i + 5 + n]);
            i += 5 + n;
            let at = out.len();
            out.extend_from_slice(&[0; 4]);
            let s0 = out.len();
            let s = render_one(st, &tex, display, mode, &mut out);
            if s != 0 {
                status = s;
                out.insert(s0, 0);
            }
            let l = (out.len() - s0) as u32;
            out[at..at + 4].copy_from_slice(&l.to_le_bytes());
        }
        st.status = status;
        st.out = out;
        st.out.as_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn out_ptr() -> *const u8 {
    with_state(|st| st.out.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn out_len() -> usize {
    with_state(|st| st.out.len())
}
#[unsafe(no_mangle)]
pub extern "C" fn out_status() -> u32 {
    with_state(|st| st.status)
}
