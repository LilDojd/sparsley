//! Runs one workload on one store in a loop, for `perf stat` and `perf record`.
//!
//! ```sh
//! cargo bench -p sparsley-bench --bench hot -- sparsley get_hit 100000 [reps]
//! ```
//!
//! Prints the median nanoseconds per element over `reps` repetitions.

#[allow(dead_code, reason = "shared with the compare bench")]
mod support;

use std::env;
use std::hint::black_box;
use std::time::Instant;

use support::keys::Keys;
use support::store::{Bevy, Cranelift, Reference, Sparsley, Store, VecOption, XSparseSet};

fn main() {
    let args: Vec<String> = env::args().skip(1).filter(|a| a != "--bench").collect();
    let [store, workload, n, rest @ ..] = args.as_slice() else {
        eprintln!("usage: hot <store> <workload> <n> [reps]");
        std::process::exit(2);
    };
    let keys = Keys::new(n.parse().expect("n"));
    let reps = rest.first().map_or(200, |r| r.parse().expect("reps"));
    let run = match store.as_str() {
        "sparsley" => run::<Sparsley>,
        "reference" => run::<Reference>,
        "xsparseset" => run::<XSparseSet>,
        "bevy_ecs" => run::<Bevy>,
        "cranelift" => run::<Cranelift>,
        "vec_option" => run::<VecOption>,
        other => panic!("unknown store {other}"),
    };
    let ns = run(workload, &keys, reps);
    println!("{store} {workload} {}: {ns:.3} ns/element", keys.n);
}

#[allow(
    clippy::cast_precision_loss,
    reason = "element counts are far below 2^52"
)]
fn run<S: Store>(workload: &str, keys: &Keys, reps: usize) -> f64 {
    let mut store = S::reserved(keys.domain, keys.n);
    for &key in &keys.live {
        store.insert(key, u64::from(key));
    }
    let mut samples: Vec<f64> = (0..reps)
        .map(|_| {
            let mut fresh = match workload {
                "insert" => Some(S::reserved(keys.domain, keys.n)),
                "insert_grow" => Some(S::empty(keys.domain)),
                _ => None,
            };
            let start = Instant::now();
            match workload {
                "insert" | "insert_grow" => {
                    let fresh = fresh.as_mut().expect("fresh store");
                    for &key in &keys.live {
                        fresh.insert(key, u64::from(key));
                    }
                    black_box(fresh);
                }
                "get_hit" => sum(keys.hits.iter().map(|&k| store.get(k).unwrap_or(0))),
                "get_miss" => sum(keys.misses.iter().map(|&k| store.get(k).unwrap_or(0))),
                "contains" => sum(keys.mixed.iter().map(|&k| u64::from(store.contains(k)))),
                "churn" => {
                    for &key in &keys.hits {
                        let value = store.remove(key).unwrap_or(0);
                        store.insert(key, value);
                    }
                }
                "clear_refill" => {
                    store.clear();
                    for &key in &keys.live {
                        store.insert(key, u64::from(key));
                    }
                }
                "iter_values" => sum([black_box(&store).sum_values()].into_iter()),
                "iter_pairs" => sum([black_box(&store).sum_pairs()].into_iter()),
                other => panic!("unknown workload {other}"),
            }
            let ns = start.elapsed().as_secs_f64() * 1e9 / keys.n as f64;
            drop(fresh);
            ns
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn sum(values: impl Iterator<Item = u64>) {
    black_box(values.sum::<u64>());
}
