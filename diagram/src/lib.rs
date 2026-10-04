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
//!             0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
//!           ┌───┲━━━┱───┲━━━┱───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
//!   sparse  │   ┃ 1 ┃   ┃   ┃   │   │   │ 2 │   │   │   │   │   │   │   │   │ …
//!           └───┺━━━┹───┺━━━┹───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
//!             ╭───╯
//!             ▼   1   2
//!           ┏━━━┱───┲━━━┓
//!   keys    ┃ 1 ┃ 7 ┃░░░┃
//!           ┣━━━╉───╊━━━┫
//!   values  ┃ c ┃ b ┃░░░┃
//!           ┗━━━┹───┺━━━┛
//!
//!   len 3 -> 2, capacity 3, key_capacity 64
//!   sparse[3]  1 -> 0  key 3 removed from position 0
//!   sparse[1]  3 -> 1  key 1 moved from position 2 to 0
//! ```
//!
//! Sparse slots store `position + 1`; an empty slot is zero and drawn blank.
//! Shaded cells are unused dense capacity, and heavy borders frame cells the
//! call changed. The line between the tables links the looked-up or moved
//! key's slot to its position. The notes list each slot the call rewrote.
//! Only the first [`COLUMNS`] slots and positions are drawn.

mod grid;
mod link;
mod notes;
mod snapshot;
mod table;

use std::fmt::{Display, Write};

use sparsley::{Key, SparseMap};

use link::Link;
pub use snapshot::Snapshot;
use table::Table;

/// The number of sparse slots and dense positions drawn.
pub const COLUMNS: usize = 16;

/// Draws the arrays of `map`.
pub fn layout<K, V>(map: &SparseMap<K, V>) -> String
where
    K: Key + TryFrom<usize> + Display,
    V: Display,
{
    let snapshot = Snapshot::of(map);
    let mut out = String::new();
    draw(&mut out, &snapshot, &snapshot, None);
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
    draw(&mut out, before, after, focus);
    notes::notes(&mut out, before, after, focus);
    finish(&out)
}

#[doc(hidden)]
pub fn index<K: Key>(key: K) -> usize {
    key.index()
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

/// Runs map calls and draws the state change of each.
///
/// `trace!(map.insert(3, 'a'))` draws one call; `trace!(map; insert(3, 'a'),
/// remove(7))` draws a sequence, separated by blank lines. Calls whose first
/// argument is a key (`get`, `get_mut`, `position`, `contains_key`, `insert`,
/// `remove`, `remove_entry`) also draw their slot lookup. Returns a `String`.
#[macro_export]
macro_rules! trace {
    // A sequence of calls on one map.
    ($map:ident; $($method:ident $args:tt),+ $(,)?) => {
        [$($crate::trace!($map . $method $args)),+].join("\n")
    };

    // Calls whose first argument is a key, so its lookup can be drawn.
    ($map:ident . get $args:tt) => { $crate::trace!(@keyed $map . get $args) };
    ($map:ident . get_mut $args:tt) => { $crate::trace!(@keyed $map . get_mut $args) };
    ($map:ident . position $args:tt) => { $crate::trace!(@keyed $map . position $args) };
    ($map:ident . contains_key $args:tt) => { $crate::trace!(@keyed $map . contains_key $args) };
    ($map:ident . insert $args:tt) => { $crate::trace!(@keyed $map . insert $args) };
    ($map:ident . remove $args:tt) => { $crate::trace!(@keyed $map . remove $args) };
    ($map:ident . remove_entry $args:tt) => { $crate::trace!(@keyed $map . remove_entry $args) };

    // Any other call.
    ($map:ident . $method:ident ($($arg:expr),* $(,)?)) => {
        $crate::trace!(@run $map . $method ($($arg),*), ::std::option::Option::None)
    };

    // Evaluates the key once, then runs the call with its index as focus.
    (@keyed $map:ident . $method:ident ($key:expr $(, $arg:expr)* $(,)?)) => {{
        let key = $key;
        let focus = ::std::option::Option::Some($crate::index(key));
        $crate::trace!(@draw $map . $method ($key $(, $arg)*) => $map.$method(key $(, $arg)*), focus)
    }};

    (@run $map:ident . $method:ident ($($arg:expr),*), $focus:expr) => {
        $crate::trace!(@draw $map . $method ($($arg),*) => $map.$method($($arg),*), $focus)
    };

    // Snapshots around `$call`; `$args` is the argument text to show.
    (@draw $map:ident . $method:ident $args:tt => $call:expr, $focus:expr) => {{
        let before = $crate::Snapshot::of(&$map);
        let result = ::std::format!("{:?}", $call);
        let call = ::std::concat!(
            ::std::stringify!($map), ".", ::std::stringify!($method), ::std::stringify!($args)
        );
        $crate::render(call, &result, &before, &$crate::Snapshot::of(&$map), $focus)
    }};
}
