//! `serde` support: a map serializes as a map, a set as a sequence, both in
//! dense order.
//!
//! Deserialization inserts entries in order, so a repeated key keeps its last
//! value. Sparse storage grows to the largest key, so bound key values when
//! deserializing untrusted input.

use core::fmt;
use core::marker::PhantomData;

use ::serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use ::serde::ser::{Serialize, Serializer};

use crate::{Key, SparseMap, SparseSet};

/// Entries preallocated from an untrusted size hint.
const MAX_PREALLOCATED: usize = 1 << 12;

impl<K: Key + Serialize, V: Serialize> Serialize for SparseMap<K, V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.iter())
    }
}

impl<K: Key + Serialize> Serialize for SparseSet<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

impl<'de, K: Key + Deserialize<'de>, V: Deserialize<'de>> Deserialize<'de> for SparseMap<K, V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor<K, V>(PhantomData<(K, V)>);

        impl<'de, K: Key + Deserialize<'de>, V: Deserialize<'de>> Visitor<'de> for MapVisitor<K, V> {
            type Value = SparseMap<K, V>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let hint = access.size_hint().unwrap_or(0);
                let mut map = SparseMap::with_capacity(hint.min(MAX_PREALLOCATED));
                while let Some((key, value)) = access.next_entry()? {
                    map.insert(key, value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_map(MapVisitor(PhantomData))
    }
}

impl<'de, K: Key + Deserialize<'de>> Deserialize<'de> for SparseSet<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SetVisitor<K>(PhantomData<K>);

        impl<'de, K: Key + Deserialize<'de>> Visitor<'de> for SetVisitor<K> {
            type Value = SparseSet<K>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a sequence")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let hint = access.size_hint().unwrap_or(0);
                let mut set = SparseSet::with_capacity(hint.min(MAX_PREALLOCATED));
                while let Some(key) = access.next_element()? {
                    set.insert(key);
                }
                Ok(set)
            }
        }

        deserializer.deserialize_seq(SetVisitor(PhantomData))
    }
}
