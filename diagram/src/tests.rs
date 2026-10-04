//! Parser, model and snapshot tests.

use crate::model::Run;
use crate::parse::{self, Error};
use crate::tokens::lex;
use insta::assert_snapshot;

/// Keys 3, 7, 1, 9 at positions 0..4, filling a capacity of 4.
const MAP: &str = "let mut map = SparseMap::from([(3, 'a'), (7, 'b'), (1, 'c'), (9, 'd')]);";

/// Runs `source` like `diagram!`, returning the text or the error message and
/// the text of the token it points at.
fn expand(source: &str) -> Result<String, (String, Option<String>)> {
    let (trees, texts) = lex(source);
    let run = |trees| -> Result<String, Error> {
        let program = parse::program(trees)?;
        Ok(Run::new(&program)?.diagram())
    };
    run(&trees).map_err(|error| (error.message, error.span.map(|span| texts[span].clone())))
}

/// Runs `source` like `state!`, returning the generated source.
fn state(source: &str) -> String {
    let (trees, _) = lex(source);
    let program = parse::program(&trees).unwrap();
    Run::new(&program).unwrap().state(program.value_type)
}

/// Asserts that `source` fails with `message` at the token `at`.
#[track_caller]
fn rejects(source: &str, message: &str, at: Option<&str>) {
    let (found, token) = expand(source).expect_err("input should be rejected");
    assert_eq!(
        (found.as_str(), token.as_deref()),
        (message, at),
        "{source}"
    );
}

/// Runs `source` like `diagram!`, panicking on errors.
#[track_caller]
fn diagram(source: &str) -> String {
    expand(source).unwrap()
}

#[test]
fn parses_constructors() {
    for source in [
        "let mut map = SparseMap::new();",
        "let mut map = SparseMap::with_capacity(8);",
        "let mut map = SparseMap::from([(1, 2), (3, -4),]);",
        "let mut set = SparseSet::new();",
        "let mut set = SparseSet::from([3usize, 1_0]);",
    ] {
        assert!(expand(source).is_ok(), "{source}");
    }
}

#[test]
fn parses_every_method() {
    let map = [
        "insert(1, 'x')",
        "get(1)",
        "get_mut(1)",
        "contains_key(1)",
        "get_index_of(1)",
        "get_index(0)",
        "remove(1)",
        "remove_entry(3)",
        "swap_remove_index(0)",
        "swap_indices(0, 1)",
        "clear()",
        "retain(|key, _| key % 2 == 0)",
        "retain(|_, v| *v != 'b')",
        "reserve(10)",
        "reserve_keys(100)",
        "shrink_to_fit()",
        "sort_unstable_keys()",
        "get_key_value(1)",
        "entry(1).or_insert('z')",
        "entry(5).or_insert('z')",
    ];
    for call in map {
        let source = format!("{MAP} map.{call};");
        assert!(expand(&source).is_ok(), "{source}: {:?}", expand(&source));
    }
    let set = [
        "insert(1)",
        "contains(1)",
        "remove(1)",
        "swap_remove_index(0)",
        "clear()",
        "retain(|k| k >= 3)",
        "sort_unstable()",
        "get_index(0)",
        "get_index_of(3)",
        "swap_indices(0, 1)",
        "reserve(4)",
        "reserve_keys(9)",
        "shrink_to_fit()",
    ];
    for call in set {
        let source = format!("let mut set = SparseSet::from([3, 7]); set.{call};");
        assert!(expand(&source).is_ok(), "{source}: {:?}", expand(&source));
    }
}

#[test]
fn rejects_bad_input() {
    rejects("", "expected `let`, found the end of the input", None);
    rejects("let map = SparseMap::new();", "expected `mut`", Some("map"));
    rejects(
        "let mut map = HashMap::new();",
        "expected `SparseMap` or `SparseSet`",
        Some("HashMap"),
    );
    rejects(
        "let mut map = SparseMap::default();",
        "expected `new()`, `with_capacity(n)` or `from([...])`",
        Some("default"),
    );
    rejects(
        "let mut map = SparseMap::new()",
        "expected `;`, found the end of the input",
        None,
    );
    rejects(
        &format!("{MAP} other.clear();"),
        "expected a call on `map`",
        Some("other"),
    );
    rejects(
        &format!("{MAP} map.drain();"),
        "`SparseMap::drain` is not supported by the diagram model",
        Some("drain"),
    );
    rejects(
        "let mut set = SparseSet::new(); set.contains_key(1);",
        "`SparseSet::contains_key` is not supported by the diagram model",
        Some("contains_key"),
    );
    rejects(
        &format!("{MAP} map.get(1, 2);"),
        "expected no more arguments",
        Some("2"),
    );
    rejects(
        &format!("{MAP} map.insert(1);"),
        "expected `,`, found the end of the input",
        Some("("),
    );
    rejects(
        &format!("{MAP} map.get(x);"),
        "expected an integer literal",
        Some("x"),
    );
    rejects(
        &format!("{MAP} map.insert(1, 2);"),
        "expected a char value, like the others",
        Some("2"),
    );
    rejects(
        &format!("{MAP} map.insert(1, \"s\");"),
        "expected a char or integer literal",
        Some("\""),
    );
    rejects(
        &format!("{MAP} map.swap_indices(0, 9);"),
        "index out of bounds: the length is 4",
        Some("map"),
    );
}

#[test]
fn rejects_bad_closures() {
    rejects(
        &format!("{MAP} map.retain(|k| k > 1);"),
        "expected `,`",
        Some("|"),
    );
    rejects(
        &format!("{MAP} map.retain(|k, v| x > 1);"),
        "expected a closure parameter",
        Some("x"),
    );
    rejects(
        &format!("{MAP} map.retain(|_, v| v > 'a');"),
        "values are passed by reference; write `*value`",
        Some("v"),
    );
    rejects(
        &format!("{MAP} map.retain(|k, _| *k > 1);"),
        "keys are passed by value; remove the `*`",
        Some("k"),
    );
    rejects(
        &format!("{MAP} map.retain(|k, _| k + 1);"),
        "expected a comparison",
        Some("+"),
    );
    rejects(
        &format!("{MAP} map.retain(|k, _| k > 'a');"),
        "expected an integer literal",
        Some("'a'"),
    );
    rejects(
        &format!("{MAP} map.retain(|_, v| *v % 2 == 0);"),
        "`%` needs integer values",
        Some("%"),
    );
    rejects(
        &format!("{MAP} map.retain(|_, v| *v == 1);"),
        "expected a char value, like the others",
        Some("1"),
    );
}

#[test]
fn growth_follows_the_library() {
    // Dense capacity grows to max(2 * capacity, 4); sparse slots to
    // max(index + 1, 2 * slots, 64).
    let tail = "usize, ";
    let ends_with = |source: &str, end: &str| {
        let state = state(source);
        assert!(state.ends_with(end), "{source}: {state}");
    };
    ends_with(
        "let mut map = SparseMap::new(); map.insert(0, 'a');",
        &format!("1{tail}4{tail}64usize) }}"),
    );
    ends_with(
        "let mut map = SparseMap::new(); map.insert(100, 'a');",
        &format!("1{tail}4{tail}101usize) }}"),
    );
    ends_with(
        &format!("{MAP} map.insert(64, 'e');"),
        &format!("5{tail}8{tail}128usize) }}"),
    );
    ends_with(
        "let mut map = SparseMap::with_capacity(5);",
        &format!("0{tail}5{tail}0usize) }}"),
    );
    ends_with(
        &format!("{MAP} map.reserve(1);"),
        &format!("4{tail}8{tail}64usize) }}"),
    );
    ends_with(
        &format!("{MAP} map.reserve(9);"),
        &format!("4{tail}13{tail}64usize) }}"),
    );
    ends_with(
        &format!("{MAP} map.reserve_keys(65);"),
        &format!("4{tail}4{tail}128usize) }}"),
    );
    ends_with(
        &format!("{MAP} map.reserve_keys(64);"),
        &format!("4{tail}4{tail}64usize) }}"),
    );
    ends_with(
        "let mut map = SparseMap::with_capacity(9); map.insert(5, 'a'); map.shrink_to_fit();",
        &format!("1{tail}1{tail}6usize) }}"),
    );
    ends_with(
        "let mut set = SparseSet::from([1, 1, 2]);",
        &format!("2{tail}3{tail}64usize) }}"),
    );
}

#[test]
fn state_lists_slots_keys_and_values() {
    let map = state("let mut map = SparseMap::from([(2, 5), (0, -1)]); map.shrink_to_fit();");
    assert_eq!(
        map,
        "{ let slots: &[::core::option::Option<usize>] = &[::core::option::Option::Some(1), \
         ::core::option::Option::None, ::core::option::Option::Some(0)]; let keys: &[usize] = &[2, 0]; \
         let values: &[i64] = &[5, -1]; (slots, keys, values, 2usize, 2usize, 3usize) }"
    );
    let set = state("let mut set = SparseSet::from([4]); set.sort_unstable();");
    assert!(
        set.contains(
            "let keys: &[usize] = &[4]; let values: &[()] = &[]; (slots, keys, values, 1usize"
        ),
        "{set}"
    );
}

#[test]
fn retain_visits_in_reverse_and_swap_removes() {
    // Removing 1 (position 2) first moves 9 to 2; removing 3 then moves 9 to 0.
    let state = state(&format!("{MAP} map.retain(|key, _| key > 5);"));
    assert!(
        state.contains("let keys: &[usize] = &[9, 7]; let values: &[char] = &['d', 'b'];"),
        "{state}"
    );
}

#[test]
fn layout_without_calls() {
    assert_snapshot!(diagram("let mut set = SparseSet::with_capacity(2);"), @"
    ```text
      sparse  (none)
                0   1
              ┌───┬───┐
      keys    │░░░│░░░│
              └───┴───┘

      len 0, capacity 2, key_capacity 0
    ```
    ");
}

#[test]
fn grow() {
    assert_snapshot!(diagram(&format!("{MAP} map.insert(12, 'e'); map.insert(70, 'f');")), @"
    ```text
    map.insert(12, 'e') -> None
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┲━━━┱───┬───┬───┐
      sparse  │   │ 3 │   │ 1 │   │   │   │ 2 │   │ 4 │   │   ┃ 5 ┃   │   │   │ …
              └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┺━━━┹───┴───┴───┘
                                ╭───────────────────────────────╯
                0   1   2   3   ▼   5   6   7
              ┌───┬───┬───┬───┲━━━┳━━━┳━━━┳━━━┓
      keys    │ 3 │ 7 │ 1 │ 9 ┃12 ┃░░░┃░░░┃░░░┃
              ├───┼───┼───┼───╊━━━╋━━━╋━━━╋━━━┫
      values  │ a │ b │ c │ d ┃ e ┃░░░┃░░░┃░░░┃
              └───┴───┴───┴───┺━━━┻━━━┻━━━┻━━━┛

      len 4 -> 5, capacity 4 -> 8, key_capacity 64
      sparse[12]  0 -> 5  key 12 pushed at index 4

    map.insert(70, 'f') -> None
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   │ 1 │   │   │   │ 2 │   │ 4 │   │   │ 5 │   │   │   │ …
              └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
                0   1   2   3   4   5   6   7
              ┌───┬───┬───┬───┬───┲━━━┱───┬───┐
      keys    │ 3 │ 7 │ 1 │ 9 │12 ┃70 ┃░░░│░░░│
              ├───┼───┼───┼───┼───╊━━━╉───┼───┤
      values  │ a │ b │ c │ d │ e ┃ f ┃░░░│░░░│
              └───┴───┴───┴───┴───┺━━━┹───┴───┘

      len 5 -> 6, capacity 8, key_capacity 64 -> 128
      sparse[70]  0 -> 6  key 70 pushed at index 5
    ```
    ");
}

#[test]
fn insert() {
    assert_snapshot!(diagram("let mut map = SparseMap::new(); map.insert(3, 'a'); map.insert(7, 'b');"), @"
    ```text
    map.insert(3, 'a') -> None
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┏━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┓
      sparse  ┃   ┃   ┃   ┃ 1 ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃ …
              ┗━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┛
                ╭───────────╯
                ▼   1   2   3
              ┏━━━┳━━━┳━━━┳━━━┓
      keys    ┃ 3 ┃░░░┃░░░┃░░░┃
              ┣━━━╋━━━╋━━━╋━━━┫
      values  ┃ a ┃░░░┃░░░┃░░░┃
              ┗━━━┻━━━┻━━━┻━━━┛

      len 0 -> 1, capacity 0 -> 4, key_capacity 0 -> 64
      sparse[3]  0 -> 1  key 3 pushed at index 0

    map.insert(7, 'b') -> None
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┬───┬───┲━━━┱───┬───┬───┬───┬───┬───┬───┬───┐
      sparse  │   │   │   │ 1 │   │   │   ┃ 2 ┃   │   │   │   │   │   │   │   │ …
              └───┴───┴───┴───┴───┴───┴───┺━━━┹───┴───┴───┴───┴───┴───┴───┴───┘
                    ╭───────────────────────╯
                0   ▼   2   3
              ┌───┲━━━┱───┬───┐
      keys    │ 3 ┃ 7 ┃░░░│░░░│
              ├───╊━━━╉───┼───┤
      values  │ a ┃ b ┃░░░│░░░│
              └───┺━━━┹───┴───┘

      len 1 -> 2, capacity 4, key_capacity 64
      sparse[7]  0 -> 2  key 7 pushed at index 1
    ```
    ");
}

#[test]
fn replace() {
    assert_snapshot!(diagram(&format!("{MAP} map.insert(7, 'B');")), @"
    ```text
    map.insert(7, 'B') -> Some('b')
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   │ 1 │   │   │   │ 2 │   │ 4 │   │   │   │   │   │   │ …
              └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
                    ╭───────────────────────╯
                0   ▼   2   3
              ┌───┬───┬───┬───┐
      keys    │ 3 │ 7 │ 1 │ 9 │
              ├───╆━━━╅───┼───┤
      values  │ a ┃ B ┃ c │ d │
              └───┺━━━┹───┴───┘

      len 4, capacity 4, key_capacity 64
      sparse[7] = 2 -> keys[1] = 7, values[1] = B
      values[1]  b -> B  key 7 replaced
    ```
    ");
}

#[test]
fn remove_middle() {
    assert_snapshot!(diagram(&format!("{MAP} map.remove(7);")), @"
    ```text
    map.remove(7) -> Some('b')
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┬───┬───┲━━━┱───┲━━━┱───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   │ 1 │   │   │   ┃   ┃   ┃ 2 ┃   │   │   │   │   │   │ …
              └───┴───┴───┴───┴───┴───┴───┺━━━┹───┺━━━┹───┴───┴───┴───┴───┴───┘
                    ╭───────────────────────────────╯
                0   ▼   2   3
              ┌───┲━━━┱───┲━━━┓
      keys    │ 3 ┃ 9 ┃ 1 ┃░░░┃
              ├───╊━━━╉───╊━━━┫
      values  │ a ┃ d ┃ c ┃░░░┃
              └───┺━━━┹───┺━━━┛

      len 4 -> 3, capacity 4, key_capacity 64
      sparse[7]  2 -> 0  key 7 removed from index 1
      sparse[9]  4 -> 2  key 9 moved from index 3 to 1
    ```
    ");
}

#[test]
fn remove_last() {
    assert_snapshot!(diagram(&format!("{MAP} map.remove(9);")), @"
    ```text
    map.remove(9) -> Some('d')
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┬───┬───┬───┬───┲━━━┱───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   │ 1 │   │   │   │ 2 │   ┃   ┃   │   │   │   │   │   │ …
              └───┴───┴───┴───┴───┴───┴───┴───┴───┺━━━┹───┴───┴───┴───┴───┴───┘
                0   1   2   3
              ┌───┬───┬───┲━━━┓
      keys    │ 3 │ 7 │ 1 ┃░░░┃
              ├───┼───┼───╊━━━┫
      values  │ a │ b │ c ┃░░░┃
              └───┴───┴───┺━━━┛

      len 4 -> 3, capacity 4, key_capacity 64
      sparse[9]  4 -> 0  key 9 removed from index 3
    ```
    ");
}

#[test]
fn clear() {
    assert_snapshot!(diagram(&format!("{MAP} map.clear();")), @"
    ```text
    map.clear()
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┲━━━┱───┲━━━┱───┬───┬───┲━━━┱───┲━━━┱───┬───┬───┬───┬───┬───┐
      sparse  │   ┃   ┃   ┃   ┃   │   │   ┃   ┃   ┃   ┃   │   │   │   │   │   │ …
              └───┺━━━┹───┺━━━┹───┴───┴───┺━━━┹───┺━━━┹───┴───┴───┴───┴───┴───┘
                0   1   2   3
              ┏━━━┳━━━┳━━━┳━━━┓
      keys    ┃░░░┃░░░┃░░░┃░░░┃
              ┣━━━╋━━━╋━━━╋━━━┫
      values  ┃░░░┃░░░┃░░░┃░░░┃
              ┗━━━┻━━━┻━━━┻━━━┛

      len 4 -> 0, capacity 4, key_capacity 64
      sparse[3]  1 -> 0  key 3 removed from index 0
      sparse[7]  2 -> 0  key 7 removed from index 1
      sparse[1]  3 -> 0  key 1 removed from index 2
      sparse[9]  4 -> 0  key 9 removed from index 3
    ```
    ");
}

#[test]
fn retain() {
    assert_snapshot!(diagram(&format!("{MAP} map.retain(|key, _| key > 2);")), @"
    ```text
    map.retain(|key, _| key > 2)
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┲━━━┱───┬───┬───┬───┬───┬───┬───┲━━━┱───┬───┬───┬───┬───┬───┐
      sparse  │   ┃   ┃   │ 1 │   │   │   │ 2 │   ┃ 3 ┃   │   │   │   │   │   │ …
              └───┺━━━┹───┴───┴───┴───┴───┴───┴───┺━━━┹───┴───┴───┴───┴───┴───┘
                        ╭───────────────────────────╯
                0   1   ▼   3
              ┌───┬───┲━━━┳━━━┓
      keys    │ 3 │ 7 ┃ 9 ┃░░░┃
              ├───┼───╊━━━╋━━━┫
      values  │ a │ b ┃ d ┃░░░┃
              └───┴───┺━━━┻━━━┛

      len 4 -> 3, capacity 4, key_capacity 64
      sparse[1]  3 -> 0  key 1 removed from index 2
      sparse[9]  4 -> 3  key 9 moved from index 3 to 2
    ```
    ");
}

#[test]
fn lookup_miss() {
    assert_snapshot!(diagram(&format!("{MAP} map.get(4);")), @"
    ```text
    map.get(4) -> None
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   │ 1 │   │   │   │ 2 │   │ 4 │   │   │   │   │   │   │ …
              └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
                0   1   2   3
              ┌───┬───┬───┬───┐
      keys    │ 3 │ 7 │ 1 │ 9 │
              ├───┼───┼───┼───┤
      values  │ a │ b │ c │ d │
              └───┴───┴───┴───┘

      len 4, capacity 4, key_capacity 64
      sparse[4] = 0 -> no entry
    ```
    ");
}

#[test]
fn set_remove() {
    assert_snapshot!(diagram("let mut set = SparseSet::from([3, 7, 1, 9]); set.remove(7);"), @"
    ```text
    set.remove(7) -> true
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┬───┬───┲━━━┱───┲━━━┱───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   │ 1 │   │   │   ┃   ┃   ┃ 2 ┃   │   │   │   │   │   │ …
              └───┴───┴───┴───┴───┴───┴───┺━━━┹───┺━━━┹───┴───┴───┴───┴───┴───┘
                    ╭───────────────────────────────╯
                0   ▼   2   3
              ┌───┲━━━┱───┲━━━┓
      keys    │ 3 ┃ 9 ┃ 1 ┃░░░┃
              └───┺━━━┹───┺━━━┛

      len 4 -> 3, capacity 4, key_capacity 64
      sparse[7]  2 -> 0  key 7 removed from index 1
      sparse[9]  4 -> 2  key 9 moved from index 3 to 1
    ```
    ");
}

#[test]
fn swap_remove_index() {
    assert_snapshot!(diagram(&format!("{MAP} map.swap_remove_index(0);")), @"
    ```text
    map.swap_remove_index(0) -> Some((3, 'a'))
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┲━━━┱───┬───┬───┬───┬───┲━━━┱───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   ┃   ┃   │   │   │ 2 │   ┃ 1 ┃   │   │   │   │   │   │ …
              └───┴───┴───┺━━━┹───┴───┴───┴───┴───┺━━━┹───┴───┴───┴───┴───┴───┘
                ╭───────────────────────────────────╯
                ▼   1   2   3
              ┏━━━┱───┬───┲━━━┓
      keys    ┃ 9 ┃ 7 │ 1 ┃░░░┃
              ┣━━━╉───┼───╊━━━┫
      values  ┃ d ┃ b │ c ┃░░░┃
              ┗━━━┹───┴───┺━━━┛

      len 4 -> 3, capacity 4, key_capacity 64
      sparse[3]  1 -> 0  key 3 removed from index 0
      sparse[9]  4 -> 1  key 9 moved from index 3 to 0
    ```
    ");
}

#[test]
fn swap_indices() {
    assert_snapshot!(diagram(&format!("{MAP} map.swap_indices(0, 3);")), @"
    ```text
    map.swap_indices(0, 3)
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┲━━━┱───┬───┬───┬───┬───┲━━━┱───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   ┃ 4 ┃   │   │   │ 2 │   ┃ 1 ┃   │   │   │   │   │   │ …
              └───┴───┴───┺━━━┹───┴───┴───┴───┴───┺━━━┹───┴───┴───┴───┴───┴───┘
                0   1   2   3
              ┏━━━┱───┬───┲━━━┓
      keys    ┃ 9 ┃ 7 │ 1 ┃ 3 ┃
              ┣━━━╉───┼───╊━━━┫
      values  ┃ d ┃ b │ c ┃ a ┃
              ┗━━━┹───┴───┺━━━┛

      len 4, capacity 4, key_capacity 64
      sparse[9]  4 -> 1  key 9 moved from index 3 to 0
      sparse[3]  1 -> 4  key 3 moved from index 0 to 3
    ```
    ");
}

#[test]
fn sort_unstable_keys() {
    assert_snapshot!(diagram(&format!("{MAP} map.sort_unstable_keys();")), @"
    ```text
    map.sort_unstable_keys()
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┲━━━┱───┲━━━┱───┬───┬───┲━━━┱───┬───┬───┬───┬───┬───┬───┬───┐
      sparse  │   ┃ 1 ┃   ┃ 2 ┃   │   │   ┃ 3 ┃   │ 4 │   │   │   │   │   │   │ …
              └───┺━━━┹───┺━━━┹───┴───┴───┺━━━┹───┴───┴───┴───┴───┴───┴───┴───┘
                0   1   2   3
              ┏━━━┳━━━┳━━━┱───┐
      keys    ┃ 1 ┃ 3 ┃ 7 ┃ 9 │
              ┣━━━╋━━━╋━━━╉───┤
      values  ┃ c ┃ a ┃ b ┃ d │
              ┗━━━┻━━━┻━━━┹───┘

      len 4, capacity 4, key_capacity 64
      sparse[1]  3 -> 1  key 1 moved from index 2 to 0
      sparse[3]  1 -> 2  key 3 moved from index 0 to 1
      sparse[7]  2 -> 3  key 7 moved from index 1 to 2
    ```
    ");
}

#[test]
fn entry_or_insert() {
    assert_snapshot!(diagram(&format!("{MAP} map.entry(5).or_insert('e');")), @"
    ```text
    map.entry(5).or_insert('e') -> 'e'
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┲━━━┱───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   │ 1 │   ┃ 5 ┃   │ 2 │   │ 4 │   │   │   │   │   │   │ …
              └───┴───┴───┴───┴───┺━━━┹───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
                                ╭───╯
                0   1   2   3   ▼   5   6   7
              ┌───┬───┬───┬───┲━━━┳━━━┳━━━┳━━━┓
      keys    │ 3 │ 7 │ 1 │ 9 ┃ 5 ┃░░░┃░░░┃░░░┃
              ├───┼───┼───┼───╊━━━╋━━━╋━━━╋━━━┫
      values  │ a │ b │ c │ d ┃ e ┃░░░┃░░░┃░░░┃
              └───┴───┴───┴───┺━━━┻━━━┻━━━┻━━━┛

      len 4 -> 5, capacity 4 -> 8, key_capacity 64
      sparse[5]  0 -> 5  key 5 pushed at index 4
    ```
    ");
}

#[test]
fn reserve_and_shrink() {
    assert_snapshot!(diagram(&format!("{MAP} map.reserve(4); map.shrink_to_fit();")), @"
    ```text
    map.reserve(4)
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   │ 1 │   │   │   │ 2 │   │ 4 │   │   │   │   │   │   │ …
              └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
                0   1   2   3   4   5   6   7
              ┌───┬───┬───┬───┲━━━┳━━━┳━━━┳━━━┓
      keys    │ 3 │ 7 │ 1 │ 9 ┃░░░┃░░░┃░░░┃░░░┃
              ├───┼───┼───┼───╊━━━╋━━━╋━━━╋━━━┫
      values  │ a │ b │ c │ d ┃░░░┃░░░┃░░░┃░░░┃
              └───┴───┴───┴───┺━━━┻━━━┻━━━┻━━━┛

      len 4, capacity 4 -> 8, key_capacity 64

    map.shrink_to_fit()
                0   1   2   3   4   5   6   7   8   9
              ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
      sparse  │   │ 3 │   │ 1 │   │   │   │ 2 │   │ 4 │
              └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
                0   1   2   3
              ┌───┬───┬───┬───┐
      keys    │ 3 │ 7 │ 1 │ 9 │
              ├───┼───┼───┼───┤
      values  │ a │ b │ c │ d │
              └───┴───┴───┴───┘

      len 4, capacity 8 -> 4, key_capacity 64 -> 10
    ```
    ");
}

#[test]
fn set_insert() {
    assert_snapshot!(diagram("let mut set = SparseSet::new(); set.insert(2); set.insert(2);"), @"
    ```text
    set.insert(2) -> true
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┏━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┓
      sparse  ┃   ┃   ┃ 1 ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃   ┃ …
              ┗━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┛
                ╭───────╯
                ▼   1   2   3
              ┏━━━┳━━━┳━━━┳━━━┓
      keys    ┃ 2 ┃░░░┃░░░┃░░░┃
              ┗━━━┻━━━┻━━━┻━━━┛

      len 0 -> 1, capacity 0 -> 4, key_capacity 0 -> 64
      sparse[2]  0 -> 1  key 2 pushed at index 0

    set.insert(2) -> false
                0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15
              ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
      sparse  │   │   │ 1 │   │   │   │   │   │   │   │   │   │   │   │   │   │ …
              └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
                ╭───────╯
                ▼   1   2   3
              ┌───┬───┬───┬───┐
      keys    │ 2 │░░░│░░░│░░░│
              └───┴───┴───┴───┘

      len 1, capacity 4, key_capacity 64
      sparse[2] = 1 -> keys[0] = 2
    ```
    ");
}
