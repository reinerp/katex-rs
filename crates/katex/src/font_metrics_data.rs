//! Font metric data storage
//! This file contains font metric data and measurements for KaTeX
//! Generated from the original JavaScript fontMetricsData.js (see
//! tools/gen_tables.py)

use crate::static_map::{Map, StrMap, U32Map};
use crate::{ParseError, font_metrics::MetricMap, namespace::KeyMap, types::ParseErrorKind};

/// Font metrics for a single character
/// The array contains: [depth, height, italic, skew, width]
#[derive(Debug, Clone, Copy)]
pub struct CharacterMetrics {
    /// Depth of the character
    pub depth: f64,
    /// Height of the character
    pub height: f64,
    /// Italic correction
    pub italic: f64,
    /// Skew of the character
    pub skew: f64,
    /// Width of the character
    pub width: f64,
}

impl CharacterMetrics {
    /// Create a new FontMetrics instance
    #[must_use]
    pub const fn new(depth: f64, height: f64, italic: f64, skew: f64, width: f64) -> Self {
        Self {
            depth,
            height,
            italic,
            skew,
            width,
        }
    }
}

// Tables generated from data/font_metrics_data.json by tools/gen_tables.py
include!("generated/font_metrics_data.rs");

/// Main font metrics data structure
#[cfg_attr(not(feature = "packed-metrics"), derive(Default))]
pub struct FontMetricsData {
    /// Custom font metrics added at runtime
    pub custom: KeyMap<String, MetricMap>,
    /// The metrics of `generated/font_metrics_packed.bin`, unpacked: (font family, (character code, metrics) by character code).
    #[cfg(feature = "packed-metrics")]
    packed: Vec<(&'static str, Vec<(u32, CharacterMetrics)>)>,
}

/// The font families of `generated/font_metrics_packed.bin`, in its order (that of [`FONT_METRICS_INDEX`]).
#[cfg(feature = "packed-metrics")]
const PACKED_FONTS: [&str; 18] = [
    "AMS-Regular",
    "Caligraphic-Regular",
    "Fraktur-Regular",
    "Main-Bold",
    "Main-BoldItalic",
    "Main-Italic",
    "Main-Regular",
    "Math-BoldItalic",
    "Math-Italic",
    "SansSerif-Bold",
    "SansSerif-Italic",
    "SansSerif-Regular",
    "Script-Regular",
    "Size1-Regular",
    "Size2-Regular",
    "Size3-Regular",
    "Size4-Regular",
    "Typewriter-Regular",
];

#[cfg(feature = "packed-metrics")]
impl Default for FontMetricsData {
    /// Unpacks the tables: the columns of `tools/gen_packed_metrics.py` (key deltas, then depth, height, italic, skew, width in 1/100000,
    /// as LEB128 numbers; the zig-zag numbers are exact multiples of 1/100000, so `n / 1e5` is the f64 of the decimal in the source).
    fn default() -> Self {
        let blob: &[u8] = include_bytes!("generated/font_metrics_packed.bin");
        let n = PACKED_FONTS.len();
        let mut pos = n * 2;
        let mut next = |pos: &mut usize| -> u32 {
            let mut v = 0_u32;
            let mut shift = 0;
            loop {
                let b = blob[*pos];
                *pos += 1;
                v |= u32::from(b & 0x7F) << shift;
                if b & 0x80 == 0 {
                    return v;
                }
                shift += 7;
            }
        };
        let mut packed = Vec::with_capacity(n);
        for (i, name) in PACKED_FONTS.iter().enumerate() {
            let count = usize::from(u16::from_le_bytes([blob[i * 2], blob[i * 2 + 1]]));
            let mut entries: Vec<(u32, CharacterMetrics)> = Vec::with_capacity(count);
            let mut key = 0_u32;
            for _ in 0..count {
                key += next(&mut pos);
                entries.push((key, CharacterMetrics::new(0.0, 0.0, 0.0, 0.0, 0.0)));
            }
            for col in 0..5 {
                for e in &mut entries {
                    let z = next(&mut pos);
                    let v = f64::from(if z & 1 == 0 { (z >> 1) as i32 } else { -(((z + 1) >> 1) as i32) }) / 100_000.0;
                    match col {
                        0 => e.1.depth = v,
                        1 => e.1.height = v,
                        2 => e.1.italic = v,
                        3 => e.1.skew = v,
                        _ => e.1.width = v,
                    }
                }
            }
            packed.push((*name, entries));
        }
        Self { custom: KeyMap::default(), packed }
    }
}

impl FontMetricsData {
    /// Get metrics for a specific character in a font family
    pub fn get_metric(
        &self,
        font_family: &str,
        char_code: u32,
    ) -> Result<Option<&CharacterMetrics>, ParseError> {
        #[cfg(feature = "packed-metrics")]
        if let Some((_, entries)) = self.packed.iter().find(|(name, _)| *name == font_family) {
            return Ok(entries
                .binary_search_by_key(&char_code, |e| e.0)
                .ok()
                .map(|i| &entries[i].1));
        }
        #[cfg(not(feature = "packed-metrics"))]
        if let Some(metrics) = FONT_METRICS_INDEX.get(font_family) {
            return Ok(metrics.get(&char_code));
        }
        if let Some(custom_metrics) = self.custom.get(font_family) {
            return Ok(custom_metrics.get(&char_code));
        }
        Err(ParseError::new(ParseErrorKind::FontMetricsNotFound {
            font_family: font_family.to_owned(),
        }))
    }

    /// Create a new FontMetricsData instance with optional custom metrics
    pub fn add_custom_metrics(
        &mut self,
        font_family: String,
        char_code: u32,
        metrics: CharacterMetrics,
    ) {
        self.custom
            .entry(font_family)
            .or_default()
            .insert(char_code, metrics);
    }
}
