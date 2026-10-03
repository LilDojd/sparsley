use core::fmt;
use core::iter::{FusedIterator, Zip};
use core::marker::PhantomData;
use core::slice;

use super::SparseMap;
use crate::dense::{Dense, RawEntries};

macro_rules! delegate_iterator {
    ($name:ident<$($lt:lifetime,)? $k:ident, $v:ident>, $item:ty, |$kp:pat_param, $vp:pat_param| $map:expr) => {
        impl<$($lt,)? $k: Copy, $v> Iterator for $name<$($lt,)? $k, $v> {
            type Item = $item;

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.inner.next().map(|($kp, $vp)| $map)
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.inner.size_hint()
            }

            #[inline]
            fn nth(&mut self, n: usize) -> Option<Self::Item> {
                self.inner.nth(n).map(|($kp, $vp)| $map)
            }

            #[inline]
            fn count(self) -> usize {
                self.inner.len()
            }

            #[inline]
            fn fold<B, F: FnMut(B, Self::Item) -> B>(self, init: B, mut f: F) -> B {
                self.inner.fold(init, |acc, ($kp, $vp)| f(acc, $map))
            }
        }

        impl<$($lt,)? $k: Copy, $v> DoubleEndedIterator for $name<$($lt,)? $k, $v> {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                self.inner.next_back().map(|($kp, $vp)| $map)
            }

            #[inline]
            fn rfold<B, F: FnMut(B, Self::Item) -> B>(self, init: B, mut f: F) -> B {
                self.inner.rfold(init, |acc, ($kp, $vp)| f(acc, $map))
            }
        }

        impl<$($lt,)? $k: Copy, $v> ExactSizeIterator for $name<$($lt,)? $k, $v> {
            #[inline]
            fn len(&self) -> usize {
                self.inner.len()
            }
        }

        impl<$($lt,)? $k: Copy, $v> FusedIterator for $name<$($lt,)? $k, $v> {}
    };
}

/// Iterator over `(key, &value)` in dense order.
///
/// Created by [`SparseMap::iter`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Iter<'a, K, V> {
    inner: Zip<slice::Iter<'a, K>, slice::Iter<'a, V>>,
}

impl<'a, K, V> Iter<'a, K, V> {
    pub(super) fn new(keys: &'a [K], values: &'a [V]) -> Self {
        Self {
            inner: keys.iter().zip(values),
        }
    }
}

delegate_iterator!(Iter<'a, K, V>, (K, &'a V), |&k, v| (k, v));

impl<K, V> Clone for Iter<'_, K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<K: Copy + fmt::Debug, V: fmt::Debug> fmt::Debug for Iter<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// Iterator over `(key, &mut value)` in dense order.
///
/// Created by [`SparseMap::iter_mut`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct IterMut<'a, K, V> {
    inner: Zip<slice::Iter<'a, K>, slice::IterMut<'a, V>>,
}

impl<'a, K, V> IterMut<'a, K, V> {
    pub(super) fn new(keys: &'a [K], values: &'a mut [V]) -> Self {
        Self {
            inner: keys.iter().zip(values),
        }
    }
}

delegate_iterator!(IterMut<'a, K, V>, (K, &'a mut V), |&k, v| (k, v));

impl<K, V> fmt::Debug for IterMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IterMut").finish_non_exhaustive()
    }
}

/// Owning iterator over `(key, value)` in dense order.
///
/// Created by [`SparseMap::into_iter`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct IntoIter<K, V> {
    entries: RawEntries<K, V>,
    _dense: Dense<K, V>,
}

/// Draining iterator over `(key, value)` in dense order.
///
/// Created by [`SparseMap::drain`]. Dropping it drops the remaining entries.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Drain<'a, K, V> {
    entries: RawEntries<K, V>,
    marker: PhantomData<&'a mut Dense<K, V>>,
}

impl<'a, K, V> Drain<'a, K, V> {
    pub(super) fn new(dense: &'a mut Dense<K, V>) -> Self {
        Self {
            entries: dense.take_entries(),
            marker: PhantomData,
        }
    }
}

macro_rules! owning_iterator {
    ($name:ident<$($lt:lifetime,)? K, V>) => {
        impl<$($lt,)? K, V> Iterator for $name<$($lt,)? K, V> {
            type Item = (K, V);

            #[inline]
            fn next(&mut self) -> Option<(K, V)> {
                // SAFETY: the entries stay allocated while `self` lives.
                unsafe { self.entries.next() }
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.entries.len(), Some(self.entries.len()))
            }

            #[inline]
            fn count(self) -> usize {
                self.entries.len()
            }
        }

        impl<$($lt,)? K, V> DoubleEndedIterator for $name<$($lt,)? K, V> {
            #[inline]
            fn next_back(&mut self) -> Option<(K, V)> {
                // SAFETY: the entries stay allocated while `self` lives.
                unsafe { self.entries.next_back() }
            }
        }

        impl<$($lt,)? K, V> ExactSizeIterator for $name<$($lt,)? K, V> {}

        impl<$($lt,)? K, V> FusedIterator for $name<$($lt,)? K, V> {}

        impl<$($lt,)? K, V> Drop for $name<$($lt,)? K, V> {
            fn drop(&mut self) {
                // SAFETY: the entries are still allocated and never read again.
                unsafe { self.entries.drop_remaining() }
            }
        }

        impl<$($lt,)? K, V> fmt::Debug for $name<$($lt,)? K, V> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("remaining", &self.entries.len())
                    .finish_non_exhaustive()
            }
        }
    };
}

owning_iterator!(IntoIter<K, V>);
owning_iterator!(Drain<'a, K, V>);

/// Owning iterator over keys in dense order.
///
/// Created by [`SparseMap::into_keys`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct IntoKeys<K, V> {
    inner: IntoIter<K, V>,
}

/// Owning iterator over values in dense order.
///
/// Created by [`SparseMap::into_values`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct IntoValues<K, V> {
    inner: IntoIter<K, V>,
}

macro_rules! projecting_iterator {
    ($name:ident, $item:ident, |$entry:ident| $project:expr) => {
        impl<K, V> Iterator for $name<K, V> {
            type Item = $item;

            #[inline]
            fn next(&mut self) -> Option<$item> {
                self.inner.next().map(|$entry| $project)
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.inner.size_hint()
            }

            #[inline]
            fn count(self) -> usize {
                self.inner.len()
            }
        }

        impl<K, V> DoubleEndedIterator for $name<K, V> {
            #[inline]
            fn next_back(&mut self) -> Option<$item> {
                self.inner.next_back().map(|$entry| $project)
            }
        }

        impl<K, V> ExactSizeIterator for $name<K, V> {}

        impl<K, V> FusedIterator for $name<K, V> {}
    };
}

projecting_iterator!(IntoKeys, K, |entry| entry.0);
projecting_iterator!(IntoValues, V, |entry| entry.1);

impl<K, V> SparseMap<K, V> {
    /// Consumes the map, yielding its keys in dense order.
    #[inline]
    pub fn into_keys(self) -> IntoKeys<K, V> {
        IntoKeys {
            inner: self.into_iter(),
        }
    }

    /// Consumes the map, yielding its values in dense order.
    #[inline]
    pub fn into_values(self) -> IntoValues<K, V> {
        IntoValues {
            inner: self.into_iter(),
        }
    }
}

impl<K, V> IntoIterator for SparseMap<K, V> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    #[inline]
    fn into_iter(self) -> IntoIter<K, V> {
        let mut dense = self.dense;
        IntoIter {
            entries: dense.take_entries(),
            _dense: dense,
        }
    }
}

impl<'a, K: Copy, V> IntoIterator for &'a SparseMap<K, V> {
    type Item = (K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K: Copy, V> IntoIterator for &'a mut SparseMap<K, V> {
    type Item = (K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
