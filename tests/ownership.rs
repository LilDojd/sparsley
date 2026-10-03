//! Drop accounting, panic safety and weird value types

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use sparsley::SparseMap;

struct Tracked<'a> {
    drops: &'a Cell<usize>,
    explode: bool,
}

impl Drop for Tracked<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        assert!(!self.explode, "boom");
    }
}

fn tracked(
    drops: &Cell<usize>,
    keys: impl IntoIterator<Item = u32>,
) -> SparseMap<u32, Tracked<'_>> {
    keys.into_iter()
        .map(|k| {
            (
                k,
                Tracked {
                    drops,
                    explode: false,
                },
            )
        })
        .collect()
}

fn assert_consistent<V>(map: &SparseMap<u32, V>) {
    assert_eq!(map.len(), map.keys().len());
    assert_eq!(map.len(), map.values().len());
    for (position, &k) in map.keys().iter().enumerate() {
        assert_eq!(map.position(k), Some(position));
        assert!(map.contains_key(k));
    }
}

#[test]
fn every_value_dropped_once() {
    let drops = Cell::new(0);
    let mut map = tracked(&drops, 0..10);

    drop(map.insert(
        3,
        Tracked {
            drops: &drops,
            explode: false,
        },
    ));
    assert_eq!(drops.get(), 1);
    drop(map.remove(4));
    assert_eq!(drops.get(), 2);
    map.retain(|k, _| k % 2 == 0);
    assert_eq!((drops.get(), map.len()), (7, 4));

    let mut drain = map.drain();
    drop(drain.next());
    drop(drain);
    assert_eq!((drops.get(), map.len()), (11, 0));

    map.extend(tracked(&drops, 0..4));
    let mut into_iter = map.into_iter();
    drop(into_iter.next_back());
    drop(into_iter);
    assert_eq!(drops.get(), 15);

    let mut map = tracked(&drops, 0..4);
    map.clear();
    assert_eq!(drops.get(), 19);
    map.extend(tracked(&drops, 0..4));
    drop(map);
    assert_eq!(drops.get(), 23);
}

#[test]
fn leaked_drain_still_empties_map() {
    let drops = Cell::new(0);
    let mut map = tracked(&drops, 0..4);
    std::mem::forget(map.drain());
    assert!(map.is_empty());
    assert!(!map.contains_key(0));
    assert_eq!(drops.get(), 0);
}

#[test]
fn panicking_retain_predicate_leaves_map_usable() {
    let drops = Cell::new(0);
    let mut map = tracked(&drops, 0..10);
    let result = catch_unwind(AssertUnwindSafe(|| {
        map.retain(|k, _| {
            assert_ne!(k, 4, "predicate panic");
            k % 2 == 0
        });
    }));
    assert!(result.is_err());
    assert_consistent(&map);
    assert_eq!(drops.get() + map.len(), 10);
    assert!((0..4).all(|k| map.contains_key(k)));

    map.insert(
        42,
        Tracked {
            drops: &drops,
            explode: false,
        },
    );
    assert_consistent(&map);
    drop(map);
    assert_eq!(drops.get(), 11);
}

#[test]
fn panicking_value_drop_leaves_map_usable() {
    type Op = fn(&mut SparseMap<u32, Tracked<'_>>);
    let cases: [(&str, Op, usize); 3] = [
        ("clear", |map| map.clear(), 0),
        ("retain", |map| map.retain(|k, _| k != 2), 4),
        ("remove", |map| drop(map.remove(2)), 4),
    ];
    for (name, op, remaining) in cases {
        let drops = Cell::new(0);
        let mut map = tracked(&drops, 0..5);
        map[2].explode = true;

        assert!(
            catch_unwind(AssertUnwindSafe(|| op(&mut map))).is_err(),
            "{name}"
        );
        assert_eq!(drops.get(), 5 - remaining, "{name}");
        assert_eq!(map.len(), remaining, "{name}");
        assert!(!map.contains_key(2), "{name}");
        assert_consistent(&map);

        map.insert(
            2,
            Tracked {
                drops: &drops,
                explode: false,
            },
        );
        assert_consistent(&map);
        drop(map);
        assert_eq!(drops.get(), 6, "{name}");
    }
}

#[test]
fn zero_sized_values() {
    thread_local!(static DROPS: Cell<usize> = const { Cell::new(0) });
    struct Unit;
    impl Drop for Unit {
        fn drop(&mut self) {
            DROPS.set(DROPS.get() + 1);
        }
    }

    let mut map: SparseMap<u32, Unit> = (0..100).map(|k| (k, Unit)).collect();
    assert!(map.insert(7, Unit).is_some());
    map.retain(|k, _| k % 3 != 0);
    assert!(map.remove(1).is_some());
    assert_consistent(&map);
    assert_eq!(DROPS.get() + map.len(), 101);
    drop(map.drain().take(5));
    assert_eq!(DROPS.get(), 101);
}

#[test]
fn over_aligned_values() {
    #[derive(Debug, Clone, Copy, PartialEq)]
    #[repr(align(128))]
    struct Aligned(u32);

    let mut map: SparseMap<u32, Aligned> = (0..20).map(|k| (k, Aligned(k))).collect();
    map.retain(|k, _| k % 4 != 0);
    let [a, b] = map.get_disjoint_mut([1, 19]);
    std::mem::swap(a.unwrap(), b.unwrap());
    for (k, v) in &map {
        assert_eq!(std::ptr::from_ref(v).addr() % 128, 0);
        let expected = match k {
            1 => 19,
            19 => 1,
            k => k,
        };
        assert_eq!(*v, Aligned(expected));
    }
}

#[test]
fn mutable_reference_values() {
    let mut backing = [0u32; 6];
    let mut map: SparseMap<u32, &mut u32> =
        backing.iter_mut().zip(0..).map(|(r, k)| (k, r)).collect();
    assert!(map.remove(0).is_some());

    let [a, b, c, missing] = map.get_disjoint_mut([5, 1, 3, 0]);
    let (a, b, c) = (a.unwrap(), b.unwrap(), c.unwrap());
    assert!(missing.is_none());
    **a += 1;
    **b += 2;
    **c += 3;
    **a += **b + **c;
    for (_, v) in &mut map {
        **v *= 10;
    }
    drop(map);
    assert_eq!(backing, [0, 20, 0, 30, 0, 60]);
}

#[test]
fn boxed_values() {
    let mut map: SparseMap<u32, Box<String>> =
        (0..8).map(|k| (k, Box::new(k.to_string()))).collect();
    *map.insert(2, Box::new("two".into())).unwrap() += "!";
    map.retain(|k, v| {
        v.push('x');
        k != 5
    });

    let mut copy = map.clone();
    copy.remove(0);
    copy.clone_from(&map);
    assert_eq!(copy, map);
    assert_eq!(*copy[2], "twox");

    let drained: Vec<_> = copy.drain().take(2).collect();
    assert_eq!(drained.len(), 2);
    assert_eq!(map.into_values().filter(|v| v.ends_with('x')).count(), 7);
}
