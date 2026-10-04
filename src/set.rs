//! A sparse set of keys.

use core::fmt;
use core::iter::{Copied, FusedIterator};
use core::slice;

use crate::Key;
use crate::map::{self, SparseMap};

/// A sparse set of [`Key`]s.
///
/// A [`SparseMap`] without values: keys live in one contiguous slice, with
/// O(1) insertion, removal and membership tests.
///
/// ```
/// use sparsley::SparseSet;
///
/// let mut alive = SparseSet::new();
/// assert!(alive.insert(5_u32));
/// assert!(!alive.insert(5));
/// alive.extend([1, 9]);
/// assert!(alive.remove(1));
/// assert_eq!(alive.as_slice(), [5, 9]);
/// ```
pub struct SparseSet<K> {
    map: SparseMap<K, ()>,
}

/// Iterator over the keys of a [`SparseSet`] in dense order.
pub type Iter<'a, K> = Copied<slice::Iter<'a, K>>;

/// Owning iterator over the keys of a [`SparseSet`] in dense order.
pub type IntoIter<K> = map::IntoKeys<K, ()>;

impl<K> SparseSet<K> {
    /// Creates an empty set without allocating.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            map: SparseMap::new(),
        }
    }

    /// Creates an empty set with room for `capacity` keys.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: SparseMap::with_capacity(capacity),
        }
    }

    /// Returns the number of keys.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the set has no keys.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Returns the number of keys the set holds without reallocating.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    /// Returns the exclusive bound of key indices that have a sparse slot.
    #[must_use]
    pub fn key_capacity(&self) -> usize {
        self.map.key_capacity()
    }

    /// Reserves room for at least `additional` more keys.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `isize::MAX` bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.map.reserve(additional);
    }

    /// Keys in dense order.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[K] {
        self.map.keys()
    }
}

impl<K: Copy> SparseSet<K> {
    /// Iterates over the keys in dense order.
    #[inline]
    pub fn iter(&self) -> Iter<'_, K> {
        self.as_slice().iter().copied()
    }
}

impl<K: Key> SparseSet<K> {
    /// Ensures every key with index below `end` has a sparse slot.
    pub fn reserve_keys(&mut self, end: usize) {
        self.map.reserve_keys(end);
    }

    /// Returns `true` if the set contains `key`.
    #[inline]
    #[must_use]
    pub fn contains(&self, key: K) -> bool {
        self.map.contains_key(key)
    }

    /// Returns the dense position of `key`.
    #[inline]
    #[must_use]
    pub fn position(&self, key: K) -> Option<usize> {
        self.map.position(key)
    }

    /// Adds `key`, returning `true` if it was absent.
    ///
    /// <!-- diagram: set-insert -->
    /// ```text
    /// set.insert(9) -> true
    ///             0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
    ///           ┌───┬───┬───┬───┬───┬───┬───┬───┬───┲━━━┱───┬───┬───┬───┬───┬───┐
    ///   sparse  │   │ 3 │   │ 1 │   │   │   │ 2 │   ┃ 4 ┃   │   │   │   │   │   │ …
    ///           └───┴───┴───┴───┴───┴───┴───┴───┴───┺━━━┹───┴───┴───┴───┴───┴───┘
    ///                         ╭───────────────────────╯
    ///             0   1   2   ▼
    ///           ┌───┬───┬───┲━━━┓
    ///   keys    │ 3 │ 7 │ 1 ┃ 9 ┃
    ///           └───┴───┴───┺━━━┛
    ///
    ///   len 3 -> 4, capacity 4, key_capacity 64
    ///   sparse[9]  0 -> 4  key 9 pushed at position 3
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the set already holds `u32::MAX` keys or allocation fails.
    #[inline]
    pub fn insert(&mut self, key: K) -> bool {
        self.map.insert(key, ()).is_none()
    }

    /// Removes `key`, returning `true` if it was present.
    ///
    /// The last key moves into the vacated position.
    ///
    /// <!-- diagram: set-remove -->
    /// ```text
    /// set.remove(3) -> true
    ///             0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
    ///           ┌───┲━━━┱───┲━━━┱───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
    ///   sparse  │   ┃ 1 ┃   ┃   ┃   │   │   │ 2 │   │   │   │   │   │   │   │   │ …
    ///           └───┺━━━┹───┺━━━┹───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
    ///             ╭───╯
    ///             ▼   1   2   3
    ///           ┏━━━┱───┲━━━┱───┐
    ///   keys    ┃ 1 ┃ 7 ┃░░░┃░░░│
    ///           ┗━━━┹───┺━━━┹───┘
    ///
    ///   len 3 -> 2, capacity 4, key_capacity 64
    ///   sparse[3]  1 -> 0  key 3 removed from position 0
    ///   sparse[1]  3 -> 1  key 1 moved from position 2 to 0
    /// ```
    #[inline]
    pub fn remove(&mut self, key: K) -> bool {
        self.map.remove(key).is_some()
    }

    /// Removes every key, keeping allocations.
    ///
    /// <!-- diagram: set-clear -->
    /// ```text
    /// set.clear()
    ///             0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
    ///           ┌───┲━━━┱───┲━━━┱───┬───┬───┲━━━┱───┬───┬───┬───┬───┬───┬───┬───┐
    ///   sparse  │   ┃   ┃   ┃   ┃   │   │   ┃   ┃   │   │   │   │   │   │   │   │ …
    ///           └───┺━━━┹───┺━━━┹───┴───┴───┺━━━┹───┴───┴───┴───┴───┴───┴───┴───┘
    ///             0   1   2   3
    ///           ┏━━━┳━━━┳━━━┱───┐
    ///   keys    ┃░░░┃░░░┃░░░┃░░░│
    ///           ┗━━━┻━━━┻━━━┹───┘
    ///
    ///   len 3 -> 0, capacity 4, key_capacity 64
    ///   sparse[3]  1 -> 0  key 3 removed from position 0
    ///   sparse[7]  2 -> 0  key 7 removed from position 1
    ///   sparse[1]  3 -> 0  key 1 removed from position 2
    /// ```
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Removes every key, yielding them in dense order.
    pub fn drain(&mut self) -> Drain<'_, K> {
        Drain {
            inner: self.map.drain(),
        }
    }

    /// Keeps only the keys for which `keep` returns `true`.
    ///
    /// Visits each key once, in reverse dense order.
    ///
    /// <!-- diagram: set-retain -->
    /// ```text
    /// set.retain(|key| key > 5)
    ///             0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
    ///           ┌───┲━━━┱───┲━━━┱───┬───┬───┲━━━┱───┬───┬───┬───┬───┬───┬───┬───┐
    ///   sparse  │   ┃   ┃   ┃   ┃   │   │   ┃ 1 ┃   │   │   │   │   │   │   │   │ …
    ///           └───┺━━━┹───┺━━━┹───┴───┴───┺━━━┹───┴───┴───┴───┴───┴───┴───┴───┘
    ///             ╭───────────────────────────╯
    ///             ▼   1   2   3
    ///           ┏━━━┳━━━┳━━━┱───┐
    ///   keys    ┃ 7 ┃░░░┃░░░┃░░░│
    ///           ┗━━━┻━━━┻━━━┹───┘
    ///
    ///   len 3 -> 1, capacity 4, key_capacity 64
    ///   sparse[3]  1 -> 0  key 3 removed from position 0
    ///   sparse[1]  3 -> 0  key 1 removed from position 2
    ///   sparse[7]  2 -> 1  key 7 moved from position 1 to 0
    /// ```
    pub fn retain(&mut self, mut keep: impl FnMut(K) -> bool) {
        self.map.retain(|key, ()| keep(key));
    }

    /// Shrinks dense capacity to fit and releases sparse slots past the
    /// largest key index.
    pub fn shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();
    }

    /// Sorts the keys in dense order.
    ///
    /// ```
    /// # use sparsley::SparseSet;
    /// let mut set = SparseSet::from([5_u32, 1, 3]);
    /// set.sort_unstable();
    /// assert_eq!(set.as_slice(), [1, 3, 5]);
    /// ```
    pub fn sort_unstable(&mut self)
    where
        K: Ord,
    {
        self.map.sort_unstable_by(|(a, ()), (b, ())| a.cmp(&b));
    }

    /// Returns `true` if every key of `self` is in `other`.
    #[must_use]
    pub fn is_subset(&self, other: &Self) -> bool {
        self.len() <= other.len() && self.iter().all(|key| other.contains(key))
    }

    /// Returns `true` if `self` and `other` share no keys.
    #[must_use]
    pub fn is_disjoint(&self, other: &Self) -> bool {
        let (small, large) = if self.len() <= other.len() {
            (self, other)
        } else {
            (other, self)
        };
        !small.iter().any(|key| large.contains(key))
    }
}

/// Draining iterator over the keys of a [`SparseSet`].
///
/// Created by [`SparseSet::drain`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Drain<'a, K> {
    inner: map::Drain<'a, K, ()>,
}

impl<K: Copy> Iterator for Drain<'_, K> {
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<K> {
        self.inner.next().map(|(key, ())| key)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<K: Copy> DoubleEndedIterator for Drain<'_, K> {
    #[inline]
    fn next_back(&mut self) -> Option<K> {
        self.inner.next_back().map(|(key, ())| key)
    }
}

impl<K: Copy> ExactSizeIterator for Drain<'_, K> {}

impl<K: Copy> FusedIterator for Drain<'_, K> {}

impl<K> fmt::Debug for Drain<'_, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Drain").finish_non_exhaustive()
    }
}

impl<K: Key> Clone for SparseSet<K> {
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
        }
    }
}

impl<K: fmt::Debug> fmt::Debug for SparseSet<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.as_slice()).finish()
    }
}

impl<K> Default for SparseSet<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// Sets are equal when they hold the same keys, regardless of dense order.
impl<K: Key> PartialEq for SparseSet<K> {
    fn eq(&self, other: &Self) -> bool {
        self.map == other.map
    }
}

impl<K: Key> Eq for SparseSet<K> {}

impl<K: Key> Extend<K> for SparseSet<K> {
    fn extend<I: IntoIterator<Item = K>>(&mut self, iter: I) {
        self.map.extend(iter.into_iter().map(|key| (key, ())));
    }
}

impl<'a, K: Key> Extend<&'a K> for SparseSet<K> {
    fn extend<I: IntoIterator<Item = &'a K>>(&mut self, iter: I) {
        self.extend(iter.into_iter().copied());
    }
}

impl<K: Key> FromIterator<K> for SparseSet<K> {
    fn from_iter<I: IntoIterator<Item = K>>(iter: I) -> Self {
        let mut set = Self::new();
        set.extend(iter);
        set
    }
}

impl<K: Key, const N: usize> From<[K; N]> for SparseSet<K> {
    fn from(keys: [K; N]) -> Self {
        keys.into_iter().collect()
    }
}

impl<K> IntoIterator for SparseSet<K> {
    type Item = K;
    type IntoIter = IntoIter<K>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.into_keys()
    }
}

impl<'a, K: Copy> IntoIterator for &'a SparseSet<K> {
    type Item = K;
    type IntoIter = Iter<'a, K>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
