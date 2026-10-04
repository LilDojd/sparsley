//! some edge cases

use std::fmt::Debug;

use sparsley::map::Entry;
use sparsley::{Key, SparseMap, SparseSet};

fn sample() -> SparseMap<u32, String> {
    [(3, "c"), (1, "a"), (7, "g"), (5, "e")]
        .map(|(k, v)| (k, v.to_owned()))
        .into()
}

#[test]
fn queries_never_allocate() {
    let mut map = SparseMap::<usize, u8>::new();
    for key in [0, 1, usize::MAX] {
        assert_eq!(map.get(key), None);
        assert_eq!(map.get_key_value(key), None);
        assert_eq!(map.get_mut(key), None);
        assert_eq!(map.get_index_of(key), None);
        assert!(!map.contains_key(key));
        assert_eq!(map.remove(key), None);
        assert_eq!(map.get_disjoint_mut([key]), [None]);
    }
    assert_eq!(map.get_index(0), None);
    assert_eq!(map.iter().next(), None);
    assert_eq!((map.capacity(), map.key_capacity()), (0, 0));

    map.insert(3, 0);
    let key_capacity = map.key_capacity();
    assert_eq!(map.get(usize::MAX), None);
    assert_eq!(map.get_key_value(usize::MAX), None);
    assert_eq!(map.remove(usize::MAX), None);
    assert_eq!(map.key_capacity(), key_capacity);
}

#[test]
fn get_key_value_returns_the_stored_key() {
    #[derive(Clone, Copy)]
    struct Tagged(usize, &'static str);

    impl PartialEq for Tagged {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl Key for Tagged {
        fn slot(self) -> usize {
            self.0
        }
    }

    let mut map = SparseMap::new();
    map.insert(Tagged(2, "original"), "old");
    assert_eq!(map.insert(Tagged(2, "replacement"), "new"), Some("old"));
    let (stored, value) = map.get_key_value(Tagged(2, "query")).unwrap();
    assert_eq!((stored.1, *value), ("original", "new"));
}

#[test]
fn key_capacity_covers_reserved_keys() {
    let mut map = SparseMap::<u32, ()>::new();
    map.reserve_keys(0);
    assert_eq!(map.key_capacity(), 0);

    map.reserve_keys(10);
    assert!(map.key_capacity() >= 10);

    assert!(matches!(map.entry(1000), Entry::Vacant(_)));
    assert!(map.key_capacity() > 1000);
    assert!(map.is_empty());

    map.insert(5000, ());
    assert!(map.key_capacity() > 5000);
    map.remove(5000);
    map.shrink_to_fit();
    assert_eq!(map.key_capacity(), 0);
}

#[test]
fn entry_api() {
    let mut map = sample();

    let mut called = false;
    let value = map.entry(1).or_insert_with(|| {
        called = true;
        String::new()
    });
    assert_eq!(*value, "a");
    assert!(!called);

    assert_eq!(*map.entry(2).or_insert_with_key(|k| k.to_string()), "2");
    assert_eq!(*map.entry(4).or_default(), "");
    assert_eq!(
        *map.entry(6)
            .and_modify(|_| unreachable!())
            .or_insert("f".into()),
        "f"
    );
    map.entry(3).and_modify(|v| v.push('!')).or_default();
    assert_eq!(map[3], "c!");

    let position = map.get_index_of(7);
    let Entry::Occupied(mut entry) = map.entry(7) else {
        panic!("7 is present");
    };
    assert_eq!(entry.key(), 7);
    assert_eq!(Some(entry.index()), position);
    assert_eq!(entry.insert("G".into()), "g");
    assert_eq!(entry.get(), "G");
    entry.get_mut().push('?');
    assert_eq!(entry.remove_entry(), (7, "G?".to_owned()));
    assert!(!map.contains_key(7));

    let Entry::Vacant(entry) = map.entry(8) else {
        panic!("8 is absent");
    };
    entry.insert("h".into()).push('!');
    assert_eq!(map[8], "h!");
}

#[test]
#[should_panic(expected = "key not present")]
fn index_missing_key_panics() {
    let _ = &sample()[2];
}

#[test]
#[should_panic(expected = "duplicate keys")]
fn get_disjoint_mut_duplicate_panics() {
    let _ = sample().get_disjoint_mut([1, 3, 1]);
}

#[test]
fn set_swap_indices_out_of_bounds_panics() {
    for mut set in [SparseSet::<u32>::new(), SparseSet::from([4, 8])] {
        for (a, b) in [(0, set.len()), (set.len(), 0), (0, usize::MAX)] {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    set.swap_indices(a, b);
                }))
                .is_err()
            );
        }
    }
}

#[test]
fn reserve_guarantees_capacity() {
    for additional in 0..16 {
        let mut map = sample();
        map.reserve(additional);
        assert!(map.capacity() >= map.len() + additional);

        let mut set = SparseSet::from([3u32, 1, 7, 5]);
        set.reserve(additional);
        assert!(set.capacity() >= set.len() + additional);
    }

    let mut set = SparseSet::<u32>::new();
    set.reserve_keys(500);
    assert!(set.key_capacity() >= 500);
}

#[test]
fn entry_finds_keys_in_distant_chunks() {
    let mut map = SparseMap::new();
    map.reserve_keys(8192);
    for key in [0u32, 1100, 5000] {
        map.insert(key, key);
        assert!(matches!(map.entry(key), Entry::Occupied(_)), "{key}");
    }
}

#[test]
fn slices_and_dense_indices() {
    let mut map = sample();
    let (keys, values) = map.as_slices();
    assert_eq!(keys, [3, 1, 7, 5]);
    assert_eq!(values, ["c", "a", "g", "e"]);

    map.as_mut_slices().1[1].push('!');
    let (key, value) = map.get_index_mut(2).unwrap();
    assert_eq!(key, 7);
    value.push('?');
    assert!(map.get_index_mut(4).is_none());
    assert_eq!(map.values(), ["c", "a!", "g?", "e"]);
}

#[test]
fn zero_sized_keys_and_values() {
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Unit;

    impl Key for Unit {
        fn slot(self) -> usize {
            0
        }
    }

    let max = u32::MAX as usize;
    let mut map = SparseMap::new();
    assert_eq!(map.capacity(), max);
    assert_eq!(map.insert(Unit, ()), None);
    assert_eq!(map.insert(Unit, ()), Some(()));
    map.shrink_to_fit();
    assert_eq!(map.capacity(), max);
    assert_eq!(map.remove_entry(Unit), Some((Unit, ())));
}

#[test]
fn set_clones_and_iterates_by_reference() {
    let set = SparseSet::from([9u32, 4, 6]);
    assert_eq!(set.clone().as_slice(), [9, 4, 6]);
    assert_eq!((&set).into_iter().collect::<Vec<_>>(), [9, 4, 6]);
}

#[test]
fn equality_ignores_dense_order() {
    let forward: SparseMap<u32, char> = [(1, 'a'), (2, 'b'), (3, 'c')].into();
    let mut backward: SparseMap<u32, char> = [(3, 'c'), (2, 'b'), (1, 'a')].into();
    assert_ne!(forward.keys(), backward.keys());
    assert_eq!(forward, backward);

    backward.insert(2, 'x');
    assert_ne!(forward, backward);
    backward.insert(2, 'b');
    backward.insert(4, 'd');
    assert_ne!(forward, backward);
    assert_ne!(backward, forward);

    assert_eq!(SparseSet::from([1u8, 2, 3]), SparseSet::from([3, 1, 2]));
    assert_ne!(SparseSet::from([1u8, 2]), SparseSet::from([1, 3]));
}

#[test]
fn debug_formatting() {
    let map: SparseMap<u32, &str> = [(2, "b"), (1, "a")].into();
    assert_eq!(format!("{map:?}"), r#"{2: "b", 1: "a"}"#);
    assert_eq!(format!("{:?}", SparseSet::from([4u8, 2])), "{4, 2}");

    let mut map = map;
    assert_eq!(format!("{:?}", map.iter()), r#"[(2, "b"), (1, "a")]"#);
    assert_eq!(format!("{:?}", map.iter_mut()), "IterMut { .. }");
    assert_eq!(
        format!("{:?}", map.entry(2)),
        r#"Occupied(OccupiedEntry { key: 2, value: "b" })"#
    );
    assert_eq!(format!("{:?}", map.entry(9)), "Vacant(VacantEntry(9))");
}

#[test]
fn extending_copied_values() {
    let mut map: SparseMap<u32, char> = [(2, 'x')].into();
    map.extend([(2, &'y'), (3, &'z')]);
    assert_eq!(map, SparseMap::from([(3, 'z'), (2, 'y')]));
}

fn check_iter<I>(make: impl Fn() -> I, expected: &[I::Item])
where
    I: DoubleEndedIterator + ExactSizeIterator,
    I::Item: PartialEq + Debug,
{
    let mut iter = make();
    let mut remaining = expected.iter();
    let mut front = true;
    while remaining.len() != 0 {
        assert_eq!(iter.len(), remaining.len());
        let (actual, expected) = if front {
            (iter.next(), remaining.next())
        } else {
            (iter.next_back(), remaining.next_back())
        };
        assert_eq!(actual.as_ref(), expected);
        front = !front;
    }
    assert_eq!(iter.len(), 0);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
}

#[test]
fn iterators_follow_dense_order() {
    let map = sample();
    let owned = [(3, "c"), (1, "a"), (7, "g"), (5, "e")].map(|(k, v)| (k, v.to_owned()));
    let borrowed: Vec<_> = owned.iter().map(|(k, v)| (*k, v)).collect();
    check_iter(|| map.iter(), &borrowed);
    check_iter(|| map.clone().into_iter(), &owned);
    check_iter(|| SparseMap::<u32, String>::new().into_iter(), &[]);
    check_iter(|| SparseMap::from([(7_u32, "g")]).into_iter(), &[(7, "g")]);

    let mut iter = map.iter();
    iter.next();
    let fork = iter.clone();
    assert!(iter.eq(fork));

    let mut copy = map.clone();
    for (k, v) in &mut copy {
        *v = k.to_string();
    }
    assert_eq!(copy.values(), ["3", "1", "7", "5"]);

    let mut set = SparseSet::from([9u32, 4, 6]);
    let mut drain = set.drain();
    assert_eq!(drain.len(), 3);
    assert_eq!((drain.next(), drain.next_back()), (Some(9), Some(6)));
    assert_eq!(drain.len(), 1);
}

fn roundtrip<K: Key + Debug + PartialEq>(keys: [K; 3]) {
    let mut map = SparseMap::new();
    for (value, key) in keys.into_iter().enumerate() {
        assert_eq!(map.insert(key, value), None);
    }
    for (value, key) in keys.into_iter().enumerate() {
        assert_eq!(map.get(key), Some(&value));
    }
    assert_eq!(map.remove_entry(keys[0]), Some((keys[0], 0)));
    assert_eq!(map.keys(), [keys[2], keys[1]]);
}

#[test]
fn key_types() {
    roundtrip([0u8, u8::MAX, 7]);
    roundtrip([0u16, u16::MAX, 7]);
    roundtrip([0u32, 300, 7]);
    #[cfg(target_pointer_width = "64")]
    roundtrip([0u64, 300, 7]);
    roundtrip([0usize, 300, 7]);
}
