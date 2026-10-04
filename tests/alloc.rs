//! Dropping maps and sets frees every allocation.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use sparsley::{SparseMap, SparseSet};

struct Counting;

thread_local! {
    static LIVE: Cell<isize> = const { Cell::new(0) };
}

// SAFETY: forwards to `System`.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.set(LIVE.get() + 1);
        // SAFETY: the caller upholds the `GlobalAlloc` contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LIVE.set(LIVE.get() + 1);
        // SAFETY: the caller upholds the `GlobalAlloc` contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.set(LIVE.get() - 1);
        // SAFETY: the caller upholds the `GlobalAlloc` contract.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller upholds the `GlobalAlloc` contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn dropping_frees_everything() {
    let before = LIVE.get();
    let mut map: SparseMap<u32, String> = (0..100).map(|k| (k * 7, k.to_string())).collect();
    map.remove(14);
    let mut set: SparseSet<u32> = (0..100).collect();
    set.shrink_to_fit();
    drop((map.clone(), map, set));
    assert_eq!(LIVE.get(), before);
}
