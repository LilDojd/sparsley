use core::fmt;
use core::mem;

use super::SparseMap;
use crate::Key;

/// A view into a single entry of a [`SparseMap`].
///
/// Created by [`SparseMap::entry`].
pub enum Entry<'a, K, V> {
    /// The key is present.
    Occupied(OccupiedEntry<'a, K, V>),
    /// The key is absent.
    Vacant(VacantEntry<'a, K, V>),
}

/// An entry whose key is present.
pub struct OccupiedEntry<'a, K, V> {
    map: &'a mut SparseMap<K, V>,
    index: usize,
}

/// An entry whose key is absent.
pub struct VacantEntry<'a, K, V> {
    map: &'a mut SparseMap<K, V>,
    key: K,
    slot: usize,
}

impl<'a, K: Key, V> Entry<'a, K, V> {
    pub(super) fn new(map: &'a mut SparseMap<K, V>, key: K) -> Self {
        let slot = key.slot();
        map.sparse.reserve(slot, super::indices(map.dense.keys()));
        let len = map.len();
        let index = map.sparse.probe(slot, len);
        match index {
            Some(index) => Self::Occupied(OccupiedEntry { map, index }),
            None => Self::Vacant(VacantEntry { map, key, slot }),
        }
    }

    /// Returns the entry's key.
    #[must_use]
    pub fn key(&self) -> K {
        match self {
            Self::Occupied(entry) => entry.key(),
            Self::Vacant(entry) => entry.key(),
        }
    }

    /// Inserts `default` if vacant, returning the value.
    #[inline]
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default),
        }
    }

    /// Inserts the result of `default` if vacant, returning the value.
    #[inline]
    pub fn or_insert_with(self, default: impl FnOnce() -> V) -> &'a mut V {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Inserts the result of `default(key)` if vacant, returning the value.
    #[inline]
    pub fn or_insert_with_key(self, default: impl FnOnce(K) -> V) -> &'a mut V {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => {
                let value = default(entry.key);
                entry.insert(value)
            }
        }
    }

    /// Calls `f` on the value if occupied.
    #[inline]
    #[must_use]
    pub fn and_modify(mut self, f: impl FnOnce(&mut V)) -> Self {
        if let Self::Occupied(entry) = &mut self {
            f(entry.get_mut());
        }
        self
    }
}

impl<'a, K: Key, V: Default> Entry<'a, K, V> {
    /// Inserts `V::default()` if vacant, returning the value.
    #[inline]
    pub fn or_default(self) -> &'a mut V {
        self.or_insert_with(V::default)
    }
}

impl<'a, K: Key, V> OccupiedEntry<'a, K, V> {
    /// Returns the stored key.
    #[must_use]
    pub fn key(&self) -> K {
        self.map.keys()[self.index]
    }

    /// Returns the dense index of the entry.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns a reference to the value.
    #[must_use]
    pub fn get(&self) -> &V {
        &self.map.values()[self.index]
    }

    /// Returns a mutable reference to the value.
    #[must_use]
    pub fn get_mut(&mut self) -> &mut V {
        &mut self.map.values_mut()[self.index]
    }

    /// Converts the entry into a mutable reference bound to the map.
    #[must_use]
    pub fn into_mut(self) -> &'a mut V {
        &mut self.map.values_mut()[self.index]
    }

    /// Replaces the value, returning the previous one.
    #[inline]
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }

    /// Removes the entry, returning its value.
    #[inline]
    pub fn remove(self) -> V {
        self.remove_entry().1
    }

    /// Removes the entry, returning the stored key and its value.
    #[inline]
    pub fn remove_entry(self) -> (K, V) {
        self.map.sparse.remove(self.key().slot());
        self.map.swap_remove(self.index)
    }
}

impl<'a, K: Key, V> VacantEntry<'a, K, V> {
    /// Returns the key that would be inserted.
    #[must_use]
    pub fn key(&self) -> K {
        self.key
    }

    /// Inserts `value`, returning a mutable reference to it.
    ///
    /// # Panics
    ///
    /// Panics if the map already holds `u32::MAX` entries or allocation fails.
    #[inline]
    pub fn insert(self, value: V) -> &'a mut V {
        let map = self.map;
        let index = map.dense.len();
        let value = map.dense.push(self.key, value);
        // SAFETY: `Entry::new` reserved `slot`, and `index` indexes a
        // pushed entry, so it is below `MAX_LEN`.
        unsafe { map.sparse.set_unchecked(self.slot, index) };
        value
    }
}

impl<K: Key + fmt::Debug, V: fmt::Debug> fmt::Debug for Entry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Occupied(entry) => f.debug_tuple("Occupied").field(entry).finish(),
            Self::Vacant(entry) => f.debug_tuple("Vacant").field(entry).finish(),
        }
    }
}

impl<K: Key + fmt::Debug, V: fmt::Debug> fmt::Debug for OccupiedEntry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", &self.key())
            .field("value", self.get())
            .finish()
    }
}

impl<K: Key + fmt::Debug, V> fmt::Debug for VacantEntry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(&self.key).finish()
    }
}
