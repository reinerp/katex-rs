//! Constant lookup tables: maps and sets stored as slices sorted by key and
//! searched by binary search.
//!
//! The constructors check at compile time that keys are strictly increasing,
//! so a table written out of order (or with a duplicate key) fails to build.
//! Iteration visits entries in key order.

/// A key type usable in [`Map`] and [`Set`].
pub trait Key: Copy + 'static {
    /// The type lookups take (`str` for string keys).
    type Borrowed: ?Sized + Ord;

    /// Returns the key in its lookup form.
    fn borrowed(&self) -> &Self::Borrowed;
}

impl Key for &'static str {
    type Borrowed = str;

    fn borrowed(&self) -> &str {
        self
    }
}

impl Key for char {
    type Borrowed = Self;

    fn borrowed(&self) -> &Self {
        self
    }
}

impl Key for u32 {
    type Borrowed = Self;

    fn borrowed(&self) -> &Self {
        self
    }
}

/// Compares two strings byte-wise in a constant context.
const fn str_less(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut i = 0;
    while i < a.len() && i < b.len() {
        if a[i] != b[i] {
            return a[i] < b[i];
        }
        i += 1;
    }
    a.len() < b.len()
}

/// A map with string keys; construct with `StrMap::new`.
pub type StrMap<V> = Map<&'static str, V>;
/// A map with character keys; construct with `CharMap::new`.
pub type CharMap<V> = Map<char, V>;
/// A map with integer keys; construct with `U32Map::new`.
pub type U32Map<V> = Map<u32, V>;
/// A set of strings; construct with `StrSet::new`.
pub type StrSet = Set<&'static str>;
/// A set of characters; construct with `CharSet::new`.
pub type CharSet = Set<char>;

/// A constant map from keys to values.
#[derive(Debug)]
pub struct Map<K: 'static, V: 'static> {
    entries: &'static [(K, V)],
}

impl<V> Map<&'static str, V> {
    /// Creates a map from entries sorted by key (checked at compile time).
    ///
    /// # Panics
    ///
    /// If the keys are not strictly increasing; in a constant this is a
    /// compile error.
    #[must_use]
    pub const fn new(entries: &'static [(&'static str, V)]) -> Self {
        let mut i = 1;
        while i < entries.len() {
            assert!(
                str_less(entries[i - 1].0, entries[i].0),
                "static map keys must be strictly increasing"
            );
            i += 1;
        }
        Self { entries }
    }
}

impl<V> Map<char, V> {
    /// Creates a map from entries sorted by key (checked at compile time).
    ///
    /// # Panics
    ///
    /// If the keys are not strictly increasing; in a constant this is a
    /// compile error.
    #[must_use]
    pub const fn new(entries: &'static [(char, V)]) -> Self {
        let mut i = 1;
        while i < entries.len() {
            assert!(
                (entries[i - 1].0 as u32) < (entries[i].0 as u32),
                "static map keys must be strictly increasing"
            );
            i += 1;
        }
        Self { entries }
    }
}

impl<V> Map<u32, V> {
    /// Creates a map from entries sorted by key (checked at compile time).
    ///
    /// # Panics
    ///
    /// If the keys are not strictly increasing; in a constant this is a
    /// compile error.
    #[must_use]
    pub const fn new(entries: &'static [(u32, V)]) -> Self {
        let mut i = 1;
        while i < entries.len() {
            assert!(
                entries[i - 1].0 < entries[i].0,
                "static map keys must be strictly increasing"
            );
            i += 1;
        }
        Self { entries }
    }
}

impl<K: Key, V> Map<K, V> {
    fn find(&self, key: &K::Borrowed) -> Option<&'static (K, V)> {
        let entries = self.entries;
        entries
            .binary_search_by(|(k, _)| k.borrowed().cmp(key))
            .ok()
            .map(|index| &entries[index])
    }

    /// Returns the value for `key`.
    pub fn get(&self, key: &K::Borrowed) -> Option<&'static V> {
        self.find(key).map(|(_, value)| value)
    }

    /// Returns the index of `key` in the key order, and its value.
    pub fn position(&self, key: &K::Borrowed) -> Option<(usize, &'static V)> {
        let entries = self.entries;
        let index = entries
            .binary_search_by(|(k, _)| k.borrowed().cmp(key))
            .ok()?;
        Some((index, &entries[index].1))
    }

    /// Returns the stored key and value for `key`.
    pub fn get_entry(&self, key: &K::Borrowed) -> Option<(&'static K, &'static V)> {
        self.find(key).map(|(k, value)| (k, value))
    }

    /// Returns the stored key equal to `key`.
    pub fn get_key(&self, key: &K::Borrowed) -> Option<&'static K> {
        self.find(key).map(|(k, _)| k)
    }

    /// Whether the map has an entry for `key`.
    pub fn contains_key(&self, key: &K::Borrowed) -> bool {
        self.find(key).is_some()
    }

    /// Number of entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterates over the entries in key order.
    pub fn entries(&self) -> impl Iterator<Item = (&'static K, &'static V)> + use<K, V> {
        self.entries.iter().map(|(k, value)| (k, value))
    }

    /// Iterates over the keys in order.
    pub fn keys(&self) -> impl Iterator<Item = &'static K> + use<K, V> {
        self.entries.iter().map(|(k, _)| k)
    }

    /// Iterates over the values in key order.
    pub fn values(&self) -> impl Iterator<Item = &'static V> + use<K, V> {
        self.entries.iter().map(|(_, value)| value)
    }
}

/// A constant set of keys.
#[derive(Debug)]
pub struct Set<K: 'static> {
    keys: &'static [K],
}

impl Set<&'static str> {
    /// Creates a set from keys sorted in increasing order (checked at compile
    /// time).
    ///
    /// # Panics
    ///
    /// If the keys are not strictly increasing; in a constant this is a
    /// compile error.
    #[must_use]
    pub const fn new(keys: &'static [&'static str]) -> Self {
        let mut i = 1;
        while i < keys.len() {
            assert!(
                str_less(keys[i - 1], keys[i]),
                "static set keys must be strictly increasing"
            );
            i += 1;
        }
        Self { keys }
    }
}

impl Set<char> {
    /// Creates a set from keys sorted in increasing order (checked at compile
    /// time).
    ///
    /// # Panics
    ///
    /// If the keys are not strictly increasing; in a constant this is a
    /// compile error.
    #[must_use]
    pub const fn new(keys: &'static [char]) -> Self {
        let mut i = 1;
        while i < keys.len() {
            assert!(
                (keys[i - 1] as u32) < (keys[i] as u32),
                "static set keys must be strictly increasing"
            );
            i += 1;
        }
        Self { keys }
    }
}

impl<K: Key> Set<K> {
    /// Whether the set contains `key`.
    pub fn contains(&self, key: &K::Borrowed) -> bool {
        self.keys
            .binary_search_by(|k| k.borrowed().cmp(key))
            .is_ok()
    }

    /// Returns the stored key equal to `key`.
    pub fn get_key(&self, key: &K::Borrowed) -> Option<&'static K> {
        let keys = self.keys;
        keys.binary_search_by(|k| k.borrowed().cmp(key))
            .ok()
            .map(|index| &keys[index])
    }

    /// Number of keys.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether the set is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Iterates over the keys in order.
    pub fn iter(&self) -> impl Iterator<Item = &'static K> + use<K> {
        self.keys.iter()
    }
}
