//! Checks the `sparsley-diagram` model

use sparsley::{SparseMap, SparseSet};

/// Runs a scenario through `sparsley_diagram::state!` and as real code on a
/// `SparseMap<usize, char>`, then compares slots, keys, values, length and
/// capacities.
macro_rules! map_case {
    (let mut $map:ident = SparseMap::$new:ident($($args:tt)*); $($call:expr;)*) => {{
        let (slots, keys, values, len, capacity, key_capacity) = sparsley_diagram::state! {
            let mut $map = SparseMap::$new($($args)*);
            $($call;)*
        };
        #[allow(unused_mut)]
        let mut $map: SparseMap<usize, char> = SparseMap::$new($($args)*);
        $(let _ = $call;)*
        assert_eq!($map.keys(), keys, "keys");
        assert_eq!($map.values(), values, "values");
        assert_eq!(
            ($map.len(), $map.capacity(), $map.key_capacity()),
            (len, capacity, key_capacity),
            "len, capacity, key_capacity"
        );
        let real: Vec<_> = (0..$map.key_capacity()).map(|key| $map.get_index_of(key)).collect();
        assert_eq!(real, slots, "slots");
    }};
}

/// Like [`map_case!`], for a `SparseSet<usize>`.
macro_rules! set_case {
    (let mut $set:ident = SparseSet::$new:ident($($args:tt)*); $($call:expr;)*) => {{
        let (slots, keys, _, len, capacity, key_capacity) = sparsley_diagram::state! {
            let mut $set = SparseSet::$new($($args)*);
            $($call;)*
        };
        #[allow(unused_mut)]
        let mut $set: SparseSet<usize> = SparseSet::$new($($args)*);
        $(let _ = $call;)*
        assert_eq!($set.as_slice(), keys, "keys");
        assert_eq!(
            ($set.len(), $set.capacity(), $set.key_capacity()),
            (len, capacity, key_capacity),
            "len, capacity, key_capacity"
        );
        let real: Vec<_> = (0..$set.key_capacity()).map(|key| $set.get_index_of(key)).collect();
        assert_eq!(real, slots, "slots");
    }};
}

#[test]
fn constructors() {
    map_case! { let mut map = SparseMap::new(); }
    map_case! { let mut map = SparseMap::with_capacity(0); }
    map_case! { let mut map = SparseMap::with_capacity(5); }
    map_case! { let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c')]); }
    map_case! { let mut map = SparseMap::from([(1, 'a'), (1, 'b'), (2, 'c')]); }
    set_case! { let mut set = SparseSet::new(); }
    set_case! { let mut set = SparseSet::with_capacity(3); }
    set_case! { let mut set = SparseSet::from([4, 4, 9]); }
}

#[test]
fn insert_grows_past_capacity_and_key_capacity() {
    map_case! {
        let mut map = SparseMap::new();
        map.insert(0, 'a');
        map.insert(4, 'b');
        map.insert(2, 'c');
        map.insert(9, 'd');
        map.insert(5, 'e');
        map.insert(64, 'f');
        map.insert(300, 'g');
        map.insert(4, 'B');
    }
    map_case! {
        let mut map = SparseMap::with_capacity(2);
        map.insert(1000, 'a');
        map.insert(1, 'b');
        map.insert(2, 'c');
    }
    set_case! {
        let mut set = SparseSet::new();
        set.insert(3);
        set.insert(3);
        set.insert(70);
        set.insert(1);
        set.insert(2);
        set.insert(0);
    }
}

#[test]
fn lookups_change_nothing() {
    map_case! {
        let mut map = SparseMap::from([(3, 'a'), (7, 'b')]);
        map.get(3);
        map.get(500);
        map.get_mut(7);
        map.contains_key(4);
        map.get_index_of(7);
        map.get_index(1);
        map.get_index(9);
        map.get_key_value(3);
    }
    set_case! {
        let mut set = SparseSet::from([3, 7]);
        set.contains(7);
        set.get_index(0);
        set.get_index_of(500);
    }
}

#[test]
fn entry_or_insert() {
    map_case! {
        let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c')]);
        map.entry(7).or_insert('x');
        map.entry(9).or_insert('d');
        map.entry(2).or_insert('e');
        map.entry(100).or_insert('f');
    }
}

#[test]
fn removal_swaps_the_last_entry_in() {
    map_case! {
        let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c'), (9, 'd')]);
        map.remove(7);
        map.remove(1);
        map.remove(1);
        map.remove(500);
        map.remove_entry(3);
    }
    map_case! {
        let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c'), (9, 'd')]);
        map.swap_remove_index(0);
        map.swap_remove_index(2);
        map.swap_remove_index(5);
        map.swap_indices(0, 1);
    }
    set_case! {
        let mut set = SparseSet::from([3, 7, 1, 9]);
        set.remove(3);
        set.remove(3);
        set.swap_remove_index(1);
        set.swap_remove_index(4);
        set.swap_indices(1, 0);
    }
}

#[test]
fn clear_keeps_allocations() {
    map_case! {
        let mut map = SparseMap::from([(3, 'a'), (70, 'b')]);
        map.clear();
        map.insert(5, 'c');
    }
    set_case! {
        let mut set = SparseSet::from([3, 70]);
        set.clear();
    }
}

#[test]
fn retain_removes_at_both_ends() {
    map_case! {
        let mut map = SparseMap::from([(7, 'h'), (1, 'b'), (2, 'c'), (0, 'a')]);
        map.retain(|key, _| key % 7 != 0);
    }
    map_case! {
        let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c'), (9, 'd'), (4, 'b')]);
        map.retain(|_, value| *value != 'b');
        map.retain(|key, _| key >= 2);
    }
    map_case! {
        let mut map = SparseMap::from([(3, 'a'), (7, 'b')]);
        map.retain(|_, value| *value > 'z');
    }
    set_case! {
        let mut set = SparseSet::from([0, 5, 1, 6, 2]);
        set.retain(|key| key % 5 != 0);
        set.retain(|key| key < 6);
    }
}

#[test]
fn reserve_and_shrink() {
    map_case! {
        let mut map = SparseMap::from([(3, 'a'), (7, 'b')]);
        map.reserve(0);
        map.reserve(1);
        map.reserve(9);
        map.reserve_keys(0);
        map.reserve_keys(64);
        map.reserve_keys(65);
        map.reserve_keys(1000);
        map.remove(7);
        map.shrink_to_fit();
    }
    map_case! {
        let mut map = SparseMap::with_capacity(8);
        map.shrink_to_fit();
        map.insert(2, 'a');
    }
    set_case! {
        let mut set = SparseSet::from([3, 90]);
        set.reserve(3);
        set.reserve_keys(200);
        set.remove(90);
        set.shrink_to_fit();
    }
}

#[test]
fn sort() {
    map_case! {
        let mut map = SparseMap::from([(9, 'a'), (3, 'b'), (7, 'c'), (1, 'd')]);
        map.sort_unstable_keys();
    }
    set_case! {
        let mut set = SparseSet::from([9, 3, 7, 1]);
        set.sort_unstable();
    }
}
