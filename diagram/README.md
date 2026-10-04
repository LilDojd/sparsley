# sparsley-diagram

Proc macros that draw memory diagrams of [`sparsley`](https://crates.io/crates/sparsley)
operations for its rustdoc. `diagram!` expands to a doc string; `state!` exposes
the modelled state so tests can check the model against the real collections.

```rust,ignore
#[doc = sparsley_diagram::diagram! {
    let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c')]);
    map.remove(3);
}]
```

Not meant as a general-purpose dependency.
