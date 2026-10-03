//! A sparse map with packed, contiguous values.

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::mem;

use crate::Key;
use crate::dense::Dense;
use crate::sparse::Sparse;

mod entry;
mod iter;
mod traits;

pub use entry::{Entry, OccupiedEntry, VacantEntry};
pub use iter::{Drain, IntoIter, IntoKeys, IntoValues, Iter, IterMut};

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
    dense: Dense<K, V>,
}

impl<K, V> SparseMap<K, V> {
    /// Creates an empty map without allocating.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sparse: Sparse::new(),
            dense: Dense::new(),
        }
    }

    /// Creates an empty map with room for `capacity` entries.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            sparse: Sparse::new(),
            dense: Dense::with_capacity(capacity),
        }
    }

    /// Returns the number of entries.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.dense.len()
    }

    /// Returns `true` if the map has no entries.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dense.len() == 0
    }

    /// Returns the number of entries the map holds without reallocating.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.dense.capacity()
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
    /// Panics if the capacity would exceed `u32::MAX` entries or `isize::MAX`
    /// bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.dense.reserve(additional);
    }

    /// Keys in dense order.
    #[inline]
    #[must_use]
    pub fn keys(&self) -> &[K] {
        self.dense.keys()
    }

    /// Values in dense order.
    #[inline]
    #[must_use]
    pub fn values(&self) -> &[V] {
        self.dense.values()
    }

    /// Mutable values in dense order.
    ///
    /// Keys stay fixed: reordering this slice reassigns values to keys.
    #[inline]
    #[must_use]
    pub fn values_mut(&mut self) -> &mut [V] {
        self.dense.values_mut()
    }

    /// Keys and values in dense order.
    #[inline]
    #[must_use]
    pub fn as_slices(&self) -> (&[K], &[V]) {
        (self.dense.keys(), self.dense.values())
    }

    /// Keys and mutable values in dense order.
    #[inline]
    #[must_use]
    pub fn as_mut_slices(&mut self) -> (&[K], &mut [V]) {
        self.dense.slices_mut()
    }
}

impl<K: Copy, V> SparseMap<K, V> {
    /// Returns the entry at dense `position`.
    #[inline]
    #[must_use]
    pub fn get_index(&self, position: usize) -> Option<(K, &V)> {
        Some((*self.keys().get(position)?, self.values().get(position)?))
    }

    /// Returns the entry at dense `position` with a mutable value.
    #[inline]
    #[must_use]
    pub fn get_index_mut(&mut self, position: usize) -> Option<(K, &mut V)> {
        let (keys, values) = self.dense.slices_mut();
        Some((*keys.get(position)?, values.get_mut(position)?))
    }

    /// Iterates over `(key, &value)` in dense order.
    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::new(self.dense.keys(), self.dense.values())
    }

    /// Iterates over `(key, &mut value)` in dense order.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        let (keys, values) = self.dense.slices_mut();
        IterMut::new(keys, values)
    }
}

impl<K: Key, V> SparseMap<K, V> {
    /// Ensures every key with index below `end` has a sparse slot.
    pub fn reserve_keys(&mut self, end: usize) {
        if let Some(last) = end.checked_sub(1) {
            self.sparse.reserve(last, indices(self.dense.keys()));
        }
    }

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
        self.position(key).is_some()
    }

    /// Returns a reference to the value of `key`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sparsley::SparseMap;
    /// let map = SparseMap::from([(1_u32, "a")]);
    /// assert_eq!(map.get(1), Some(&"a"));
    /// assert_eq!(map.get(2), None);
    /// assert_eq!(map.get(u32::MAX), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn get(&self, key: K) -> Option<&V> {
        let position = self.position(key)?;
        // SAFETY: `position` only returns positions below `len`.
        Some(unsafe { self.dense.values().get_unchecked(position) })
    }

    /// Returns a mutable reference to the value of `key`.
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        let position = self.position(key)?;
        // SAFETY: `position` only returns positions below `len`.
        Some(unsafe { self.dense.values_mut().get_unchecked_mut(position) })
    }

    /// Returns mutable references to the values of `N` keys at once.
    ///
    /// Each element is `None` if its key is absent.
    ///
    /// # Panics
    ///
    /// Panics if two present keys are equal.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sparsley::SparseMap;
    /// let mut health = SparseMap::from([(1_u32, 100), (2, 80)]);
    /// let [a, b, c] = health.get_disjoint_mut([1, 2, 3]);
    /// core::mem::swap(a.unwrap(), b.unwrap());
    /// assert!(c.is_none());
    /// assert_eq!(health[1], 80);
    /// ```
    #[must_use]
    pub fn get_disjoint_mut<const N: usize>(&mut self, keys: [K; N]) -> [Option<&mut V>; N] {
        let positions = keys.map(|key| self.position(key));
        for (i, position) in positions.iter().enumerate() {
            assert!(
                position.is_none() || !positions[..i].contains(position),
                "duplicate keys in get_disjoint_mut"
            );
        }
        let values = self.dense.values_mut().as_mut_ptr();
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
    ///
    /// # Examples
    ///
    /// ```
    /// # use sparsley::SparseMap;
    /// let mut map = SparseMap::new();
    /// assert_eq!(map.insert(3_u32, "a"), None);
    /// assert_eq!(map.insert(3, "b"), Some("a"));
    /// assert_eq!(map[3], "b");
    /// ```
    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let len = self.dense.len();
        let Some(slot) = self.sparse.slot_mut(key.index()) else {
            return self.insert_slow(key, value);
        };
        if let Some(position) = slot.position(len) {
            // SAFETY: `position` only returns positions below `len`.
            let stored = unsafe { self.dense.values_mut().get_unchecked_mut(position) };
            return Some(mem::replace(stored, value));
        }
        if self.dense.is_full() {
            return self.insert_slow(key, value);
        }
        // SAFETY: the storage is not full, so `len` becomes a valid position
        // below `MAX_LEN`.
        unsafe {
            self.dense.push_unchecked(key, value);
            slot.set_unchecked(len);
        }
        None
    }

    #[cold]
    #[inline(never)]
    fn insert_slow(&mut self, key: K, value: V) -> Option<V> {
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
    ///
    /// # Examples
    ///
    /// ```
    /// # use sparsley::SparseMap;
    /// let mut counts = SparseMap::new();
    /// for key in [3_u32, 1, 3] {
    ///     *counts.entry(key).or_insert(0) += 1;
    /// }
    /// assert_eq!(counts[3], 2);
    /// ```
    #[inline]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        Entry::new(self, key)
    }

    /// Removes `key`, returning its value.
    ///
    /// The last entry moves into the vacated position.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sparsley::SparseMap;
    /// let mut map = SparseMap::from([(1_u32, "a"), (2, "b"), (3, "c")]);
    /// assert_eq!(map.remove(1), Some("a"));
    /// assert_eq!(map.remove(1), None);
    /// assert_eq!(map.keys(), [3, 2]);
    /// ```
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

    /// Removes the entry at dense `position`, moving the last entry into its
    /// place.
    ///
    /// ```
    /// # use sparsley::SparseMap;
    /// let mut map = SparseMap::from([(1_u32, 'a'), (2, 'b'), (3, 'c')]);
    /// assert_eq!(map.swap_remove_index(0), Some((1, 'a')));
    /// assert_eq!(map.keys(), [3, 2]);
    /// ```
    pub fn swap_remove_index(&mut self, position: usize) -> Option<(K, V)> {
        let key = *self.keys().get(position)?;
        self.sparse.remove(key.index());
        Some(self.swap_remove(position))
    }

    /// Swaps the entries at dense positions `a` and `b`.
    ///
    /// # Panics
    ///
    /// Panics if either position is out of bounds.
    ///
    /// ```
    /// # use sparsley::SparseMap;
    /// let mut map = SparseMap::from([(1_u32, 'a'), (2, 'b')]);
    /// map.swap_indices(0, 1);
    /// assert_eq!(map.keys(), [2, 1]);
    /// assert_eq!(map[1], 'a');
    /// ```
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        self.dense.swap(a, b);
        let keys = self.dense.keys();
        let (key_a, key_b) = (keys[a], keys[b]);
        self.sparse.set(key_a.index(), a);
        self.sparse.set(key_b.index(), b);
    }

    /// Removes every entry, keeping allocations.
    pub fn clear(&mut self) {
        self.sparse.remove_all(indices(self.dense.keys()));
        self.dense.clear();
    }

    /// Removes every entry, yielding them in dense order.
    ///
    /// The map is empty once this returns, even if the iterator is leaked.
    /// Allocations are kept.
    pub fn drain(&mut self) -> Drain<'_, K, V> {
        self.sparse.remove_all(indices(self.dense.keys()));
        Drain::new(&mut self.dense)
    }

    /// Keeps only the entries for which `keep` returns `true`.
    ///
    /// Visits each entry once, in reverse dense order. Removed entries are
    /// replaced by already visited ones.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sparsley::SparseMap;
    /// let mut map: SparseMap<u32, u32> = (0..8).map(|key| (key, key * 10)).collect();
    /// map.retain(|key, value| {
    ///     *value += 1;
    ///     key % 2 == 0
    /// });
    /// assert_eq!(map.len(), 4);
    /// assert_eq!(map[6], 61);
    /// ```
    pub fn retain(&mut self, mut keep: impl FnMut(K, &mut V) -> bool) {
        for position in (0..self.len()).rev() {
            let (keys, values) = self.dense.slices_mut();
            let key = keys[position];
            if !keep(key, &mut values[position]) {
                self.sparse.remove(key.index());
                self.swap_remove(position);
            }
        }
    }

    /// Sorts the entries in dense order with `compare`.
    ///
    /// Unstable, O(n log n), and allocates a permutation of `len` indices.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sparsley::SparseMap;
    /// let mut map = SparseMap::from([(5_u32, 'b'), (1, 'c'), (3, 'a')]);
    /// map.sort_unstable_by(|(a, _), (b, _)| a.cmp(&b));
    /// assert_eq!(map.keys(), [1, 3, 5]);
    /// assert_eq!(map[3], 'a');
    /// ```
    pub fn sort_unstable_by(&mut self, mut compare: impl FnMut((K, &V), (K, &V)) -> Ordering) {
        let (keys, values) = (self.dense.keys(), self.dense.values());
        let mut order: Vec<usize> = (0..self.len()).collect();
        order.sort_unstable_by(|&a, &b| compare((keys[a], &values[a]), (keys[b], &values[b])));
        for start in 0..order.len() {
            let mut target = start;
            loop {
                let source = mem::replace(&mut order[target], target);
                if source == start {
                    break;
                }
                self.dense.swap(target, source);
                target = source;
            }
        }
        for (position, key) in self.dense.keys().iter().enumerate() {
            self.sparse.set(key.index(), position);
        }
    }

    /// Shrinks dense capacity to fit and releases sparse slots past the
    /// largest key index.
    pub fn shrink_to_fit(&mut self) {
        self.dense.shrink_to_fit();
        let end = self.keys().iter().map(|key| key.index() + 1).max();
        self.sparse.shrink_to(end.unwrap_or(0));
    }

    /// Removes the entry at `position`, moving the last entry into its place.
    /// The removed key's slot must already be empty.
    #[inline]
    fn swap_remove(&mut self, position: usize) -> (K, V) {
        let entry = self.dense.swap_remove(position);
        if let Some(&moved) = self.keys().get(position) {
            self.sparse.set(moved.index(), position);
        }
        entry
    }
}

impl<K, V> Default for SparseMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// Sparse indices of `keys`, in dense order.
fn indices<K: Key>(keys: &[K]) -> impl ExactSizeIterator<Item = usize> + '_ {
    keys.iter().map(|key| key.index())
}
