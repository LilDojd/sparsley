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

    /// Returns the key at dense `index`.
    #[inline]
    #[must_use]
    pub fn get_index(&self, index: usize) -> Option<K> {
        self.as_slice().get(index).copied()
    }
}

impl<K: Key> SparseSet<K> {
    /// Ensures every key whose slot is below `end` has a sparse slot.
    pub fn reserve_keys(&mut self, end: usize) {
        self.map.reserve_keys(end);
    }

    /// Returns `true` if the set contains `key`.
    #[inline]
    #[must_use]
    #[doc(alias = "contains_key")]
    pub fn contains(&self, key: K) -> bool {
        self.map.contains_key(key)
    }

    /// Returns the dense index of `key`.
    #[inline]
    #[must_use]
    #[doc(alias = "position", alias = "index_of")]
    pub fn get_index_of(&self, key: K) -> Option<usize> {
        self.map.get_index_of(key)
    }

    /// Adds `key`, returning `true` if it was absent.
    ///
    #[doc = sparsley_diagram::diagram! {
        let mut set = SparseSet::from([3, 7, 1]);
        set.insert(9);
    }]
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
    /// The last key moves into the gap.
    ///
    #[doc = sparsley_diagram::diagram! {
        let mut set = SparseSet::from([3, 7, 1]);
        set.remove(3);
    }]
    #[inline]
    pub fn remove(&mut self, key: K) -> bool {
        self.map.remove(key).is_some()
    }

    /// Removes the key at dense `index`, moving the last key into the gap.
    pub fn swap_remove_index(&mut self, index: usize) -> Option<K> {
        self.map.swap_remove_index(index).map(|(key, ())| key)
    }

    /// Swaps the keys at dense indices `a` and `b`.
    ///
    /// # Panics
    ///
    /// Panics if either index is out of bounds.
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        self.map.swap_indices(a, b);
    }

    /// Removes every key, keeping allocations.
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
        self.map.sort_unstable_keys();
    }

    /// Returns `true` if every key of `self` is in `other`.
    #[must_use]
    pub fn is_subset(&self, other: &Self) -> bool {
        self.len() <= other.len() && self.iter().all(|key| other.contains(key))
    }

    /// Returns `true` if every key of `other` is in `self`.
    #[must_use]
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    /// Iterates over the keys in both `self` and `other`, walking the smaller
    /// set in dense order.
    ///
    /// ```
    /// # use sparsley::SparseSet;
    /// let a = SparseSet::from([1_u32, 2, 3]);
    /// let b = SparseSet::from([2_u32, 3, 4, 5]);
    /// assert_eq!(a.intersection(&b).collect::<Vec<_>>(), [2, 3]);
    /// ```
    pub fn intersection<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = K> + 'a {
        let (small, large) = if self.len() <= other.len() {
            (self, other)
        } else {
            (other, self)
        };
        small.iter().filter(move |&key| large.contains(key))
    }

    /// Iterates over the keys in `self` but not in `other`, in dense order.
    ///
    /// ```
    /// # use sparsley::SparseSet;
    /// let a = SparseSet::from([1_u32, 2, 3]);
    /// let b = SparseSet::from([2_u32, 4]);
    /// assert_eq!(a.difference(&b).collect::<Vec<_>>(), [1, 3]);
    /// ```
    pub fn difference<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = K> + 'a {
        self.iter().filter(move |&key| !other.contains(key))
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

/// Owning iterator over the keys of a [`SparseSet`] in dense order.
///
/// Created by [`SparseSet::into_iter`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct IntoIter<K> {
    inner: map::IntoKeys<K, ()>,
}

macro_rules! key_iterator {
    ($name:ident<$($lt:lifetime,)? K>, $next:expr) => {
        impl<$($lt,)? K> Iterator for $name<$($lt,)? K> {
            type Item = K;

            #[inline]
            fn next(&mut self) -> Option<K> {
                self.inner.next().map($next)
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.inner.size_hint()
            }
        }

        impl<$($lt,)? K> DoubleEndedIterator for $name<$($lt,)? K> {
            #[inline]
            fn next_back(&mut self) -> Option<K> {
                self.inner.next_back().map($next)
            }
        }

        impl<$($lt,)? K> ExactSizeIterator for $name<$($lt,)? K> {}

        impl<$($lt,)? K> FusedIterator for $name<$($lt,)? K> {}

        impl<$($lt,)? K> fmt::Debug for $name<$($lt,)? K> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }
    };
}

key_iterator!(Drain<'a, K>, key_of);
key_iterator!(IntoIter<K>, core::convert::identity);

fn key_of<K>((key, ()): (K, ())) -> K {
    key
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
        IntoIter {
            inner: self.map.into_keys(),
        }
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
