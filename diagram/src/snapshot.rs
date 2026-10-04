/// The observable state of a map or set, as drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub(crate) kind: Kind,
    /// The first [`COLUMNS`](crate::render::COLUMNS) sparse slots.
    pub(crate) slots: Vec<Slot>,
    pub(crate) key_capacity: usize,
    /// The dense entries, in position order.
    pub(crate) entries: Vec<Entry>,
    pub(crate) capacity: usize,
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

impl Snapshot {
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
