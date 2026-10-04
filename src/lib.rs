//! Sparse sets with packed, contiguous storage.
//!
//! [`SparseMap`] maps small integer [`Key`]s, such as entity ids, to values.
//! Lookup, insertion and removal are O(1); iteration runs over plain slices.
//! [`SparseSet`] is a [`SparseMap<K, ()>`].
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
#![doc = sparsley_diagram::diagram! {
    let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c')]);
}]
//!
//! The sparse array maps each key's [`slot`](Key::slot) to its dense index.
//! A slot stores `index + 1` in a `u32`, so an empty slot (drawn blank) is
//! zero: sparse memory comes from zeroed allocations, which the system
//! allocator maps lazily.
//!
//! Keys and values live in one allocation as two parallel arrays, so
//! [`SparseMap::values`] is a `&[V]`. In the diagrams, shaded cells are
//! unused capacity and heavy borders frame the cells a call changed.
//!
//! Removal swaps the entry with the last entry, then "repairs" its slot:
//!
#![doc = sparsley_diagram::diagram! {
    let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c')]);
    map.remove(3);
}]
//!
//! Dense order is therefore unspecified, and dense indices change on removal.
//!
//! # Keys
//!
//! [`Key`] is implemented for the unsigned integers. Implement it for your own
//! handle types; a key's slot should be small and dense, like an entity id.

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

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
