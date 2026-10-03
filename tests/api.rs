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
        assert_eq!(map.get_mut(key), None);
        assert_eq!(map.position(key), None);
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
    assert_eq!(map.remove(usize::MAX), None);
    assert_eq!(map.key_capacity(), key_capacity);
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

    let position = map.position(7);
    let Entry::Occupied(mut entry) = map.entry(7) else {
        panic!("7 is present");
    };
    assert_eq!(entry.key(), 7);
    assert_eq!(Some(entry.position()), position);
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
    let mut map: SparseMap<u32, &str> = [(2, "b"), (1, "a")].into();
    assert_eq!(format!("{map:?}"), r#"{2: "b", 1: "a"}"#);
    assert_eq!(format!("{:?}", map.iter()), r#"[(2, "b"), (1, "a")]"#);
    assert_eq!(
        format!("{:?}", map.entry(1)),
        r#"Occupied(OccupiedEntry { key: 1, value: "a" })"#
    );
    assert_eq!(format!("{:?}", map.entry(9)), "Vacant(VacantEntry(9))");
    assert_eq!(format!("{:?}", SparseSet::from([4u8, 2])), "{4, 2}");
}

#[test]
fn collecting_keeps_last_duplicate() {
    let map: SparseMap<u32, char> = [(1, 'a'), (2, 'b'), (1, 'c')].into();
    assert_eq!((map.len(), map[1]), (2, 'c'));

    let mut map: SparseMap<u32, char> = [(2, 'x')].into_iter().collect();
    map.extend([(2, &'y'), (3, &'z')]);
    assert_eq!(map, SparseMap::from([(3, 'z'), (2, 'y')]));

    let set: SparseSet<u16> = [5, 5, 1].into_iter().collect();
    assert_eq!(set.as_slice(), [5, 1]);
}

fn check_iter<I>(make: impl Fn() -> I, expected: &[I::Item])
where
    I: DoubleEndedIterator + ExactSizeIterator,
    I::Item: Clone + PartialEq + Debug,
{
    let reversed: Vec<_> = expected.iter().rev().cloned().collect();
    assert_eq!(make().len(), expected.len());
    assert_eq!(make().count(), expected.len());
    assert_eq!(make().collect::<Vec<_>>(), expected);
    assert_eq!(make().rev().collect::<Vec<_>>(), reversed);
    assert_eq!(make().fold(Vec::new(), push), expected);
    assert_eq!(make().rfold(Vec::new(), push), reversed);
    for n in 0..=expected.len() {
        assert_eq!(make().nth(n), expected.get(n).cloned());
    }

    let mut iter = make();
    assert_eq!(iter.next(), expected.first().cloned());
    assert_eq!(
        iter.next_back(),
        expected.get(1..).and_then(<[_]>::last).cloned()
    );
    assert_eq!(iter.len(), expected.len().saturating_sub(2));
}

fn push<T>(mut acc: Vec<T>, item: T) -> Vec<T> {
    acc.push(item);
    acc
}

#[test]
fn iterators_follow_dense_order() {
    let map = sample();
    let dense: Vec<_> = map.keys().iter().copied().zip(map.values()).collect();
    check_iter(|| map.iter(), &dense);

    let owned: Vec<_> = dense.iter().map(|&(k, v)| (k, v.clone())).collect();
    check_iter(|| map.clone().into_iter(), &owned);

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

#[test]
fn subset_and_disjoint() {
    let empty = SparseSet::<u32>::new();
    let small = SparseSet::from([1, 2]);
    let large = SparseSet::from([3, 2, 1]);
    let other = SparseSet::from([7, 8, 9, 10]);

    assert!(empty.is_subset(&small));
    assert!(small.is_subset(&small));
    assert!(small.is_subset(&large));
    assert!(!large.is_subset(&small));
    assert!(!SparseSet::from([1, 4]).is_subset(&large));

    assert!(empty.is_disjoint(&empty));
    assert!(small.is_disjoint(&other));
    assert!(other.is_disjoint(&small));
    assert!(!large.is_disjoint(&small));
    assert!(!SparseSet::from([10, 3]).is_disjoint(&large));
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
