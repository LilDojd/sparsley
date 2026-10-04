//! Serde round trips.
#![cfg(feature = "serde")]

use sparsley::{SparseMap, SparseSet};

#[test]
fn map_round_trips_as_a_json_object() {
    let map = SparseMap::from([(3_u32, 'a'), (7, 'b')]);
    let json = serde_json::to_string(&map).unwrap();
    assert_eq!(json, r#"{"3":"a","7":"b"}"#);
    let back: SparseMap<u32, char> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, map);
}

#[test]
fn set_round_trips_as_a_json_array() {
    let set = SparseSet::from([9_u32, 2, 5]);
    let json = serde_json::to_string(&set).unwrap();
    assert_eq!(json, "[9,2,5]");
    let back: SparseSet<u32> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, set);
}

#[test]
fn repeated_keys_keep_the_last_value() {
    let map: SparseMap<u32, u8> = serde_json::from_str(r#"{"1":1,"1":2}"#).unwrap();
    assert_eq!(map.len(), 1);
    assert_eq!(map[1], 2);
}
