//! A sparse map with packed, contiguous values.

use alloc::vec::{self, Vec};

use crate::Key;
use crate::sparse::Sparse;

mod entry;
mod iter;
mod traits;

pub use entry::{Entry, OccupiedEntry, VacantEntry};
pub use iter::{Drain, IntoIter, Iter, IterMut};

/// A map from [`Key`]s to values, stored densely.
///
/// Keys and values live in two parallel, contiguous arrays. A sparse array
/// maps each key's [`index`](Key::index) to its dense position, giving O(1)
/// lookup, insertion and removal, and iteration at slice speed.
///
/// Removal moves the last entry into the vacated position, so dense order is
/// unspecified and positions are not stable across removals.
///
/// The map holds at most `u32::MAX` entries.
pub struct SparseMap<K, V> {
    sparse: Sparse,
    keys: Vec<K>,
    values: Vec<V>,
}

impl<K, V> SparseMap<K, V> {
    /// Creates an empty map without allocating.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sparse: Sparse::new(),
            keys: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Creates an empty map with room for `capacity` entries.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            sparse: Sparse::new(),
            keys: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
        }
    }

    /// Returns the number of entries.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns `true` if the map has no entries.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Returns the number of entries the map holds without reallocating.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.keys.capacity().min(self.values.capacity())
    }

    /// Returns the exclusive bound of key indices that have a sparse slot.
    #[must_use]
    pub fn key_capacity(&self) -> usize {
        self.sparse.len()
    }

    /// Reserves room for at least `additional` more entries.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `isize::MAX` bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.keys.reserve(additional);
        self.values.reserve(additional);
    }

    /// Ensures every key with index below `end` has a sparse slot.
    pub fn reserve_keys(&mut self, end: usize) {
        if let Some(last) = end.checked_sub(1) {
            self.sparse.reserve(last);
        }
    }

    /// Keys in dense order.
    #[inline]
    #[must_use]
    pub fn keys(&self) -> &[K] {
        &self.keys
    }

    /// Values in dense order.
    #[inline]
    #[must_use]
    pub fn values(&self) -> &[V] {
        &self.values
    }

    /// Mutable values in dense order.
    ///
    /// Keys stay fixed: reordering this slice reassigns values to keys.
    #[inline]
    #[must_use]
    pub fn values_mut(&mut self) -> &mut [V] {
        &mut self.values
    }

    /// Keys and values in dense order.
    #[inline]
    #[must_use]
    pub fn as_slices(&self) -> (&[K], &[V]) {
        (&self.keys, &self.values)
    }

    /// Keys and mutable values in dense order.
    #[inline]
    #[must_use]
    pub fn as_mut_slices(&mut self) -> (&[K], &mut [V]) {
        (&self.keys, &mut self.values)
    }

    /// Consumes the map, yielding its keys in dense order.
    #[inline]
    pub fn into_keys(self) -> vec::IntoIter<K> {
        self.keys.into_iter()
    }

    /// Consumes the map, yielding its values in dense order.
    #[inline]
    pub fn into_values(self) -> vec::IntoIter<V> {
        self.values.into_iter()
    }

    fn into_vecs(self) -> (Vec<K>, Vec<V>) {
        (self.keys, self.values)
    }

    #[cold]
    #[inline(never)]
    fn grow(&mut self) {
        self.reserve(1);
    }
}

impl<K: Copy, V> SparseMap<K, V> {
    /// Returns the entry at dense `position`.
    #[inline]
    #[must_use]
    pub fn get_index(&self, position: usize) -> Option<(K, &V)> {
        Some((*self.keys.get(position)?, self.values.get(position)?))
    }

    /// Returns the entry at dense `position` with a mutable value.
    #[inline]
    #[must_use]
    pub fn get_index_mut(&mut self, position: usize) -> Option<(K, &mut V)> {
        Some((*self.keys.get(position)?, self.values.get_mut(position)?))
    }

    /// Iterates over `(key, &value)` in dense order.
    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::new(&self.keys, &self.values)
    }

    /// Iterates over `(key, &mut value)` in dense order.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut::new(&self.keys, &mut self.values)
    }
}

impl<K: Key, V> SparseMap<K, V> {
    /// Returns the dense position of `key`.
    #[inline]
    #[must_use]
    pub fn position(&self, key: K) -> Option<usize> {
        self.sparse.position(key.index(), self.len())
    }

    /// Returns `true` if the map contains `key`.
    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: K) -> bool {
        self.sparse.contains(key.index())
    }

    /// Returns a reference to the value of `key`.
    #[inline]
    #[must_use]
    pub fn get(&self, key: K) -> Option<&V> {
        self.values.get(self.position(key)?)
    }

    /// Returns a mutable reference to the value of `key`.
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        let position = self.position(key)?;
        self.values.get_mut(position)
    }

    /// Returns mutable references to the values of `N` keys at once.
    ///
    /// Each element is `None` if its key is absent.
    ///
    /// # Panics
    ///
    /// Panics if two present keys are equal.
    #[must_use]
    pub fn get_disjoint_mut<const N: usize>(&mut self, keys: [K; N]) -> [Option<&mut V>; N] {
        let positions = keys.map(|key| self.position(key));
        for (i, position) in positions.iter().enumerate() {
            assert!(
                position.is_none() || !positions[..i].contains(position),
                "duplicate keys in get_disjoint_mut"
            );
        }
        let values = self.values.as_mut_ptr();
        // SAFETY: present positions are in bounds and pairwise distinct, so
        // the references are disjoint and live no longer than `&mut self`.
        positions.map(|position| position.map(|position| unsafe { &mut *values.add(position) }))
    }

    /// Inserts `value` at `key`, returning the previous value.
    ///
    /// The stored key is kept on replacement. A new entry is appended to the
    /// dense arrays.
    ///
    /// # Panics
    ///
    /// Panics if the map already holds `u32::MAX` entries or allocation fails.
    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.entry(key) {
            Entry::Occupied(mut entry) => Some(entry.insert(value)),
            Entry::Vacant(entry) => {
                entry.insert(value);
                None
            }
        }
    }

    /// Returns the entry of `key` for in-place manipulation.
    ///
    /// Ensures `key` has a sparse slot.
    #[inline]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        Entry::new(self, key)
    }

    /// Removes `key`, returning its value.
    ///
    /// The last entry moves into the vacated position.
    #[inline]
    pub fn remove(&mut self, key: K) -> Option<V> {
        self.remove_entry(key).map(|(_, value)| value)
    }

    /// Removes `key`, returning the stored key and its value.
    #[inline]
    pub fn remove_entry(&mut self, key: K) -> Option<(K, V)> {
        let position = self.sparse.take(key.index(), self.len())?;
        Some(self.swap_remove(position))
    }

    /// Removes every entry, keeping allocations.
    pub fn clear(&mut self) {
        self.sparse.remove_all(self.keys.iter().map(|key| key.index()));
        self.keys.clear();
        self.values.clear();
    }

    /// Removes every entry, yielding them in dense order.
    ///
    /// The map is empty once this returns, even if the iterator is leaked.
    /// Allocations are kept.
    pub fn drain(&mut self) -> Drain<'_, K, V> {
        self.sparse.remove_all(self.keys.iter().map(|key| key.index()));
        Drain::new(self.keys.drain(..), self.values.drain(..))
    }

    /// Keeps only the entries for which `keep` returns `true`.
    ///
    /// Visits each entry once, in reverse dense order. Removed entries are
    /// replaced by already visited ones.
    pub fn retain(&mut self, mut keep: impl FnMut(K, &mut V) -> bool) {
        for position in (0..self.len()).rev() {
            if !keep(self.keys[position], &mut self.values[position]) {
                self.sparse.remove(self.keys[position].index());
                self.swap_remove(position);
            }
        }
    }

    /// Shrinks dense capacity to fit and releases sparse slots past the
    /// largest key index.
    pub fn shrink_to_fit(&mut self) {
        self.keys.shrink_to_fit();
        self.values.shrink_to_fit();
        let end = self.keys.iter().map(|key| key.index() + 1).max();
        self.sparse.shrink_to(end.unwrap_or(0));
    }

    /// Removes the entry at `position`, moving the last entry into its place.
    /// The removed key's slot must already be empty.
    #[inline]
    fn swap_remove(&mut self, position: usize) -> (K, V) {
        let key = self.keys.swap_remove(position);
        let value = self.values.swap_remove(position);
        if let Some(&moved) = self.keys.get(position) {
            self.sparse.set(moved.index(), position);
        }
        (key, value)
    }
}

impl<K, V> Default for SparseMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
