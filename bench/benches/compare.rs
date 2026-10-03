//! Compares `sparsley` with its previous implementation, other sparse sets and
//! a `Vec<Option<_>>` baseline.
//!
//! Each workload and size is a group named `{workload}/{n}` holding one
//! function per store, with `n` elements of throughput.

mod support;

use std::hint::black_box;
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::{
    BatchSize, BenchmarkGroup, Criterion, SamplingMode, Throughput, criterion_group, criterion_main,
};
use support::check::check;
use support::keys::Keys;
use support::store::{Bevy, Cranelift, Reference, Sparsley, Store, VecOption, XSparseSet};

const SIZES: [usize; 3] = [1_000, 100_000, 1_000_000];

#[derive(Clone, Copy)]
enum Workload {
    /// Inserts into a store reserved for the keys and entries.
    Insert,
    /// Inserts into a store without reservation.
    InsertGrow,
    GetHit,
    GetMiss,
    /// Half hits, half misses, in random order.
    ContainsMixed,
    Remove,
    /// Removes and reinserts every key.
    Churn,
    IterValues,
    IterPairs,
    /// Clears a full store, then inserts every key again.
    ClearRefill,
}

impl Workload {
    const ALL: [Self; 10] = [
        Self::Insert,
        Self::InsertGrow,
        Self::GetHit,
        Self::GetMiss,
        Self::ContainsMixed,
        Self::Remove,
        Self::Churn,
        Self::IterValues,
        Self::IterPairs,
        Self::ClearRefill,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::InsertGrow => "insert_grow",
            Self::GetHit => "get_hit",
            Self::GetMiss => "get_miss",
            Self::ContainsMixed => "contains_mixed",
            Self::Remove => "remove",
            Self::Churn => "churn",
            Self::IterValues => "iter_values",
            Self::IterPairs => "iter_pairs",
            Self::ClearRefill => "clear_refill",
        }
    }
}

fn insert_all<S: Store>(store: &mut S, keys: &[u32]) {
    for &key in keys {
        store.insert(key, u64::from(key));
    }
}

fn filled<S: Store>(keys: &Keys) -> S {
    let mut store = S::reserved(keys.domain, keys.n);
    insert_all(&mut store, &keys.live);
    store
}

fn sum_gets<S: Store>(store: &S, keys: &[u32]) -> u64 {
    keys.iter().map(|&key| store.get(key).unwrap_or(0)).sum()
}

fn count_contains<S: Store>(store: &S, keys: &[u32]) -> usize {
    keys.iter().filter(|&&key| store.contains(key)).count()
}

fn sum_removes<S: Store>(store: &mut S, keys: &[u32]) -> u64 {
    keys.iter().map(|&key| store.remove(key).unwrap_or(0)).sum()
}

fn churn<S: Store>(store: &mut S, keys: &[u32]) {
    for &key in keys {
        let value = store.remove(key).unwrap_or(0);
        store.insert(key, value);
    }
}

fn run<S: Store>(group: &mut BenchmarkGroup<'_, WallTime>, workload: Workload, keys: &Keys) {
    let reserved = || S::reserved(keys.domain, keys.n);
    let empty = || S::empty(keys.domain);
    let batched = BatchSize::PerIteration;
    // Built on first use, once, so filtered-out benchmarks skip it.
    let mut store = None;
    group.bench_function(S::NAME, |b| match workload {
        Workload::Insert => {
            b.iter_batched_ref(reserved, |s| insert_all(black_box(s), &keys.live), batched);
        }
        Workload::InsertGrow => {
            b.iter_batched_ref(empty, |s| insert_all(black_box(s), &keys.live), batched);
        }
        Workload::Remove => {
            let full = || filled::<S>(keys);
            b.iter_batched_ref(full, |s| sum_removes(black_box(s), &keys.hits), batched);
        }
        _ => {
            let s = store.get_or_insert_with(|| filled::<S>(keys));
            match workload {
                Workload::GetHit => b.iter(|| sum_gets(black_box(&*s), &keys.hits)),
                Workload::GetMiss => b.iter(|| sum_gets(black_box(&*s), &keys.misses)),
                Workload::ContainsMixed => b.iter(|| count_contains(black_box(&*s), &keys.mixed)),
                Workload::Churn => b.iter(|| churn(black_box(&mut *s), &keys.hits)),
                Workload::IterValues => b.iter(|| black_box(&*s).sum_values()),
                Workload::IterPairs => b.iter(|| black_box(&*s).sum_pairs()),
                Workload::ClearRefill => b.iter(|| {
                    let s = black_box(&mut *s);
                    s.clear();
                    insert_all(s, &keys.live);
                }),
                Workload::Insert | Workload::InsertGrow | Workload::Remove => unreachable!(),
            }
        }
    });
}

fn compare(c: &mut Criterion) {
    check::<Sparsley>();
    check::<Reference>();
    check::<XSparseSet>();
    check::<Bevy>();
    check::<Cranelift>();
    check::<VecOption>();

    let key_sets = SIZES.map(Keys::new);
    for workload in Workload::ALL {
        for keys in &key_sets {
            let mut group = c.benchmark_group(format!("{}/{}", workload.name(), keys.n));
            group.throughput(Throughput::Elements(keys.n as u64));
            if keys.n >= 100_000 {
                group.sampling_mode(SamplingMode::Flat);
            }
            run::<Sparsley>(&mut group, workload, keys);
            run::<Reference>(&mut group, workload, keys);
            run::<XSparseSet>(&mut group, workload, keys);
            run::<Bevy>(&mut group, workload, keys);
            run::<Cranelift>(&mut group, workload, keys);
            run::<VecOption>(&mut group, workload, keys);
            group.finish();
        }
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_millis(500));
    targets = compare
}
criterion_main!(benches);
