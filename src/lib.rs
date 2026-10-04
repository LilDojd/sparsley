//! Sparse sets with packed, contiguous storage.
//!
//! [`SparseMap`] maps small integer [`Key`]s, such as entity ids, to values.
//! Lookup, insertion and removal are O(1); iteration runs over plain slices.
//! [`SparseSet`] is the same structure without values.
//!
//! ```
//! use sparsley::SparseMap;
//!
//! let mut velocity = SparseMap::new();
//! velocity.insert(7_u32, [0.0_f32, 1.0]);
//! velocity.insert(42, [2.0, 3.0]);
//!
//! for v in velocity.values_mut() {
//!     v[1] -= 9.81;
//! }
//! assert_eq!(velocity[42], [2.0, 3.0 - 9.81]);
//! assert_eq!(velocity.remove(7), Some([0.0, 1.0 - 9.81]));
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
//! The sparse array maps each key's [`slot`](Key::slot) to its dense index.
//! A slot stores `index + 1` in a `u32`, so an empty slot (drawn blank) is
//! zero: sparse memory comes from zeroed allocations, which the system
//! allocator maps lazily, and a lookup is two dependent loads.
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
//!           ┌───┲━━━┱───┲━━━┱───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
//!   sparse  │   ┃ 1 ┃   ┃   ┃   │   │   │ 2 │   │   │   │   │   │   │   │   │ …
//!           └───┺━━━┹───┺━━━┹───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
//!             ╭───╯
//!             ▼   1   2   3
//!           ┏━━━┱───┲━━━┱───┐
//!   keys    ┃ 1 ┃ 7 ┃░░░┃░░░│
//!           ┣━━━╉───╊━━━╉───┤
//!   values  ┃ c ┃ b ┃░░░┃░░░│
//!           ┗━━━┹───┺━━━┹───┘
//!
//!   len 3 -> 2, capacity 4, key_capacity 64
//!   sparse[3]  1 -> 0  key 3 removed from position 0
//!   sparse[1]  3 -> 1  key 1 moved from position 2 to 0
//! ```
//!
//! Dense order is therefore unspecified, and dense indices change on removal.
//! The methods of [`SparseMap`] diagram their effect the same way.
//!
//! # Keys
//!
//! [`Key`] is implemented for the unsigned integers. Implement it for your own
//! handle types; a key's slot should be small and dense, like an entity id.
//! For generational handles, key the map by the handle's index and check
//! generations where handles are issued: two generations of one index would
//! share a slot.
//!
//! # Limits
//!
//! Sparse memory is four bytes per slot up to the largest key slot. Use a hash
//! map for huge or scattered keys. A map holds at most `u32::MAX` entries.
//!
//! # Features
//!
//! * `serde`: serializes [`SparseMap`] as a map and [`SparseSet`] as a
//!   sequence.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

mod dense;
mod key;
pub mod map;
#[cfg(feature = "serde")]
mod serde;
pub mod set;
mod sparse;

pub use key::Key;
pub use map::SparseMap;
pub use set::SparseSet;
