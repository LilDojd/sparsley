//! Draws snapshots as boxed tables with notes.

use std::fmt::Write;

use crate::link::Link;
use crate::notes;
use crate::snapshot::Snapshot;
use crate::table::Table;

/// The number of sparse slots and dense positions drawn.
pub(crate) const COLUMNS: usize = 16;

/// Draws the arrays of a snapshot.
pub(crate) fn layout(snapshot: &Snapshot) -> String {
    let mut out = String::new();
    draw(&mut out, snapshot, snapshot, None);
    finish(&out)
}

/// Draws the state change of `call`, which returned `result`.
///
/// `focus` is the key index the call looked up, if any; an unchanged focused
/// slot is drawn as a lookup.
pub(crate) fn render(
    call: &str,
    result: &str,
    before: &Snapshot,
    after: &Snapshot,
    focus: Option<usize>,
) -> String {
    let mut out = String::from(call);
    if result != "()" {
        let _ = write!(out, " -> {result}");
    }
    out.push('\n');
    draw(&mut out, before, after, focus);
    notes::notes(&mut out, before, after, focus);
    finish(&out)
}

/// Draws both tables, the link between them and the summary, marking changes
/// from `before`.
fn draw(out: &mut String, before: &Snapshot, after: &Snapshot, focus: Option<usize>) {
    let sparse = Table::sparse(before, after);
    let dense = Table::dense(before, after);
    let width = sparse.width().max(dense.width());
    let link = Link::find(before, after, focus);
    sparse.draw(out, width, None);
    if let Some(link) = &link {
        link.draw(out, width);
    }
    dense.draw(out, width, link.map(|link| link.position));
    notes::summary(out, before, after);
}

/// Strips trailing spaces from every line.
fn finish(out: &str) -> String {
    out.lines()
        .map(|line| line.trim_end().to_owned() + "\n")
        .collect()
}
