//! Model-based tests against `BTreeMap` and `BTreeSet`.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use proptest::test_runner::Config;
use sparsley::map::Entry;
use sparsley::{SparseMap, SparseSet};

const LARGE: u32 = if cfg!(miri) { 1 << 12 } else { 1 << 20 };
const MAX_OPS: usize = if cfg!(miri) { 16 } else { 128 };

fn config() -> Config {
    if cfg!(miri) {
        Config {
            cases: 8,
            failure_persistence: None,
            ..Config::default()
        }
    } else {
        Config::default()
    }
}

fn key() -> impl Strategy<Value = u32> {
    prop_oneof![15 => 0..256u32, 1 => LARGE..LARGE + 4]
}

fn probe_keys() -> impl Iterator<Item = u32> {
    (0..256).chain(LARGE..LARGE + 4)
}

#[derive(Debug, Clone)]
enum MapOp {
    Insert(u32, u32),
    Remove(u32),
    RemoveEntry(u32),
    GetMut(u32, u32),
    OrInsert(u32, u32),
    AndModify(u32),
    EntryRemove(u32),
    Retain(u32),
    Clear,
    Drain(usize),
    Extend(Vec<(u32, u32)>),
    ShrinkToFit,
    GetDisjointMut(u32, u32),
    ReserveKeys(usize),
}

fn map_op() -> impl Strategy<Value = MapOp> {
    prop_oneof![
        8 => (key(), any::<u32>()).prop_map(|(k, v)| MapOp::Insert(k, v)),
        3 => key().prop_map(MapOp::Remove),
        2 => key().prop_map(MapOp::RemoveEntry),
        2 => (key(), any::<u32>()).prop_map(|(k, v)| MapOp::GetMut(k, v)),
        2 => (key(), any::<u32>()).prop_map(|(k, v)| MapOp::OrInsert(k, v)),
        2 => key().prop_map(MapOp::AndModify),
        2 => key().prop_map(MapOp::EntryRemove),
        1 => (1..5u32).prop_map(MapOp::Retain),
        1 => Just(MapOp::Clear),
        1 => (0..8usize).prop_map(MapOp::Drain),
        2 => prop::collection::vec((key(), any::<u32>()), 0..8).prop_map(MapOp::Extend),
        1 => Just(MapOp::ShrinkToFit),
        2 => (key(), key()).prop_map(|(a, b)| MapOp::GetDisjointMut(a, b)),
        1 => (0..300usize).prop_map(MapOp::ReserveKeys),
    ]
}

fn apply(map: &mut SparseMap<u32, u32>, model: &mut BTreeMap<u32, u32>, op: MapOp) {
    match op {
        MapOp::Insert(k, v) => assert_eq!(map.insert(k, v), model.insert(k, v)),
        MapOp::Remove(k) => assert_eq!(map.remove(k), model.remove(&k)),
        MapOp::RemoveEntry(k) => assert_eq!(map.remove_entry(k), model.remove_entry(&k)),
        MapOp::GetMut(k, v) => {
            if let Some(slot) = map.get_mut(k) {
                *slot = v;
            }
            if let Some(slot) = model.get_mut(&k) {
                *slot = v;
            }
        }
        MapOp::OrInsert(k, v) => {
            let slot = map.entry(k).or_insert(v);
            *slot = slot.wrapping_add(1);
            let slot = model.entry(k).or_insert(v);
            *slot = slot.wrapping_add(1);
        }
        MapOp::AndModify(k) => {
            let entry = map.entry(k).and_modify(|v| *v ^= 1);
            assert_eq!(entry.key(), k);
            if let Some(v) = model.get_mut(&k) {
                *v ^= 1;
            }
        }
        MapOp::EntryRemove(k) => match map.entry(k) {
            Entry::Occupied(entry) => {
                assert_eq!(entry.key(), k);
                assert_eq!(Some(entry.remove()), model.remove(&k));
            }
            Entry::Vacant(entry) => {
                assert_eq!(entry.key(), k);
                assert!(!model.contains_key(&k));
            }
        },
        MapOp::Retain(m) => {
            let keep = |k: u32, v: &mut u32| {
                *v = v.wrapping_add(1);
                !(k ^ *v).is_multiple_of(m)
            };
            map.retain(keep);
            model.retain(|&k, v| keep(k, v));
        }
        MapOp::Clear => {
            map.clear();
            model.clear();
        }
        MapOp::Drain(n) => {
            let mut drain = map.drain();
            assert_eq!(drain.len(), model.len());
            for (k, v) in drain.by_ref().take(n) {
                assert_eq!(model.get(&k), Some(&v));
            }
            drop(drain);
            model.clear();
        }
        MapOp::Extend(entries) => {
            map.extend(entries.iter().copied());
            model.extend(entries);
        }
        MapOp::ShrinkToFit => {
            map.shrink_to_fit();
            let end = model.last_key_value().map_or(0, |(&k, _)| k as usize + 1);
            assert_eq!(map.key_capacity(), end);
        }
        MapOp::GetDisjointMut(a, b) => {
            if a == b && model.contains_key(&a) {
                return;
            }
            let [x, y] = map.get_disjoint_mut([a, b]);
            assert_eq!(x.as_deref(), model.get(&a));
            assert_eq!(y.as_deref(), model.get(&b));
            if let (Some(x), Some(y)) = (x, y) {
                std::mem::swap(x, y);
                let (va, vb) = (model[&a], model[&b]);
                model.insert(a, vb);
                model.insert(b, va);
            }
        }
        MapOp::ReserveKeys(end) => {
            map.reserve_keys(end);
            assert!(map.key_capacity() >= end);
        }
    }
}

fn check_map(map: &SparseMap<u32, u32>, model: &BTreeMap<u32, u32>) {
    assert_eq!(map.len(), model.len());
    assert_eq!(map.is_empty(), model.is_empty());
    assert_eq!(map.keys().len(), map.values().len());
    for k in probe_keys() {
        assert_eq!(map.get(k), model.get(&k));
        assert_eq!(map.contains_key(k), model.contains_key(&k));
        let indexed = map.position(k).and_then(|p| map.get_index(p));
        assert_eq!(indexed, model.get(&k).map(|v| (k, v)));
    }
    for (position, (&k, v)) in map.keys().iter().zip(map.values()).enumerate() {
        assert_eq!(map.position(k), Some(position));
        assert_eq!(map.get(k), Some(v));
    }
    let snapshot: BTreeMap<u32, u32> = map.iter().map(|(k, &v)| (k, v)).collect();
    assert_eq!(&snapshot, model);
}

#[derive(Debug, Clone)]
enum SetOp {
    Insert(u32),
    Remove(u32),
    Retain(u32),
    Clear,
    Drain(usize),
    Extend(Vec<u32>),
    ShrinkToFit,
}

fn set_op() -> impl Strategy<Value = SetOp> {
    prop_oneof![
        8 => key().prop_map(SetOp::Insert),
        4 => key().prop_map(SetOp::Remove),
        1 => (1..5u32).prop_map(SetOp::Retain),
        1 => Just(SetOp::Clear),
        1 => (0..8usize).prop_map(SetOp::Drain),
        2 => prop::collection::vec(key(), 0..8).prop_map(SetOp::Extend),
        1 => Just(SetOp::ShrinkToFit),
    ]
}

fn apply_set(set: &mut SparseSet<u32>, model: &mut BTreeSet<u32>, op: SetOp) {
    match op {
        SetOp::Insert(k) => assert_eq!(set.insert(k), model.insert(k)),
        SetOp::Remove(k) => assert_eq!(set.remove(k), model.remove(&k)),
        SetOp::Retain(m) => {
            set.retain(|k| k % m != 0);
            model.retain(|k| k % m != 0);
        }
        SetOp::Clear => {
            set.clear();
            model.clear();
        }
        SetOp::Drain(n) => {
            assert!(set.drain().take(n).all(|k| model.contains(&k)));
            model.clear();
        }
        SetOp::Extend(keys) => {
            set.extend(&keys);
            model.extend(keys);
        }
        SetOp::ShrinkToFit => set.shrink_to_fit(),
    }
}

fn check_set(set: &SparseSet<u32>, model: &BTreeSet<u32>) {
    assert_eq!(set.len(), model.len());
    for k in probe_keys() {
        assert_eq!(set.contains(k), model.contains(&k));
        let found = set.position(k).map(|p| set.as_slice()[p]);
        assert_eq!(found, model.contains(&k).then_some(k));
    }
    assert_eq!(&set.iter().collect::<BTreeSet<_>>(), model);
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn map_matches_btreemap(ops in prop::collection::vec(map_op(), 0..MAX_OPS)) {
        let mut map = SparseMap::new();
        let mut model = BTreeMap::new();
        for op in ops {
            apply(&mut map, &mut model, op);
            check_map(&map, &model);
        }
    }

    #[test]
    fn set_matches_btreeset(ops in prop::collection::vec(set_op(), 0..MAX_OPS)) {
        let mut set = SparseSet::new();
        let mut model = BTreeSet::new();
        for op in ops {
            apply_set(&mut set, &mut model, op);
            check_set(&set, &model);
        }
    }
}
