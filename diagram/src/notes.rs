use std::fmt::{self, Display, Write};

use crate::snapshot::{Entry, Slot, Snapshot};

/// How a call rewrote the slot of an entry.
enum Change {
    Removed { from: usize },
    Moved { from: usize, to: usize },
    Pushed { to: usize },
}

/// A rewritten slot and the entry it belongs to.
struct SlotChange<'a> {
    entry: &'a Entry,
    change: Change,
}

impl Change {
    /// Returns the slot before and after the call.
    fn slots(&self) -> (Slot, Slot) {
        match *self {
            Self::Removed { from } => (Slot::Position(from), Slot::Empty),
            Self::Moved { from, to } => (Slot::Position(from), Slot::Position(to)),
            Self::Pushed { to } => (Slot::Empty, Slot::Position(to)),
        }
    }
}

impl Display for SlotChange<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = &self.entry.key;
        match self.change {
            Change::Removed { from } => write!(f, "key {key} removed from position {from}"),
            Change::Moved { from, to } => write!(f, "key {key} moved from position {from} to {to}"),
            Change::Pushed { to } => write!(f, "key {key} pushed at position {to}"),
        }
    }
}

/// Lists removed entries in old dense order, then moved and pushed entries
/// in new dense order.
fn slot_changes<'a>(before: &'a Snapshot, after: &'a Snapshot) -> Vec<SlotChange<'a>> {
    let removed = before
        .entries
        .iter()
        .enumerate()
        .filter_map(|(from, entry)| {
            let change = Change::Removed { from };
            after
                .position(entry.index)
                .is_none()
                .then_some(SlotChange { entry, change })
        });
    let placed = after.entries.iter().enumerate().filter_map(|(to, entry)| {
        let change = match before.position(entry.index) {
            None => Change::Pushed { to },
            Some(from) if from != to => Change::Moved { from, to },
            Some(_) => return None,
        };
        Some(SlotChange { entry, change })
    });
    removed.chain(placed).collect()
}

/// Writes `len` and both capacities, showing any that changed.
pub(crate) fn summary(out: &mut String, before: &Snapshot, after: &Snapshot) {
    let change = |old: usize, new: usize| {
        if old == new {
            new.to_string()
        } else {
            format!("{old} -> {new}")
        }
    };
    let _ = writeln!(
        out,
        "\n  len {}, capacity {}, key_capacity {}",
        change(before.len(), after.len()),
        change(before.capacity, after.capacity),
        change(before.key_capacity, after.key_capacity),
    );
}

/// Writes one line per rewritten slot, the lookup of `focus` if its slot was
/// not rewritten, and one line per replaced value.
pub(crate) fn notes(out: &mut String, before: &Snapshot, after: &Snapshot, focus: Option<usize>) {
    let changes = slot_changes(before, after);
    let target = |index| format!("sparse[{index}]");
    let width = changes
        .iter()
        .map(|change| target(change.entry.index).len())
        .max()
        .unwrap_or(0);
    for change in &changes {
        let (old, new) = change.change.slots();
        let _ = writeln!(
            out,
            "  {:<width$}  {} -> {}  {change}",
            target(change.entry.index),
            old.stored(),
            new.stored(),
        );
    }
    let rewritten = |index| changes.iter().any(|change| change.entry.index == index);
    if let Some(index) = focus.filter(|&index| !rewritten(index)) {
        lookup(out, after, index);
    }
    replacements(out, before, after);
}

fn lookup(out: &mut String, after: &Snapshot, index: usize) {
    let position = after.position(index);
    let stored = Slot::from(position).stored();
    let _ = match position {
        Some(position) => {
            let Entry { key, value, .. } = &after.entries[position];
            writeln!(
                out,
                "  sparse[{index}] = {stored} -> keys[{position}] = {key}, values[{position}] = {value}"
            )
        }
        None => writeln!(out, "  sparse[{index}] = {stored} -> no entry"),
    };
}

/// Writes entries that kept their position but changed value.
fn replacements(out: &mut String, before: &Snapshot, after: &Snapshot) {
    for (position, new) in after.entries.iter().enumerate() {
        let Some(old) = before.entries.get(position) else {
            continue;
        };
        if old.index == new.index && old.value != new.value {
            let _ = writeln!(
                out,
                "  values[{position}]  {} -> {}  key {} replaced",
                old.value, new.value, new.key
            );
        }
    }
}
