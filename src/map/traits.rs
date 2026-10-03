use core::fmt;
use core::ops::{Index, IndexMut};

use super::SparseMap;
use crate::Key;
use crate::dense::MAX_LEN;

impl<K: Key, V: Clone> Clone for SparseMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            sparse: self.sparse.clone_with(super::indices(self.dense.keys())),
            dense: self.dense.clone(),
        }
    }
}

impl<K: Copy + fmt::Debug, V: fmt::Debug> fmt::Debug for SparseMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// Maps are equal when they hold the same entries, regardless of dense order.
impl<K: Key, V: PartialEq> PartialEq for SparseMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .all(|(key, value)| other.get(key) == Some(value))
    }
}

impl<K: Key, V: Eq> Eq for SparseMap<K, V> {}

impl<K: Key, V> Index<K> for SparseMap<K, V> {
    type Output = V;

    /// # Panics
    ///
    /// Panics if `key` is absent.
    #[inline]
    #[track_caller]
    fn index(&self, key: K) -> &V {
        self.get(key).expect("key not present in SparseMap")
    }
}

impl<K: Key, V> IndexMut<K> for SparseMap<K, V> {
    /// # Panics
    ///
    /// Panics if `key` is absent.
    #[inline]
    #[track_caller]
    fn index_mut(&mut self, key: K) -> &mut V {
        self.get_mut(key).expect("key not present in SparseMap")
    }
}

impl<K: Key, V> Extend<(K, V)> for SparseMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        // Duplicate keys may make the hint overshoot, as in `HashMap`.
        let hint = iter.size_hint().0;
        let additional = if self.is_empty() { hint } else { hint.div_ceil(2) };
        self.reserve(additional.min(MAX_LEN - self.len()));
        iter.for_each(|(key, value)| {
            self.insert(key, value);
        });
    }
}

impl<'a, K: Key, V: Copy> Extend<(K, &'a V)> for SparseMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, &'a V)>>(&mut self, iter: I) {
        self.extend(iter.into_iter().map(|(key, &value)| (key, value)));
    }
}

impl<K: Key, V> FromIterator<(K, V)> for SparseMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl<K: Key, V, const N: usize> From<[(K, V); N]> for SparseMap<K, V> {
    fn from(entries: [(K, V); N]) -> Self {
        entries.into_iter().collect()
    }
}
