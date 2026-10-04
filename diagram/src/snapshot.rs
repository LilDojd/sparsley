use std::fmt::Display;

use sparsley::{Key, SparseMap};

use crate::COLUMNS;

/// The observable state of a map, read through its public API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// The first [`COLUMNS`] sparse slots.
    pub(crate) slots: Vec<Slot>,
    pub(crate) key_capacity: usize,
    /// The dense entries, in position order.
    pub(crate) entries: Vec<Entry>,
    pub(crate) capacity: usize,
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
    pub(crate) value: String,
}

impl Snapshot {
    /// Reads the state of `map`.
    pub fn of<K, V>(map: &SparseMap<K, V>) -> Self
    where
        K: Key + TryFrom<usize> + Display,
        V: Display,
    {
        let slots = (0..map.key_capacity().min(COLUMNS))
            .map(|index| {
                let position = K::try_from(index).ok().and_then(|key| map.position(key));
                Slot::from(position)
            })
            .collect();
        let entries = map
            .iter()
            .map(|(key, value)| Entry {
                index: key.index(),
                key: key.to_string(),
                value: value.to_string(),
            })
            .collect();
        Self {
            slots,
            key_capacity: map.key_capacity(),
            entries,
            capacity: map.capacity(),
        }
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
