//! A sparse set: packed, contiguous values with O(1) keyed access.

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
