//! Serde wire format, duplicate handling, borrowing and error propagation.
#![cfg(feature = "serde")]

use sparsley::{SparseMap, SparseSet};

#[test]
fn map_json_preserves_dense_order_and_last_value() {
    for (input, canonical, expected) in [
        ("{}", "{}", vec![]),
        (
            r#"{"3":"a","1":"b"}"#,
            r#"{"3":"a","1":"b"}"#,
            vec![(3, 'a'), (1, 'b')],
        ),
        (
            r#"{"3":"a","1":"b","3":"c"}"#,
            r#"{"3":"c","1":"b"}"#,
            vec![(3, 'c'), (1, 'b')],
        ),
    ] {
        let map: SparseMap<u32, char> = serde_json::from_str(input).unwrap();
        assert_eq!(
            map.iter().map(|(k, &v)| (k, v)).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(serde_json::to_string(&map).unwrap(), canonical);
    }
}

#[test]
fn set_json_preserves_dense_order_and_deduplicates() {
    for (input, canonical, expected) in [
        ("[]", "[]", vec![]),
        ("[9,2,5]", "[9,2,5]", vec![9, 2, 5]),
        ("[9,2,9,5,2]", "[9,2,5]", vec![9, 2, 5]),
    ] {
        let set: SparseSet<u32> = serde_json::from_str(input).unwrap();
        assert_eq!(set.as_slice(), expected);
        assert_eq!(serde_json::to_string(&set).unwrap(), canonical);
    }
}

#[test]
fn map_values_can_borrow_from_the_input() {
    let json = String::from(r#"{"3":"three","1":"one"}"#);
    let map: SparseMap<u32, &str> = serde_json::from_str(&json).unwrap();
    assert_eq!(map[3], "three");
    assert_eq!(map[1], "one");
}

#[test]
fn invalid_json_is_rejected() {
    for input in [
        "[]",
        r#"{"256":"a"}"#,
        r#"{"1":"a","2":17}"#,
        r#"{"1":"a","2":"#,
    ] {
        assert!(
            serde_json::from_str::<SparseMap<u8, char>>(input).is_err(),
            "{input}"
        );
    }
    for input in ["{}", "[256]", "[1,-1]", "[1,\"2\"]", "[1,"] {
        assert!(
            serde_json::from_str::<SparseSet<u8>>(input).is_err(),
            "{input}"
        );
    }
}

#[test]
fn errors_name_the_expected_input() {
    let err = serde_json::from_str::<SparseMap<u8, char>>("[]").unwrap_err();
    assert!(err.to_string().contains("expected a map"), "{err}");
    let err = serde_json::from_str::<SparseSet<u8>>("{}").unwrap_err();
    assert!(err.to_string().contains("expected a sequence"), "{err}");
}
