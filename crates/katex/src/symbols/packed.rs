//! The symbol tables as `tools/gen_tables.py` packs them (`generated/symbols_packed.bin`), for the `packed-symbols` feature.
//!
//! For the math table and then the text table: the number of entries n (LEB128), the n names (length, bytes), n bytes (bit 7: the character the entry
//! replaces is a key too, bit 6: the font is AMS, the low bits: the index of the group in [`GROUPS`]), and n replacement characters (LEB128, 0 for none).

use super::{Atom, CharInfo, Font, Group, NonAtom, Symbols};
use crate::namespace::KeyMap;

const BLOB: &[u8] = include_bytes!("../generated/symbols_packed.bin");

/// The groups, in the order of `PACKED_GROUPS` in `tools/gen_tables.py`.
const GROUPS: [Group; 11] = [
    Group::Atom(Atom::Bin),
    Group::Atom(Atom::Close),
    Group::Atom(Atom::Inner),
    Group::Atom(Atom::Open),
    Group::Atom(Atom::Punct),
    Group::Atom(Atom::Rel),
    Group::NonAtom(NonAtom::AccentToken),
    Group::NonAtom(NonAtom::MathOrd),
    Group::NonAtom(NonAtom::OpToken),
    Group::NonAtom(NonAtom::Spacing),
    Group::NonAtom(NonAtom::TextOrd),
];

fn varint(blob: &[u8], pos: &mut usize) -> u32 {
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
}

/// Reads one table of the blob from `pos`: every name, and the replaced character of the entries that accept it, with their information.
fn read_table(blob: &[u8], pos: &mut usize, table: &mut KeyMap<String, CharInfo>) {
    let n = varint(blob, pos) as usize;
    let mut names = Vec::with_capacity(n);
    for _ in 0..n {
        let len = varint(blob, pos) as usize;
        names.push(core::str::from_utf8(&blob[*pos..*pos + len]).unwrap_or_default());
        *pos += len;
    }
    let info_at = *pos;
    *pos += n;
    table.reserve(n + n / 2);
    for (i, name) in names.into_iter().enumerate() {
        let b = blob[info_at + i];
        let replace = char::from_u32(varint(blob, pos)).filter(|c| *c != '\0');
        let info = CharInfo {
            font: if b & 0x40 != 0 { Font::Ams } else { Font::Main },
            group: GROUPS[usize::from(b & 0x0F)],
            replace,
        };
        if b & 0x80 != 0
            && let Some(c) = replace
        {
            table.insert(c.to_string(), info.clone());
        }
        table.insert(name.to_owned(), info);
    }
}

/// Fills the math and text tables of `symbols` with the entries of the blob.
pub(super) fn load(symbols: &mut Symbols) {
    let mut pos = 0;
    read_table(BLOB, &mut pos, &mut symbols.math);
    read_table(BLOB, &mut pos, &mut symbols.text);
}

#[cfg(all(test, not(feature = "packed-symbols")))]
mod tests {
    use super::*;
    use crate::symbols::{POPULATE_MATH_SYMBOLS_MAP, POPULATE_TEXT_SYMBOLS_MAP};

    #[test]
    fn the_blob_is_the_static_tables() {
        let mut s = Symbols::new();
        load(&mut s);
        for (map, table) in [(&POPULATE_MATH_SYMBOLS_MAP, &s.math), (&POPULATE_TEXT_SYMBOLS_MAP, &s.text)] {
            assert_eq!(table.len(), map.len());
            for (k, v) in map.entries() {
                let got = table.get(*k).unwrap_or_else(|| panic!("{k:?} missing"));
                assert_eq!((&got.font, &got.group, got.replace), (&v.font, &v.group, v.replace), "{k:?}");
            }
        }
    }
}
