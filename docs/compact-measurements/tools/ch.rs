use katex::types::{OutputFormat, TrustSetting};
use katex::{render_to_string, KatexContext, Settings};
use std::io::{Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

fn settings(display: bool, mode: &str) -> Settings {
    let mut b = Settings::builder()
        .display_mode(display)
        .throw_on_error(false)
        .trust(TrustSetting::Bool(false))
        .max_expand(1000)
        .max_size(100.0);
    b = if mode == "legacy" {
        b.output(OutputFormat::HtmlAndMathml)
    } else if mode == "html" {
        b.output(OutputFormat::Html)
    } else {
        // compact:LEVELS[:nolabel]
        let mut it = mode.split(':').skip(1);
        let lv: u32 = it.next().filter(|s| !s.is_empty()).map_or(katex::compact::levels::ALL, |s| s.parse().unwrap());
        let mut label = true;
        let mut prec = 0u32;
        let mut wrap = katex::compact::DEFAULT_WRAP_MIN;
        let mut line: Option<(f64, f64)> = None;
        for t in it {
            if t == "nolabel" { label = false; } else if let Some(n) = t.strip_prefix('p') { prec = n.parse().unwrap(); }
            else if let Some(n) = t.strip_prefix('w') { wrap = n.parse().unwrap(); }
            else if let Some(n) = t.strip_prefix('l') { let (a, d) = n.split_once(',').unwrap(); line = Some((a.parse().unwrap(), d.parse().unwrap())); }
        }
        let mut b = b.output(OutputFormat::CompactHtml).compact_levels(lv).compact_label(label).compact_precision(prec).compact_wrap_min(wrap);
        if let Some(l) = line { b = b.compact_line(l); }
        b
    };
    b.build()
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn parse_input(path: &str) -> Vec<(u64, bool, String)> {
    let mut buf = Vec::new();
    std::fs::File::open(path).unwrap().read_to_end(&mut buf).unwrap();
    let mut out = Vec::new();
    let mut i = 0;
    while i < buf.len() {
        let nl = i + buf[i..].iter().position(|&b| b == b'\n').unwrap();
        let hdr = std::str::from_utf8(&buf[i..nl]).unwrap();
        let mut it = hdr.split(' ');
        let id: u64 = it.next().unwrap().parse().unwrap();
        let disp = it.next().unwrap() == "1";
        let len: usize = it.next().unwrap().parse().unwrap();
        let tex = String::from_utf8(buf[nl + 1..nl + 1 + len].to_vec()).unwrap();
        out.push((id, disp, tex));
        i = nl + 1 + len + 1;
    }
    out
}

// ch render formulas.bin out.jsonl MODE REPS [orig|inline|display]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    std::panic::set_hook(Box::new(|_| {}));
    let ctx = KatexContext::default();
    let items = parse_input(&a[2]);
    let mode = a[4].clone();
    let reps: usize = a[5].parse().unwrap();
    let force = a.get(6).map(String::as_str).unwrap_or("orig");
    let disp = |d: bool| match force { "inline" => false, "display" => true, _ => d };
    let go = |tex: &str, d: bool| {
        let s = settings(d, &mode);
        catch_unwind(AssertUnwindSafe(|| render_to_string(&ctx, tex, &s).map_err(|e| e.to_string()))).map_err(|p| {
            p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
        })
    };
    for (_, d, t) in &items { let _ = go(t, disp(*d)); }
    let mut out = std::io::BufWriter::new(std::fs::File::create(&a[3]).unwrap());
    for (id, d, t) in &items {
        let mut times = Vec::new();
        let mut res = None;
        for _ in 0..reps {
            let st = Instant::now();
            let r = go(t, disp(*d));
            times.push(st.elapsed().as_nanos() as u64);
            res = Some(r);
        }
        times.sort();
        let ns = times[times.len() / 2];
        let line = match res.unwrap() {
            Ok(Ok(h)) => format!("{{\"id\":{id},\"ns\":{ns},\"html\":{}}}", esc(&h)),
            Ok(Err(e)) => format!("{{\"id\":{id},\"ns\":{ns},\"err\":{}}}", esc(&e)),
            Err(p) => format!("{{\"id\":{id},\"ns\":{ns},\"panic\":{}}}", esc(&p)),
        };
        writeln!(out, "{line}").unwrap();
    }
}
