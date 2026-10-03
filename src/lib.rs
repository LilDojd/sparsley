//! A sparse set: packed, contiguous values with O(1) keyed access.

#![no_std]

extern crate alloc;

mod key;
pub mod map;
mod sparse;

pub use key::Key;
pub use map::SparseMap;
