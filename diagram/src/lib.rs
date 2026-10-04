//! ASCII memory diagrams of [`SparseMap`] operations.
//!
//! [`trace!`] runs a map call and draws every array the map owns afterwards:
//!
//! ```
//! use sparsley::SparseMap;
//! use sparsley_diagram::trace;
//!
//! let mut map = SparseMap::from([(3_usize, 'a'), (7, 'b'), (1, 'c')]);
//! print!("{}", trace!(map.remove(3)));
//! ```
//!
//! <!-- diagram: crate -->
//! ```text
//! map.remove(3) -> Some('a')
//!             0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  ..
//!   sparse    .   1*  .   .*  .   .   .   2   .   .   .   .   .   .   .   .
//!
//!             0   1   2
//!   keys      1*  7   _*
//!   values    c*  b   _*
//!
//!   len 3 -> 2, capacity 3, key_capacity 64
//!   sparse[3]  1 -> .  key 3 removed from position 0
//!   sparse[1]  3 -> 1  key 1 moved from position 2 to 0
//! ```
//!
//! Sparse slots store `position + 1`; `.` is an empty slot. `_` is unused
//! dense capacity, and `*` marks cells the call changed. The notes list each
//! slot the call rewrote, or the slot a lookup read. Only the first
//! [`COLUMNS`] slots and positions are drawn.

use std::fmt::{Display, Write};

use sparsley::{Key, SparseMap};

/// The number of sparse slots and dense positions drawn.
pub const COLUMNS: usize = 16;

/// The observable state of a map, read through its public API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Dense position of each drawn key index that has a slot.
    slots: Vec<Option<usize>>,
    /// Index and rendering of each dense key.
    keys: Vec<(usize, String)>,
    values: Vec<String>,
    capacity: usize,
    key_capacity: usize,
}

impl Snapshot {
    /// Reads the state of `map`.
    pub fn of<K, V>(map: &SparseMap<K, V>) -> Self
    where
        K: Key + TryFrom<usize> + Display,
        V: Display,
    {
        let slots = (0..map.key_capacity().min(COLUMNS))
            .map(|index| K::try_from(index).ok().and_then(|key| map.position(key)))
            .collect();
        Self {
            slots,
            keys: map
                .keys()
                .iter()
                .map(|&key| (key.index(), key.to_string()))
                .collect(),
            values: map.values().iter().map(ToString::to_string).collect(),
            capacity: map.capacity(),
            key_capacity: map.key_capacity(),
        }
    }

    fn len(&self) -> usize {
        self.keys.len()
    }

    fn position(&self, index: usize) -> Option<usize> {
        self.keys.iter().position(|&(i, _)| i == index)
    }

    fn sparse_cells(&self) -> Vec<String> {
        self.slots.iter().map(|&slot| slot_text(slot)).collect()
    }

    fn dense_cells(&self, column: impl Fn(usize) -> String) -> Vec<String> {
        (0..self.capacity.min(COLUMNS))
            .map(|position| {
                if position < self.len() {
                    column(position)
                } else {
                    "_".into()
                }
            })
            .collect()
    }

    fn key_cells(&self) -> Vec<String> {
        self.dense_cells(|position| self.keys[position].1.clone())
    }

    fn value_cells(&self) -> Vec<String> {
        self.dense_cells(|position| self.values[position].clone())
    }
}

/// Draws the arrays of `map`.
pub fn layout<K, V>(map: &SparseMap<K, V>) -> String
where
    K: Key + TryFrom<usize> + Display,
    V: Display,
{
    let snapshot = Snapshot::of(map);
    let mut out = String::new();
    draw(&mut out, &snapshot, &snapshot);
    summary(&mut out, &snapshot, &snapshot);
    finish(&out)
}

/// Draws the state change of `call`, which returned `result`.
///
/// `focus` is the key index the call looked up, if any; an unchanged focused
/// slot is drawn as a lookup.
pub fn render(
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
    draw(&mut out, before, after);
    summary(&mut out, before, after);
    notes(&mut out, before, after, focus);
    finish(&out)
}

#[doc(hidden)]
pub fn index<K: Key>(key: K) -> usize {
    key.index()
}

fn draw(out: &mut String, before: &Snapshot, after: &Snapshot) {
    let slots = after.slots.len();
    let mut header = indices(slots);
    if after.key_capacity > slots {
        header.push("..".into());
    }
    if slots == 0 {
        out.push_str("  sparse    (none)\n");
    } else {
        row(out, "", &header, None);
        row(
            out,
            "sparse",
            &after.sparse_cells(),
            Some(&before.sparse_cells()),
        );
    }
    out.push('\n');
    let mut header = indices(after.capacity.min(COLUMNS));
    if after.capacity > COLUMNS {
        header.push("..".into());
    }
    if after.capacity == 0 {
        out.push_str("  keys      (none)\n  values    (none)\n");
    } else {
        row(out, "", &header, None);
        row(out, "keys", &after.key_cells(), Some(&before.key_cells()));
        row(
            out,
            "values",
            &after.value_cells(),
            Some(&before.value_cells()),
        );
    }
}

fn summary(out: &mut String, before: &Snapshot, after: &Snapshot) {
    let _ = writeln!(
        out,
        "\n  len {}, capacity {}, key_capacity {}",
        change(before.len(), after.len()),
        change(before.capacity, after.capacity),
        change(before.key_capacity, after.key_capacity),
    );
}

/// Writes one line per slot or value the call changed, plus the lookup of an
/// unchanged `focus` slot.
fn notes(out: &mut String, before: &Snapshot, after: &Snapshot, focus: Option<usize>) {
    let mut lines = Vec::new();
    for (from, (index, key)) in before.keys.iter().enumerate() {
        if after.position(*index).is_none() {
            let what = format!("key {key} removed from position {from}");
            lines.push((*index, Some(from), None, what));
        }
    }
    for (to, (index, key)) in after.keys.iter().enumerate() {
        match before.position(*index) {
            Some(from) if from != to => {
                let what = format!("key {key} moved from position {from} to {to}");
                lines.push((*index, Some(from), Some(to), what));
            }
            Some(_) => {}
            None => lines.push((
                *index,
                None,
                Some(to),
                format!("key {key} pushed at position {to}"),
            )),
        }
    }
    let width = lines
        .iter()
        .map(|&(index, ..)| format!("sparse[{index}]").len())
        .max()
        .unwrap_or(0);
    for (index, from, to, what) in &lines {
        let target = format!("sparse[{index}]");
        let _ = writeln!(
            out,
            "  {target:<width$}  {} -> {}  {what}",
            slot_text(*from),
            slot_text(*to),
        );
    }
    if let Some(index) = focus.filter(|&index| lines.iter().all(|line| line.0 != index)) {
        let slot = slot_text(after.position(index));
        let _ = match after.position(index) {
            Some(position) => writeln!(
                out,
                "  sparse[{index}] = {slot} -> keys[{position}] = {}, values[{position}] = {}",
                after.keys[position].1, after.values[position],
            ),
            None => writeln!(out, "  sparse[{index}] = {slot} -> no entry"),
        };
    }
    for (position, (index, key)) in after.keys.iter().enumerate() {
        if let Some(old) = before.position(*index).filter(|&from| from == position) {
            let (old, new) = (&before.values[old], &after.values[position]);
            if old != new {
                let _ = writeln!(
                    out,
                    "  values[{position}]  {old} -> {new}  key {key} replaced"
                );
            }
        }
    }
}

fn slot_text(position: Option<usize>) -> String {
    position.map_or_else(|| ".".into(), |position| (position + 1).to_string())
}

fn change(before: usize, after: usize) -> String {
    if before == after {
        after.to_string()
    } else {
        format!("{before} -> {after}")
    }
}

fn indices(count: usize) -> Vec<String> {
    (0..count).map(|i| i.to_string()).collect()
}

/// Writes a labelled row, marking cells that differ from `before` with `*`.
fn row(out: &mut String, label: &str, cells: &[String], before: Option<&[String]>) {
    let _ = write!(out, "  {label:<8}");
    for (i, cell) in cells.iter().enumerate() {
        let changed = before.is_some_and(|before| before.get(i) != Some(cell));
        let _ = write!(out, "{cell:>3}{}", if changed { '*' } else { ' ' });
    }
    out.push('\n');
}

fn finish(out: &str) -> String {
    out.lines()
        .map(|line| line.trim_end().to_owned() + "\n")
        .collect()
}

/// Runs map calls and draws the state change of each.
///
/// `trace!(map.insert(3, 'a'))` draws one call; `trace!(map; insert(3, 'a'),
/// remove(7))` draws a sequence, separated by blank lines. Calls whose first
/// argument is a key (`get`, `get_mut`, `position`, `contains_key`, `insert`,
/// `remove`, `remove_entry`) also draw their slot lookup. Returns a `String`.
#[macro_export]
macro_rules! trace {
    (@run $map:ident . $method:ident $args:tt => $call:expr, $focus:expr) => {{
        let before = $crate::Snapshot::of(&$map);
        let result = ::std::format!("{:?}", $call);
        $crate::render(
            ::std::concat!(::std::stringify!($map), ".", ::std::stringify!($method), ::std::stringify!($args)),
            &result,
            &before,
            &$crate::Snapshot::of(&$map),
            $focus,
        )
    }};
    (@keyed $map:ident . $method:ident ($key:expr $(, $arg:expr)* $(,)?)) => {{
        let key = $key;
        let result = $crate::trace!(
            @run $map . $method ($key $(, $arg)*) => $map.$method(key $(, $arg)*),
            ::std::option::Option::Some($crate::index(key))
        );
        result
    }};
    ($map:ident; $($method:ident $args:tt),+ $(,)?) => {
        [$($crate::trace!($map . $method $args)),+].join("\n")
    };
    ($map:ident . get $args:tt) => { $crate::trace!(@keyed $map . get $args) };
    ($map:ident . get_mut $args:tt) => { $crate::trace!(@keyed $map . get_mut $args) };
    ($map:ident . position $args:tt) => { $crate::trace!(@keyed $map . position $args) };
    ($map:ident . contains_key $args:tt) => { $crate::trace!(@keyed $map . contains_key $args) };
    ($map:ident . insert $args:tt) => { $crate::trace!(@keyed $map . insert $args) };
    ($map:ident . remove $args:tt) => { $crate::trace!(@keyed $map . remove $args) };
    ($map:ident . remove_entry $args:tt) => { $crate::trace!(@keyed $map . remove_entry $args) };
    ($map:ident . $method:ident ($($arg:expr),* $(,)?)) => {
        $crate::trace!(@run $map . $method ($($arg),*) => $map.$method($($arg),*), ::std::option::Option::None)
    };
}
