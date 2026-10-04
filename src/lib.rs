//! Sparse sets with packed, contiguous storage.
//!
//! [`SparseMap`] maps small integer [`Key`]s, such as entity ids, to values.
//! Lookup, insertion and removal are O(1); iteration runs over plain slices.
//! [`SparseSet`] is the same structure without values.
//!
//! ```
//! use sparsley::SparseMap;
//!
//! let mut positions = SparseMap::new();
//! positions.insert(7_u32, [0.0_f32, 1.0]);
//! positions.insert(42, [2.0, 3.0]);
//!
//! for position in positions.values_mut() {
//!     position[1] -= 9.81;
//! }
//! assert_eq!(positions[42], [2.0, 3.0 - 9.81]);
//! assert_eq!(positions.remove(7), Some([0.0, 1.0 - 9.81]));
//! ```
//!
//! # Layout
//!
//! <!-- diagram: layout -->
//! ```text
//!             0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
//!           ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
//!   sparse  │   │ 3 │   │ 1 │   │   │   │ 2 │   │   │   │   │   │   │   │   │ …
//!           └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
//!             0   1   2   3
//!           ┌───┬───┬───┬───┐
//!   keys    │ 3 │ 7 │ 1 │░░░│
//!           ├───┼───┼───┼───┤
//!   values  │ a │ b │ c │░░░│
//!           └───┴───┴───┴───┘
//!
//!   len 3, capacity 4, key_capacity 64
//! ```
//!
//! The sparse array maps a key's index to its dense position. Slots store
//! `position + 1` in a `u32`, so an empty slot (drawn blank) is zero: sparse
//! memory comes from zeroed allocations, which the system allocator maps
//! lazily, and a lookup is two loads and two comparisons.
//!
//! Keys and values live in one allocation as two parallel arrays, so
//! [`SparseMap::values`] is a real `&[V]`. Shaded cells are unused capacity.
//!
//! Removal moves the last entry into the hole, then repairs its slot:
//!
//! <!-- diagram: remove -->
//! ```text
//! map.remove(3) -> Some('a')
//!             0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
//!           ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
//!   sparse  │   │ 1*│   │  *│   │   │   │ 2 │   │   │   │   │   │   │   │   │ …
//!           └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
//!             0   1   2   3
//!           ┌───┬───┬───┬───┐
//!   keys    │ 1*│ 7 │░░*│░░░│
//!           ├───┼───┼───┼───┤
//!   values  │ c*│ b │░░*│░░░│
//!           └───┴───┴───┴───┘
//!
//!   len 3 -> 2, capacity 4, key_capacity 64
//!   sparse[3]  1 -> 0  key 3 removed from position 0
//!   sparse[1]  3 -> 1  key 1 moved from position 2 to 0
//! ```
//!
//! Dense order is therefore unspecified, and positions change on removal.
//! `DESIGN.md` in the repository diagrams every operation.
//!
//! # Limits
//!
//! Sparse memory is four bytes per slot up to the largest key index. Use a
//! hash map for huge or scattered keys. A map holds at most `u32::MAX`
//! entries.

#![no_std]

extern crate alloc;

mod dense;
mod key;
pub mod map;
pub mod set;
mod sparse;

pub use key::Key;
pub use map::SparseMap;
pub use set::SparseSet;
