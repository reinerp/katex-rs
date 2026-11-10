//! Insertion-ordered attribute map for output nodes.

use core::fmt;
use core::iter::Map;
use core::mem;
use core::slice;

use alloc::vec;

/// Attributes of an HTML, SVG or MathML node, in insertion order.
///
/// KaTeX keeps attributes in JavaScript objects, whose string keys enumerate in
/// insertion order, so it writes them in the order they were set. Keeping the
/// same order makes the markup deterministic and byte-compatible with KaTeX;
/// a hash map would write them in a different order on every run. Setting an
/// attribute that is already present replaces its value in place, as assigning
/// to an object property does. Nodes have a handful of attributes, so this is
/// a vector with linear lookup.
#[derive(Clone, Default)]
pub struct AttributeMap {
    entries: Vec<(String, String)>,
}

impl AttributeMap {
    /// An empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Sets `key` to `value`, keeping the position of an existing `key`.
    /// Returns the previous value, if any.
    pub fn insert(&mut self, key: String, value: String) -> Option<String> {
        if let Some(old) = self.get_mut(&key) {
            return Some(mem::replace(old, value));
        }
        self.entries.push((key, value));
        None
    }

    /// The value of `key`, if set.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&String> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The value of `key` for modification, if set.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut String> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Whether `key` is set.
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Removes `key`, keeping the order of the others. Returns its value.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(index).1)
    }

    /// The number of attributes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no attributes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The attributes in insertion order.
    pub fn iter(&self) -> Iter<'_> {
        self.into_iter()
    }
}

/// Iterator over an [`AttributeMap`]'s entries in insertion order.
pub type Iter<'a> =
    Map<slice::Iter<'a, (String, String)>, fn(&(String, String)) -> (&String, &String)>;

impl<'a> IntoIterator for &'a AttributeMap {
    type Item = (&'a String, &'a String);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

impl IntoIterator for AttributeMap {
    type Item = (String, String);
    type IntoIter = vec::IntoIter<(String, String)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl Extend<(String, String)> for AttributeMap {
    fn extend<I: IntoIterator<Item = (String, String)>>(&mut self, iter: I) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

impl FromIterator<(String, String)> for AttributeMap {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl PartialEq for AttributeMap {
    /// Equal as maps: the same attributes with the same values, in any order.
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

impl Eq for AttributeMap {}

impl fmt::Debug for AttributeMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str)]) -> AttributeMap {
        entries
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn keeps_insertion_order_and_replaces_in_place() {
        let mut m = map(&[("width", "1"), ("height", "2"), ("style", "x")]);
        assert_eq!(
            m.insert("height".to_owned(), "3".to_owned()),
            Some("2".to_owned())
        );
        assert_eq!(m.insert("viewBox".to_owned(), "v".to_owned()), None);
        let keys: Vec<&String> = m.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["width", "height", "style", "viewBox"]);
        assert_eq!(m.get("height").map(String::as_str), Some("3"));
        assert_eq!(m.remove("width").as_deref(), Some("1"));
        assert!(!m.contains_key("width"));
        assert_eq!(m.len(), 3);
        assert_eq!(m, map(&[("viewBox", "v"), ("style", "x"), ("height", "3")]));
        assert_ne!(m, map(&[("viewBox", "v"), ("style", "x"), ("height", "4")]));
    }
}
