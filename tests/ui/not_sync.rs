use std::cell::Cell;

use sparsley::SparseMap;

fn assert_sync<T: Sync>() {}

fn main() {
    assert_sync::<SparseMap<u32, Cell<u8>>>();
}
