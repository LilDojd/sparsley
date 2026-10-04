//! Expands the macros through the compiler's token streams.

use sparsley_diagram::{diagram, state};

/// Documented by a generated diagram.
#[doc = diagram! {
    let mut map = SparseMap::from([(3, 'a'), (7, 'b')]);
    map.remove(3);
}]
struct Documented;

#[test]
fn diagram_expands_to_a_text_block() {
    const DIAGRAM: &str = diagram! {
        let mut map = SparseMap::with_capacity(2);
        map.insert(5usize, -3);
        map.retain(|_, value| *value % 2 != 0);
    };
    assert!(
        DIAGRAM.starts_with("```text\nmap.insert(5, -3) -> None\n"),
        "{DIAGRAM}"
    );
    assert!(
        DIAGRAM.contains("\nmap.retain(|_, value| *value % 2 != 0)\n"),
        "{DIAGRAM}"
    );
    assert!(DIAGRAM.ends_with("```"));
    let _ = Documented;
}

#[test]
fn state_expands_to_a_typed_tuple() {
    let (slots, keys, values, len, capacity, key_capacity) = state! {
        let mut set = SparseSet::new();
        set.insert(2);
        set.insert(0);
        set.swap_remove_index(0);
    };
    assert_eq!(
        slots,
        [Some(0), None, None]
            .iter()
            .chain(&[None; 61])
            .copied()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        (keys, values, len, capacity, key_capacity),
        (&[0][..], &[][..], 1, 4, 64)
    );
}
