use std::fmt::Write;
use std::iter;

use crate::COLUMNS;
use crate::grid::{Line, glyph};
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

/// Rows sharing a header of column indices. Borders around changed cells are
/// heavy.
pub(crate) struct Table {
    rows: Vec<Row>,
    /// Whether columns past [`COLUMNS`] are left out.
    truncated: bool,
}

/// Returns the text column of the last character of cell `column`, where
/// links attach.
pub(crate) fn anchor(column: usize, width: usize) -> usize {
    LABEL + 1 + (width + 2) * column + width - 1
}

impl Cell {
    fn width(&self) -> usize {
        match self {
            Self::Text(text) => text.chars().count(),
            Self::Empty | Self::Unused => 0,
        }
    }

    /// Writes the cell right-aligned in `width`, plus one space of padding.
    fn draw(&self, out: &mut String, width: usize) {
        let _ = match self {
            Self::Empty => write!(out, "{:width$} ", ""),
            Self::Unused => write!(
                out,
                "{}",
                iter::repeat_n(SHADE, width + 1).collect::<String>()
            ),
            Self::Text(text) => write!(out, "{text:>width$} "),
        };
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

    /// Draws the table, with `▼` in place of the index of an `arrow` column.
    pub(crate) fn draw(&self, out: &mut String, width: usize, arrow: Option<usize>) {
        let columns = self.columns();
        if columns == 0 {
            for row in &self.rows {
                let _ = writeln!(out, "  {:<8}(none)", row.label);
            }
            return;
        }
        let _ = write!(out, "{:LABEL$} ", "");
        for column in 0..columns {
            let index = if arrow == Some(column) {
                "▼".to_owned()
            } else {
                column.to_string()
            };
            let _ = write!(out, "{index:>width$}  ");
        }
        out.push('\n');
        for border in 0..=self.rows.len() {
            self.draw_border(out, width, border);
            if let Some(row) = self.rows.get(border) {
                self.draw_row(out, width, border, row);
            }
        }
    }

    /// Draws horizontal border `border`, which lies above row `border`.
    fn draw_border(&self, out: &mut String, width: usize, border: usize) {
        let _ = write!(out, "{:LABEL$}", "");
        for boundary in 0..=self.columns() {
            out.push(self.junction(border, boundary));
            if boundary < self.columns() {
                let line = self.horizontal(border, boundary);
                let segment = glyph(Line::Blank, Line::Blank, line, line);
                out.extend(iter::repeat_n(segment, width + 1));
            }
        }
        out.push('\n');
    }

    fn draw_row(&self, out: &mut String, width: usize, index: usize, row: &Row) {
        let _ = write!(out, "  {:<8}", row.label);
        for boundary in 0..=self.columns() {
            let line = self.vertical(index, boundary);
            out.push(glyph(line, line, Line::Blank, Line::Blank));
            if let Some((cell, _)) = row.cells.get(boundary) {
                cell.draw(out, width);
            }
        }
        if self.truncated {
            out.push_str(" …");
        }
        out.push('\n');
    }

    fn changed(&self, row: usize, column: usize) -> bool {
        self.rows
            .get(row)
            .and_then(|row| row.cells.get(column))
            .is_some_and(|&(_, changed)| changed)
    }

    /// The segment of horizontal border `border` over `column`.
    fn horizontal(&self, border: usize, column: usize) -> Line {
        let above = border
            .checked_sub(1)
            .is_some_and(|row| self.changed(row, column));
        Line::weight(above || self.changed(border, column))
    }

    /// The segment of vertical boundary `boundary` beside `row`.
    fn vertical(&self, row: usize, boundary: usize) -> Line {
        let left = boundary
            .checked_sub(1)
            .is_some_and(|column| self.changed(row, column));
        Line::weight(left || self.changed(row, boundary))
    }

    fn junction(&self, border: usize, boundary: usize) -> char {
        let up = match border.checked_sub(1) {
            Some(row) => self.vertical(row, boundary),
            None => Line::Blank,
        };
        let down = if border < self.rows.len() {
            self.vertical(border, boundary)
        } else {
            Line::Blank
        };
        let left = match boundary.checked_sub(1) {
            Some(column) => self.horizontal(border, column),
            None => Line::Blank,
        };
        let right = if boundary < self.columns() {
            self.horizontal(border, boundary)
        } else {
            Line::Blank
        };
        glyph(up, down, left, right)
    }
}
