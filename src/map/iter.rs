use alloc::vec;
use core::fmt;
use core::iter::{FusedIterator, Zip};
use core::slice;

use super::SparseMap;

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
    inner: Zip<vec::IntoIter<K>, vec::IntoIter<V>>,
}

delegate_iterator!(IntoIter<K, V>, (K, V), |k, v| (k, v));

impl<K, V> fmt::Debug for IntoIter<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoIter").finish_non_exhaustive()
    }
}

/// Draining iterator over `(key, value)` in dense order.
///
/// Created by [`SparseMap::drain`]. Dropping it drops the remaining entries.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Drain<'a, K, V> {
    inner: Zip<vec::Drain<'a, K>, vec::Drain<'a, V>>,
}

impl<'a, K, V> Drain<'a, K, V> {
    pub(super) fn new(keys: vec::Drain<'a, K>, values: vec::Drain<'a, V>) -> Self {
        Self {
            inner: keys.zip(values),
        }
    }
}

delegate_iterator!(Drain<'a, K, V>, (K, V), |k, v| (k, v));

impl<K, V> fmt::Debug for Drain<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Drain").finish_non_exhaustive()
    }
}

impl<K: Copy, V> IntoIterator for SparseMap<K, V> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let (keys, values) = self.into_vecs();
        IntoIter {
            inner: keys.into_iter().zip(values),
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
