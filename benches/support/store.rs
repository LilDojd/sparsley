#![allow(
    clippy::inline_always,
    reason = "adapters must vanish so only the store is measured"
)]

use cranelift_entity::{EntityRef, SparseMapValue};

use super::reference;

/// The operations every benchmarked map supports, keyed by `u32`.
pub(crate) trait Store {
    const NAME: &'static str;

    /// An empty store with room for keys below `domain` and for `entries`
    /// values, as far as its API allows.
    fn reserved(domain: usize, entries: usize) -> Self;

    /// An empty store with the least setup its API requires.
    fn empty(domain: usize) -> Self;

    fn insert(&mut self, key: u32, value: u64);
    fn get(&self, key: u32) -> Option<u64>;
    fn contains(&self, key: u32) -> bool;
    fn remove(&mut self, key: u32) -> Option<u64>;
    fn clear(&mut self);
    fn sum_values(&self) -> u64;
    fn sum_pairs(&self) -> u64;
}

#[allow(clippy::cast_possible_truncation, reason = "keys are `u32`")]
const fn key(index: usize) -> u32 {
    index as u32
}

const fn index(key: u32) -> usize {
    key as usize
}

/// This crate.
pub(crate) type Sparsley = sparsley::SparseMap<u32, u64>;

impl Store for Sparsley {
    const NAME: &'static str = "sparsley";

    #[inline(always)]
    fn reserved(domain: usize, entries: usize) -> Self {
        let mut map = Self::with_capacity(entries);
        map.reserve_keys(domain);
        map
    }

    #[inline(always)]
    fn empty(_: usize) -> Self {
        Self::new()
    }

    #[inline(always)]
    fn insert(&mut self, key: u32, value: u64) {
        self.insert(key, value);
    }

    #[inline(always)]
    fn get(&self, key: u32) -> Option<u64> {
        self.get(key).copied()
    }

    #[inline(always)]
    fn contains(&self, key: u32) -> bool {
        self.contains_key(key)
    }

    #[inline(always)]
    fn remove(&mut self, key: u32) -> Option<u64> {
        self.remove(key)
    }

    #[inline(always)]
    fn clear(&mut self) {
        self.clear();
    }

    #[inline(always)]
    fn sum_values(&self) -> u64 {
        self.values().iter().sum()
    }

    #[inline(always)]
    fn sum_pairs(&self) -> u64 {
        self.iter().map(|(k, &v)| u64::from(k) + v).sum()
    }
}

/// The previous implementation: `usize` keys, fixed key capacity.
pub(crate) type Reference = reference::SparseSet<u64>;

impl Store for Reference {
    const NAME: &'static str = "reference";

    #[inline(always)]
    fn reserved(domain: usize, entries: usize) -> Self {
        Self::with_capacities(domain, entries)
    }

    #[inline(always)]
    fn empty(domain: usize) -> Self {
        Self::with_capacity(domain)
    }

    #[inline(always)]
    fn insert(&mut self, key: u32, value: u64) {
        self.insert(index(key), value);
    }

    #[inline(always)]
    fn get(&self, key: u32) -> Option<u64> {
        self.get(index(key)).copied()
    }

    #[inline(always)]
    fn contains(&self, key: u32) -> bool {
        self.contains(index(key))
    }

    #[inline(always)]
    fn remove(&mut self, key: u32) -> Option<u64> {
        self.remove(index(key))
    }

    #[inline(always)]
    fn clear(&mut self) {
        self.clear();
    }

    #[inline(always)]
    fn sum_values(&self) -> u64 {
        self.values().iter().sum()
    }

    #[inline(always)]
    fn sum_pairs(&self) -> u64 {
        self.iter().map(|(k, &v)| k as u64 + v).sum()
    }
}

/// `xsparseset`: no reservation API, `Into<usize>` keys.
pub(crate) type XSparseSet = xsparseset::SparseSetVec<usize, u64>;

impl Store for XSparseSet {
    const NAME: &'static str = "xsparseset";

    #[inline(always)]
    fn reserved(_: usize, _: usize) -> Self {
        Self::default()
    }

    #[inline(always)]
    fn empty(_: usize) -> Self {
        Self::default()
    }

    #[inline(always)]
    fn insert(&mut self, key: u32, value: u64) {
        self.insert(index(key), value);
    }

    #[inline(always)]
    fn get(&self, key: u32) -> Option<u64> {
        self.get(index(key)).copied()
    }

    #[inline(always)]
    fn contains(&self, key: u32) -> bool {
        self.contains(index(key))
    }

    #[inline(always)]
    fn remove(&mut self, key: u32) -> Option<u64> {
        self.swap_remove_by_id(index(key))
    }

    #[inline(always)]
    fn clear(&mut self) {
        self.clear();
    }

    #[inline(always)]
    fn sum_values(&self) -> u64 {
        self.data().iter().sum()
    }

    #[inline(always)]
    fn sum_pairs(&self) -> u64 {
        let pairs = self.ids().iter().zip(self.data());
        pairs.map(|(&k, &v)| k as u64 + v).sum()
    }
}

/// `bevy_ecs`: reserves dense storage only.
pub(crate) type Bevy = bevy_ecs::storage::SparseSet<u32, u64>;

impl Store for Bevy {
    const NAME: &'static str = "bevy_ecs";

    #[inline(always)]
    fn reserved(_: usize, entries: usize) -> Self {
        Self::with_capacity(entries)
    }

    #[inline(always)]
    fn empty(_: usize) -> Self {
        Self::new()
    }

    #[inline(always)]
    fn insert(&mut self, key: u32, value: u64) {
        self.insert(key, value);
    }

    #[inline(always)]
    fn get(&self, key: u32) -> Option<u64> {
        self.get(key).copied()
    }

    #[inline(always)]
    fn contains(&self, key: u32) -> bool {
        self.contains(key)
    }

    #[inline(always)]
    fn remove(&mut self, key: u32) -> Option<u64> {
        self.remove(key)
    }

    #[inline(always)]
    fn clear(&mut self) {
        self.clear();
    }

    #[inline(always)]
    fn sum_values(&self) -> u64 {
        self.values().sum()
    }

    #[inline(always)]
    fn sum_pairs(&self) -> u64 {
        self.iter().map(|(&k, &v)| u64::from(k) + v).sum()
    }
}

/// Key of the `cranelift-entity` map.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct CraneliftKey(u32);

impl EntityRef for CraneliftKey {
    #[inline(always)]
    fn new(index: usize) -> Self {
        Self(key(index))
    }

    #[inline(always)]
    fn index(self) -> usize {
        index(self.0)
    }
}

/// Value of the `cranelift-entity` map, which must store its own key.
pub(crate) struct CraneliftEntry {
    key: CraneliftKey,
    value: u64,
}

impl SparseMapValue<CraneliftKey> for CraneliftEntry {
    #[inline(always)]
    fn key(&self) -> CraneliftKey {
        self.key
    }
}

/// `cranelift-entity`: `AoS` dense, no reservation API, O(1) clear.
pub(crate) type Cranelift = cranelift_entity::SparseMap<CraneliftKey, CraneliftEntry>;

impl Store for Cranelift {
    const NAME: &'static str = "cranelift";

    #[inline(always)]
    fn reserved(_: usize, _: usize) -> Self {
        Self::new()
    }

    #[inline(always)]
    fn empty(_: usize) -> Self {
        Self::new()
    }

    #[inline(always)]
    fn insert(&mut self, key: u32, value: u64) {
        self.insert(CraneliftEntry {
            key: CraneliftKey(key),
            value,
        });
    }

    #[inline(always)]
    fn get(&self, key: u32) -> Option<u64> {
        self.get(CraneliftKey(key)).map(|entry| entry.value)
    }

    #[inline(always)]
    fn contains(&self, key: u32) -> bool {
        self.contains_key(CraneliftKey(key))
    }

    #[inline(always)]
    fn remove(&mut self, key: u32) -> Option<u64> {
        self.remove(CraneliftKey(key)).map(|entry| entry.value)
    }

    #[inline(always)]
    fn clear(&mut self) {
        self.clear();
    }

    #[inline(always)]
    fn sum_values(&self) -> u64 {
        self.as_slice().iter().map(|entry| entry.value).sum()
    }

    #[inline(always)]
    fn sum_pairs(&self) -> u64 {
        let pairs = self.as_slice().iter();
        pairs
            .map(|entry| u64::from(entry.key.0) + entry.value)
            .sum()
    }
}

/// Naive baseline: one optional value per key, iteration scans holes.
pub(crate) struct VecOption(Vec<Option<u64>>);

impl Store for VecOption {
    const NAME: &'static str = "vec_option";

    #[inline(always)]
    fn reserved(domain: usize, _: usize) -> Self {
        Self(vec![None; domain])
    }

    #[inline(always)]
    fn empty(_: usize) -> Self {
        Self(Vec::new())
    }

    #[inline(always)]
    fn insert(&mut self, key: u32, value: u64) {
        let index = index(key);
        if index >= self.0.len() {
            self.0.resize(index + 1, None);
        }
        self.0[index] = Some(value);
    }

    #[inline(always)]
    fn get(&self, key: u32) -> Option<u64> {
        self.0.get(index(key)).copied().flatten()
    }

    #[inline(always)]
    fn contains(&self, key: u32) -> bool {
        matches!(self.0.get(index(key)), Some(Some(_)))
    }

    #[inline(always)]
    fn remove(&mut self, key: u32) -> Option<u64> {
        self.0.get_mut(index(key))?.take()
    }

    #[inline(always)]
    fn clear(&mut self) {
        self.0.fill(None);
    }

    #[inline(always)]
    fn sum_values(&self) -> u64 {
        self.0.iter().flatten().sum()
    }

    #[inline(always)]
    fn sum_pairs(&self) -> u64 {
        let pairs = self.0.iter().enumerate();
        pairs.filter_map(|(k, v)| Some(k as u64 + (*v)?)).sum()
    }
}
