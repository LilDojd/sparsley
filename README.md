<p align="center">
  <img src="assets/sparsley_logo.svg" alt="sparsley" width="420">
</p>

# sparsley

[![crates.io](https://img.shields.io/crates/v/sparsley.svg)](https://crates.io/crates/sparsley)
[![docs.rs](https://img.shields.io/docsrs/sparsley)](https://docs.rs/sparsley)
[![CI](https://github.com/LilDojd/sparsley/actions/workflows/ci.yml/badge.svg)](https://github.com/LilDojd/sparsley/actions/workflows/ci.yml)
[![MSRV 1.92](https://img.shields.io/badge/MSRV-1.92-blue.svg)](Cargo.toml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![no_std](https://img.shields.io/badge/no__std-alloc-success.svg)](https://docs.rs/sparsley)

A [sparse set](https://research.swtch.com/sparse) for Rust: a map from small integer
keys, such as entity ids, to densely packed values.

- Faster insert, lookup and removal than other sparse sets
  ([benchmarks](#benchmarks)).
- Values are stored contiguously: `values()` is a `&[V]`.
- A `HashMap`-like API with entries, iterators and set operations.

```rust
use sparsley::{Key, SparseMap, SparseSet};

#[derive(Clone, Copy)]
struct Entity(u32);

impl Key for Entity {
    fn slot(self) -> usize {
        self.0 as usize
    }
}

let mut velocity = SparseMap::new();
velocity.insert(Entity(7), [1.0_f32, 0.0]);
velocity.insert(Entity(42), [0.0, -9.81]);

for v in velocity.values_mut() {
    v[1] -= 9.81;
}
*velocity.entry(Entity(7)).or_default() = [0.0, 0.0];
assert_eq!(velocity.remove(Entity(42)), Some([0.0, -19.62]));

// Or use a `SparseSet<K>`
let mut alive = SparseSet::new();
alive.insert(3_u32);
assert!(alive.contains(3));
```

## Layout

<!-- diagram: layout -->
```text
            0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
          ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
  sparse  │   │ 3 │   │ 1 │   │   │   │ 2 │   │   │   │   │   │   │   │   │ …
          └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
            0   1   2
          ┌───┬───┬───┐
  keys    │ 3 │ 7 │ 1 │
          ├───┼───┼───┤
  values  │ a │ b │ c │
          └───┴───┴───┘

  len 3, capacity 3, key_capacity 64
```

Removal moves the last entry into the hole and sets the slot to 0

<!-- diagram: remove -->
```text
map.remove(3) -> Some('a')
            0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
          ┌───┲━━━┱───┲━━━┱───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
  sparse  │   ┃ 1 ┃   ┃   ┃   │   │   │ 2 │   │   │   │   │   │   │   │   │ …
          └───┺━━━┹───┺━━━┹───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
            ╭───╯
            ▼   1   2
          ┏━━━┱───┲━━━┓
  keys    ┃ 1 ┃ 7 ┃░░░┃
          ┣━━━╉───╊━━━┫
  values  ┃ c ┃ b ┃░░░┃
          ┗━━━┹───┺━━━┛

  len 3 -> 2, capacity 3, key_capacity 64
  sparse[3]  1 -> 0  key 3 removed from index 0
  sparse[1]  3 -> 1  key 1 moved from index 2 to 0
```

## When to use it

Use sparsley if your keys are small integers that are near each other.
Entity ids and slot-map indices are good examples. With these keys, access
and iteration are fast.

Do not use sparsley if your keys are scattered. Scattered keys are far apart
in a large range, for example hashes, random 64-bit ids or timestamps.
sparsley keeps a 4-byte entry for every key from zero to the largest key, so
it can find any value directly. Thus, one key of 1,000,000,000 needs
approximately 4 GB. Use a hash map for scattered keys.

A map holds a maximum of `u32::MAX` entries.

## Benchmarks

Nanoseconds per element with one million `u32` keys; lower is better.

| | sparsley | xsparseset | bevy_ecs | cranelift |
|---|--:|--:|--:|--:|
| insert | **1.30** | 1.88 | 2.57 | 1.55 |
| get hit | **0.97** | 1.18 | 1.39 | 1.89 |
| get miss | **0.42** | 0.53 | 0.45 | 0.82 |
| remove | **2.15** | 12.40 | 2.86 | 3.03 |
| iterate values | **0.09** | **0.09** | **0.09** | 0.11 |
| clear + refill | 1.42 | 1.51 | 2.35 | **1.13** |

AMD Ryzen 9 9950X3D, Linux 7.2.8, rustc 1.99.0.

Run `cargo bench -p sparsley-bench` to reproduce.

## Safety

The crate uses `unsafe` for the shared dense allocation and unchecked slot
writes. Every block carries a `SAFETY` comment, and CI runs the full test
suite, including property tests against `BTreeMap` and `BTreeSet` models,
under Miri with both stacked and tree borrows, on 64-bit and 32-bit targets:

```sh
cargo test
cargo +nightly miri test
MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test
cargo +nightly miri test --target i686-unknown-linux-gnu
```

## MSRV

Rust 1.92, checked in CI.

## Future work

- Paging for the sparse array. [EnTT](https://github.com/skypjack/entt), for example, splits its
  sparse array into pages of 4096 entries and versions its entities. Its
  lookups do slightly more work, but it allocates only the pages that hold
  keys. The goal is a good tradeoff between performance and use in a real ECS.

## Further reading

sparsley is built on ideas from these two write-ups. Many thanks to their
authors for such clear and detailed explanations.

- Russ Cox, [Using Uninitialized Memory for Fun and Profit](https://research.swtch.com/sparse):
  the classic sparse set, with constant-time insert, lookup, removal and clear.
- Michele Caini (skypjack), [ECS back and forth, part 9](https://skypjack.github.io/2020-08-02-ecs-baf-part-9/):
  sparse sets as storage for ECS components.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
