use std::collections::BTreeMap;

use super::keys::Rng;
use super::store::Store;

const DOMAIN: usize = 300;

/// Asserts that `S` agrees with a `BTreeMap` on a random operation sequence,
/// for both a reserved and an unreserved store.
pub(crate) fn check<S: Store>() {
    for (mut store, seed) in [(S::reserved(DOMAIN, 64), 1), (S::empty(DOMAIN), 2)] {
        let mut model = BTreeMap::new();
        let mut rng = Rng::new(seed);
        for _ in 0..20_000 {
            let key = u32::try_from(rng.below(DOMAIN)).unwrap();
            match rng.below(100) {
                0..40 => {
                    let value = rng.next() >> 32;
                    store.insert(key, value);
                    model.insert(key, value);
                }
                40..60 => assert_eq!(store.remove(key), model.remove(&key), "{} remove", S::NAME),
                60..80 => assert_eq!(store.get(key), model.get(&key).copied(), "{} get", S::NAME),
                80..99 => assert_eq!(
                    store.contains(key),
                    model.contains_key(&key),
                    "{} contains",
                    S::NAME
                ),
                _ => {
                    store.clear();
                    model.clear();
                }
            }
            assert_eq!(
                store.sum_values(),
                model.values().sum::<u64>(),
                "{} values",
                S::NAME
            );
            assert_eq!(
                store.sum_pairs(),
                model.iter().map(|(&k, &v)| u64::from(k) + v).sum::<u64>(),
                "{} pairs",
                S::NAME
            );
        }
    }
}
