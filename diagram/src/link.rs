use std::cmp::Ordering;
use std::fmt::Write;
use std::iter;

use crate::render::COLUMNS;
use crate::snapshot::Snapshot;
use crate::table::anchor;

/// A line from a sparse slot down to the dense position it holds.
pub(crate) struct Link {
    slot: usize,
    pub(crate) position: usize,
}

impl Link {
    /// Links the `focus` key if it is live after the call, otherwise the one
    /// entry the call moved, if both ends are drawn.
    pub(crate) fn find(before: &Snapshot, after: &Snapshot, focus: Option<usize>) -> Option<Self> {
        let live = focus.filter(|&index| after.position(index).is_some());
        let index = live.or_else(|| moved(before, after))?;
        let link = Self {
            slot: index,
            position: after.position(index)?,
        };
        let drawn = link.slot < after.slots.len() && link.position < after.capacity.min(COLUMNS);
        drawn.then_some(link)
    }

    /// Draws the line between the tables; the dense header draws its head.
    pub(crate) fn draw(&self, out: &mut String, width: usize) {
        let (from, to) = (anchor(self.slot, width), anchor(self.position, width));
        let (start, line) = match from.cmp(&to) {
            Ordering::Equal => (from, "│".to_owned()),
            Ordering::Greater => (to, format!("╭{}╯", dashes(from - to - 1))),
            Ordering::Less => (from, format!("╰{}╮", dashes(to - from - 1))),
        };
        let _ = writeln!(out, "{:start$}{line}", "");
    }
}

fn dashes(count: usize) -> String {
    iter::repeat_n('─', count).collect()
}

/// Returns the index of the only entry whose position changed, if exactly one
/// did.
fn moved(before: &Snapshot, after: &Snapshot) -> Option<usize> {
    let mut moved = after.entries.iter().enumerate().filter_map(|(to, entry)| {
        let from = before.position(entry.index)?;
        (from != to).then_some(entry.index)
    });
    let index = moved.next()?;
    moved.next().is_none().then_some(index)
}
