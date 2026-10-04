use std::fmt::Write;
use std::iter;

use crate::COLUMNS;
use crate::snapshot::{Entry, Slot, Snapshot};

/// Fill for unused dense capacity.
const SHADE: char = '░';

/// The width of the row labels, including their indent.
const LABEL: usize = 10;

/// A drawn table cell.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Cell {
    /// An empty sparse slot.
    Empty,
    /// Dense capacity past `len`.
    Unused,
    Text(String),
}

/// A labelled row of cells, each flagged if the call changed it.
struct Row {
    label: &'static str,
    cells: Vec<(Cell, bool)>,
}

/// Rows sharing a header of column indices.
pub(crate) struct Table {
    rows: Vec<Row>,
    /// Whether columns past [`COLUMNS`] are left out.
    truncated: bool,
}

impl Cell {
    fn width(&self) -> usize {
        match self {
            Self::Text(text) => text.chars().count(),
            Self::Empty | Self::Unused => 0,
        }
    }

    /// Writes the cell right-aligned in `width`, then its change mark.
    fn draw(&self, out: &mut String, width: usize, changed: bool) {
        let text = match self {
            Self::Empty => String::new(),
            Self::Unused => iter::repeat_n(SHADE, width).collect(),
            Self::Text(text) => text.clone(),
        };
        let mark = match self {
            _ if changed => '*',
            Self::Unused => SHADE,
            _ => ' ',
        };
        let _ = write!(out, "{text:>width$}{mark}│");
    }
}

impl From<Slot> for Cell {
    fn from(slot: Slot) -> Self {
        match slot {
            Slot::Empty => Self::Empty,
            Slot::Position(_) => Self::Text(slot.stored().to_string()),
        }
    }
}

impl Row {
    /// A row of `cells`, flagging those that differ from `old`.
    fn diff(label: &'static str, cells: Vec<Cell>, old: &[Cell]) -> Self {
        let cells = cells
            .into_iter()
            .enumerate()
            .map(|(column, cell)| {
                let changed = old.get(column) != Some(&cell);
                (cell, changed)
            })
            .collect();
        Self { label, cells }
    }

    fn draw(&self, out: &mut String, width: usize, truncated: bool) {
        let _ = write!(out, "  {:<8}│", self.label);
        for (cell, changed) in &self.cells {
            cell.draw(out, width, *changed);
        }
        if truncated {
            out.push_str(" …");
        }
        out.push('\n');
    }
}

impl Table {
    /// The drawn sparse slots.
    pub(crate) fn sparse(before: &Snapshot, after: &Snapshot) -> Self {
        let cells = |snapshot: &Snapshot| {
            snapshot
                .slots
                .iter()
                .map(|&slot| Cell::from(slot))
                .collect()
        };
        Self {
            rows: vec![Row::diff("sparse", cells(after), &cells(before))],
            truncated: after.key_capacity > after.slots.len(),
        }
    }

    /// The drawn dense keys and values, up to capacity.
    pub(crate) fn dense(before: &Snapshot, after: &Snapshot) -> Self {
        let row = |label, field: fn(&Entry) -> &str| {
            let cells = |snapshot: &Snapshot| -> Vec<Cell> {
                (0..snapshot.capacity.min(COLUMNS))
                    .map(|position| match snapshot.entries.get(position) {
                        Some(entry) => Cell::Text(field(entry).to_owned()),
                        None => Cell::Unused,
                    })
                    .collect()
            };
            Row::diff(label, cells(after), &cells(before))
        };
        Self {
            rows: vec![
                row("keys", |entry| &entry.key),
                row("values", |entry| &entry.value),
            ],
            truncated: after.capacity > COLUMNS,
        }
    }

    fn columns(&self) -> usize {
        self.rows[0].cells.len()
    }

    /// Returns the text width that fits every cell and column index.
    pub(crate) fn width(&self) -> usize {
        let cells = self.rows.iter().flat_map(|row| &row.cells);
        let widest = cells.map(|(cell, _)| cell.width()).max().unwrap_or(0);
        let index = self.columns().saturating_sub(1).to_string().len();
        widest.max(index).max(2)
    }

    pub(crate) fn draw(&self, out: &mut String, width: usize) {
        let columns = self.columns();
        if columns == 0 {
            for row in &self.rows {
                let _ = writeln!(out, "  {:<8}(none)", row.label);
            }
            return;
        }
        let _ = write!(out, "{:LABEL$} ", "");
        for column in 0..columns {
            let _ = write!(out, "{column:>width$}  ");
        }
        out.push('\n');
        let border = |out: &mut String, [left, middle, right]: [&str; 3]| {
            let line = vec!["─".repeat(width + 1); columns].join(middle);
            let _ = writeln!(out, "{:LABEL$}{left}{line}{right}", "");
        };
        border(out, ["┌", "┬", "┐"]);
        for (i, row) in self.rows.iter().enumerate() {
            if i > 0 {
                border(out, ["├", "┼", "┤"]);
            }
            row.draw(out, width, self.truncated);
        }
        border(out, ["└", "┴", "┘"]);
    }
}
