use std::fmt::Display;

use sparsley::{Key, SparseMap, SparseSet};

use crate::COLUMNS;

/// The observable state of a map or set, read through its public API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub(crate) kind: Kind,
    /// The first [`COLUMNS`] sparse slots.
    pub(crate) slots: Vec<Slot>,
    pub(crate) key_capacity: usize,
    /// The dense entries, in position order.
    pub(crate) entries: Vec<Entry>,
    pub(crate) capacity: usize,
}

/// A collection whose memory can be drawn.
pub trait Observe {
    /// Reads the current state.
    fn snapshot(&self) -> Snapshot;
}

impl<T: Observe + ?Sized> Observe for &T {
    fn snapshot(&self) -> Snapshot {
        (**self).snapshot()
    }
}

impl<T: Observe + ?Sized> Observe for &mut T {
    fn snapshot(&self) -> Snapshot {
        (**self).snapshot()
    }
}

/// Whether the dense storage holds values next to the keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Map,
    Set,
}

/// A sparse slot: empty, or pointing at a dense position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Empty,
    Position(usize),
}

/// A dense entry, rendered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    /// The key's sparse index.
    pub(crate) index: usize,
    pub(crate) key: String,
    /// The value, or `None` in a set.
    pub(crate) value: Option<String>,
}

impl<K, V> Observe for SparseMap<K, V>
where
    K: Key + TryFrom<usize> + Display,
    V: Display,
{
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            kind: Kind::Map,
            slots: slots(self.key_capacity(), |key| self.get_index_of(key)),
            key_capacity: self.key_capacity(),
            entries: self
                .iter()
                .map(|(key, value)| Entry::new(key, Some(value.to_string())))
                .collect(),
            capacity: self.capacity(),
        }
    }
}

impl<K> Observe for SparseSet<K>
where
    K: Key + TryFrom<usize> + Display,
{
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            kind: Kind::Set,
            slots: slots(self.key_capacity(), |key| self.get_index_of(key)),
            key_capacity: self.key_capacity(),
            entries: self.iter().map(|key| Entry::new(key, None)).collect(),
            capacity: self.capacity(),
        }
    }
}

/// Reads the first [`COLUMNS`] slots through `position`.
fn slots<K: TryFrom<usize>>(
    key_capacity: usize,
    position: impl Fn(K) -> Option<usize>,
) -> Vec<Slot> {
    (0..key_capacity.min(COLUMNS))
        .map(|index| Slot::from(K::try_from(index).ok().and_then(&position)))
        .collect()
}

impl Entry {
    fn new<K: Key + Display>(key: K, value: Option<String>) -> Self {
        Self {
            index: key.slot(),
            key: key.to_string(),
            value,
        }
    }
}

impl Snapshot {
    /// Reads the state of a map or set.
    pub fn of(collection: &impl Observe) -> Self {
        collection.snapshot()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns the dense position of the key with sparse `index`.
    pub(crate) fn position(&self, index: usize) -> Option<usize> {
        self.entries.iter().position(|entry| entry.index == index)
    }
}

impl Slot {
    /// Returns the value held in memory: `position + 1`, or zero when empty.
    pub(crate) fn stored(self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Position(position) => position + 1,
        }
    }
}

impl From<Option<usize>> for Slot {
    fn from(position: Option<usize>) -> Self {
        position.map_or(Self::Empty, Self::Position)
    }
}
