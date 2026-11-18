//! Namespace management with TeX-like grouping semantics
//!
//! This is a Rust port of KaTeX's `src/Namespace.js`.
//! A `Namespace` refers to a space of nameable things like macros or lengths,
//! which can be set either globally or local to a nested group using an
//! undo stack similar to how TeX implements this functionality.

use crate::static_map::Map;
use core::cell::RefMut;

use std::collections::{HashMap, HashSet};

use crate::types::{ParseError, ParseErrorKind};

/// The hasher of the `fast-hash` feature: a multiply-and-rotate hash (as rustc's `FxHasher`) of the machine word, in place of SipHash with random keys.
/// The renderer's tables are keyed by short strings and small integers it controls, and nothing it writes depends on the order of a table.
#[cfg(feature = "fast-hash")]
#[derive(Default, Clone, Copy)]
pub struct FastHasher(usize);

#[cfg(feature = "fast-hash")]
impl FastHasher {
    #[inline]
    fn add(&mut self, word: usize) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x9E37_79B9_usize);
    }
}

#[cfg(feature = "fast-hash")]
impl core::hash::Hasher for FastHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(size_of::<usize>());
        for c in &mut chunks {
            let mut w = [0_u8; size_of::<usize>()];
            w.copy_from_slice(c);
            self.add(usize::from_le_bytes(w));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut w = [0_u8; size_of::<usize>()];
            w[..rest.len()].copy_from_slice(rest);
            self.add(usize::from_le_bytes(w));
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(usize::from(i));
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(i as usize);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i);
    }
    #[inline]
    fn finish(&self) -> u64 {
        // the top bits pick the control byte of the table: mix them in
        (self.0.rotate_left(15) ^ self.0) as u64
    }
}

/// Make it easier to switch between different hash backends.
#[cfg(not(feature = "fast-hash"))]
pub type KeyMap<K, V> = HashMap<K, V>;
/// Alias for the default hash set.
#[cfg(not(feature = "fast-hash"))]
pub type KeySet<K> = HashSet<K>;
/// Make it easier to switch between different hash backends.
#[cfg(feature = "fast-hash")]
pub type KeyMap<K, V> = HashMap<K, V, core::hash::BuildHasherDefault<FastHasher>>;
/// Alias for the default hash set.
#[cfg(feature = "fast-hash")]
pub type KeySet<K> = HashSet<K, core::hash::BuildHasherDefault<FastHasher>>;
/// Mapping type alias
pub type Mapping<V> = KeyMap<String, V>;

/// A `Namespace` implements scoped definitions with begin/end group semantics.
///
/// Performance characteristics mirror the JS version:
/// - `get` and local `set` are O(1)
/// - global `set` is O(depth), where depth is the group nesting level
#[derive(Debug)]
pub struct Namespace<'a, V: Clone + 'static> {
    /// Current (mutable) mapping. Represents the global-level table that
    /// local changes modify, with undos recorded on the stack.
    current: RefMut<'a, Mapping<V>>,
    /// Built-in immutable mappings that never change.
    builtins: &'static Map<&'static str, V>,
    /// Stack of undo maps for nested groups. The stored value is the previous
    /// value of a name (or `None` to indicate deletion) to restore on pop.
    undef_stack: Vec<KeyMap<String, Option<V>>>,
}

impl<'a, V: Clone> Namespace<'a, V> {
    /// Create a new namespace.
    /// - `builtins` are immutable defaults
    /// - `global` initializes the mutable global mapping
    #[must_use]
    pub const fn new(
        builtins: &'static Map<&'static str, V>,
        global: RefMut<'a, Mapping<V>>,
    ) -> Self {
        Self {
            current: global,
            builtins,
            undef_stack: Vec::new(),
        }
    }

    /// Start a new nested group, affecting future local `set`s.
    pub fn begin_group(&mut self) {
        self.undef_stack.push(KeyMap::default());
    }

    /// Purge any key from current
    pub fn purge(&mut self, name: &str) {
        self.current.remove(name);
    }

    fn restore_changes<I>(&mut self, undefs: I)
    where
        I: IntoIterator<Item = (String, Option<V>)>,
    {
        for (name, previous) in undefs {
            match previous {
                Some(v) => {
                    self.current.insert(name, v);
                }
                None => {
                    self.current.remove(&name);
                }
            }
        }
    }

    /// End current nested group, restoring values before the group began.
    pub fn end_group(&mut self) -> Result<(), ParseError> {
        let undefs = self
            .undef_stack
            .pop()
            .ok_or_else(|| ParseError::new(ParseErrorKind::UnbalancedNamespaceDestruction))?;
        self.restore_changes(undefs);
        Ok(())
    }

    /// Ends all currently nested groups (if any), restoring values before the
    /// groups began.
    pub fn end_groups(&mut self) -> usize {
        let mut count = 0;
        while let Some(undefs) = self.undef_stack.pop() {
            self.restore_changes(undefs);
            count += 1;
        }
        count
    }

    /// Detect whether `name` has a definition (either current or builtin)
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.current.contains_key(name) || self.builtins.contains_key(name)
    }

    /// Get the current value of a name, or `None` if there is no value.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&V> {
        self.current.get(name).or_else(|| self.builtins.get(name))
    }

    /// Set the current value of a name, and optionally set it globally too.
    ///
    /// Local `set` sets the current value and (when appropriate) adds an undo
    /// operation to the undo stack. Global `set` may change the undo operation
    /// at every level, so takes time linear in the number of nested groups.
    /// A value of `None` means to delete existing definitions.
    /// In JavaScript, `global` is optional and defaults to `false`.
    pub fn set(&mut self, name: &str, value: Option<V>, global: bool) {
        if global {
            // Global set is equivalent to setting in all groups. Simulate this
            // by destroying any undos currently scheduled for this name, and
            // adding an undo with the new value (in case it later gets locally
            // reset within this environment).
            for level in &mut self.undef_stack {
                level.remove(name);
            }
            if let Some(top) = self.undef_stack.last_mut() {
                top.insert(name.to_owned(), value.clone());
            }
        } else {
            // Undo this set at end of this group (possibly to `None`), unless
            // an undo is already in place, in which case that older value is
            // the correct one.
            if let Some(top) = self.undef_stack.last_mut()
                && !top.contains_key(name)
            {
                let prev = self.current.get(name).cloned();
                top.insert(name.to_owned(), prev);
            }
        }

        match value {
            Some(v) => {
                self.current.insert(name.to_owned(), v);
            }
            None => {
                self.current.remove(name);
            }
        }
    }
}
