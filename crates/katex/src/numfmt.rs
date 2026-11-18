//! Reading and writing the decimal numbers of the renderer: `em` lengths with four decimals, the numbers of TeX dimensions.
//!
//! With the default feature `std-float` these are `str::parse::<f64>` and `format!("{n:.4}")`. Without it, small routines of this module stand in
//! for them, so that the float formatting and parsing code of `core` (and its 10 KB table of powers of five) is not linked in:
//!
//! * [`fixed`] (and [`fixed4`]) is exact: it writes the same text as `format!("{n:.4}")` for every `f64` (rounding the exact binary value, ties to even).
//! * [`parse_f64`] is exact for a number of up to 15 significant digits and a decimal exponent of at most 22 (a quotient or a product of two
//!   exactly represented numbers is correctly rounded), which covers the numbers of TeX dimensions and the `em` values the renderer writes. Longer
//!   numbers are read with a few ulps of error.

/// The number in `s` (as `str::parse::<f64>` reads it), if it is one.
#[must_use]
pub(crate) fn parse_f64(s: &str) -> Option<f64> {
    #[cfg(feature = "std-float")]
    {
        s.parse::<f64>().ok()
    }
    #[cfg(not(feature = "std-float"))]
    {
        parse_small(s)
    }
}

/// `format!("{n:.4}")`.
#[must_use]
pub(crate) fn fixed4(n: f64) -> String {
    fixed(n, 4)
}

/// `format!("{n:.prec$}")` for `prec` up to 9.
#[must_use]
pub(crate) fn fixed(n: f64, prec: usize) -> String {
    #[cfg(feature = "std-float")]
    {
        format!("{n:.prec$}")
    }
    #[cfg(not(feature = "std-float"))]
    {
        fixed_exact(n, prec as u32)
    }
}

#[cfg(any(test, not(feature = "std-float")))]
const POW10: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

/// `str::parse::<f64>` for a decimal number, a sign, `inf`, `infinity` or `nan`; see the module documentation for the precision.
#[cfg(any(test, not(feature = "std-float")))]
fn parse_small(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let mut i = 0;
    let neg = match b.first() {
        Some(b'-') => {
            i = 1;
            true
        }
        Some(b'+') => {
            i = 1;
            false
        }
        _ => false,
    };
    let signed = |v: f64| if neg { -v } else { v };
    let rest = &s[i..];
    if rest.eq_ignore_ascii_case("inf") || rest.eq_ignore_ascii_case("infinity") {
        return Some(signed(f64::INFINITY));
    }
    if rest.eq_ignore_ascii_case("nan") {
        return Some(f64::NAN);
    }
    let mut mant: u64 = 0;
    let mut digits = 0_u32; // significant digits kept in `mant`
    let mut exp10: i64 = 0;
    let mut seen = false;
    let mut after_point = false;
    while i < b.len() {
        match b[i] {
            c @ b'0'..=b'9' => {
                seen = true;
                if digits < 19 {
                    mant = mant * 10 + u64::from(c - b'0');
                    if mant != 0 {
                        digits += 1;
                    }
                    if after_point {
                        exp10 -= 1;
                    }
                } else if !after_point {
                    exp10 += 1;
                }
            }
            b'.' if !after_point => after_point = true,
            _ => break,
        }
        i += 1;
    }
    if !seen {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        let eneg = match b.get(i) {
            Some(b'-') => {
                i += 1;
                true
            }
            Some(b'+') => {
                i += 1;
                false
            }
            _ => false,
        };
        let start = i;
        let mut e: i64 = 0;
        while i < b.len() && b[i].is_ascii_digit() {
            e = (e * 10 + i64::from(b[i] - b'0')).min(100_000);
            i += 1;
        }
        if i == start {
            return None;
        }
        exp10 += if eneg { -e } else { e };
    }
    if i != b.len() {
        return None;
    }
    if mant == 0 {
        return Some(signed(0.0));
    }
    let mut v = mant as f64;
    if digits <= 15 && (-22..=22).contains(&exp10) {
        // both operands are exact: one correctly rounded operation
        if exp10 < 0 {
            v /= POW10[(-exp10) as usize];
        } else {
            v *= POW10[exp10 as usize];
        }
    } else {
        let mut e = exp10;
        while e > 0 {
            let k = e.min(22);
            v *= POW10[k as usize];
            e -= k;
        }
        while e < 0 {
            let k = (-e).min(22);
            v /= POW10[k as usize];
            e += k;
        }
    }
    Some(signed(v))
}

/// The decimal digits of `m * 2^e` (e >= 0) as a string, by doubling a base 10^9 number.
#[cfg(any(test, not(feature = "std-float")))]
fn big_digits(m: u64, e: u32) -> String {
    let mut limbs: Vec<u32> = vec![(m % 1_000_000_000) as u32, ((m / 1_000_000_000) % 1_000_000_000) as u32, (m / 1_000_000_000_000_000_000) as u32];
    for _ in 0..e {
        let mut carry = 0_u32;
        for l in &mut limbs {
            let v = *l * 2 + carry;
            carry = v / 1_000_000_000;
            *l = v % 1_000_000_000;
        }
        if carry != 0 {
            limbs.push(carry);
        }
    }
    while limbs.len() > 1 && limbs.last() == Some(&0) {
        limbs.pop();
    }
    let mut out = limbs.last().copied().unwrap_or(0).to_string();
    for l in limbs.iter().rev().skip(1) {
        let s = l.to_string();
        out.push_str(&"0".repeat(9 - s.len()));
        out.push_str(&s);
    }
    out
}

/// `format!("{n:.prec$}")`: the exact value of `n` rounded to `prec` decimals (at most 9), ties to even.
#[cfg(any(test, not(feature = "std-float")))]
fn fixed_exact(n: f64, prec: u32) -> String {
    if n.is_nan() {
        return "NaN".to_owned();
    }
    let mut out = String::new();
    if n.is_sign_negative() {
        out.push('-');
    }
    if n.is_infinite() {
        out.push_str("inf");
        return out;
    }
    let bits = n.abs().to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1_u64 << 52) - 1);
    let (m, e) = if exp == 0 { (frac, -1074) } else { (frac | (1_u64 << 52), exp - 1075) };
    // `scaled` is value * 10^prec rounded to an integer, as decimal digits
    let scaled = if e >= 0 {
        let mut s = big_digits(m, e as u32);
        s.push_str(&"0".repeat(prec as usize));
        s
    } else {
        let s = (-e) as u32;
        let num = u128::from(m) * 10_u128.pow(prec);
        if s >= 128 {
            "0".to_owned()
        } else {
            let mut q = num >> s;
            let rem = num & ((1_u128 << s) - 1);
            let half = 1_u128 << (s - 1);
            if rem > half || (rem == half && q & 1 == 1) {
                q += 1;
            }
            q.to_string()
        }
    };
    let pad = (prec as usize + 1).saturating_sub(scaled.len());
    out.push_str(&"0".repeat(pad));
    out.push_str(&scaled);
    if prec > 0 {
        out.insert(out.len() - prec as usize, '.');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_fixed(n: f64) {
        assert_eq!(fixed_exact(n, 4), format!("{n:.4}"), "fixed4 of {n:e} ({:#x})", n.to_bits());
        assert_eq!(fixed_exact(n, 6), format!("{n:.6}"), "fixed6 of {n:e} ({:#x})", n.to_bits());
    }

    #[test]
    fn fixed4_matches_core() {
        for n in [0.0, -0.0, 1.0, -1.5, 0.00005, 0.00015, 0.00025, 0.00004999999, 1.23456, 2.5e-5, 5e-5, 0.5, 1e15, 9.3e14, 1e22, 1e23, 1.7976931348623157e308, 5e-324, 123456789.12345, f64::INFINITY, f64::NEG_INFINITY, f64::NAN, 0.30000000000000004, 1.00005, 1.00015, 0.99995, 99999.99995] {
            check_fixed(n);
            check_fixed(-n);
        }
        let mut x = 0x9e37_79b9_7f4a_7c15_u64;
        for _ in 0..50_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            check_fixed(f64::from_bits(x));
            // numbers near what the renderer writes: multiples of 1/20000 and their neighbours
            let k = (x % 4_000_000) as f64 / 20_000.0 - 100.0;
            check_fixed(k);
            check_fixed(f64::from_bits(k.to_bits() + 1));
            check_fixed(f64::from_bits(k.to_bits().wrapping_sub(1)));
        }
    }

    #[test]
    fn parse_matches_core() {
        for s in ["0", "1", "-1", "+1.5", ".5", "5.", "1.5e3", "1e22", "1e-22", "0.1", "0.2", "12345678901234.5", "0.0001", "-0", "-0.0", "inf", "-inf", "Infinity", "nan", "1e", "e1", ".", "", "-", "1.2.3", "1,5", "0.68889", "100.00000", "1e400", "1e-400"] {
            let a = parse_small(s);
            let b = s.parse::<f64>().ok();
            match (a, b) {
                (Some(x), Some(y)) if x.is_nan() && y.is_nan() => {}
                (a, b) => assert_eq!(a.map(f64::to_bits), b.map(f64::to_bits), "parse of {s:?}"),
            }
        }
        // every number of up to 15 significant digits and four decimals, the way the renderer writes them
        for i in 0..400_000_i64 {
            let s = format!("{}.{:04}", i / 10_000 - 20, i % 10_000);
            assert_eq!(parse_small(&s).map(f64::to_bits), s.parse::<f64>().ok().map(f64::to_bits), "{s}");
        }
    }
}
