//! The NBT array types, which serde's data model has no place for.
//!
//! Each array passes through serde as a single-entry map whose key is a
//! reserved token and whose value is the big-endian payload as bytes. The
//! tokens are fastnbt's, so its array types and these are interchangeable.

use alloc::{string::String, vec::Vec};
use core::{
    fmt,
    ops::{Deref, DerefMut},
};

use serde::{
    de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor},
    ser::{Serialize, SerializeStruct, Serializer},
};

pub(crate) const BYTE_ARRAY_TOKEN: &str = "__fastnbt_byte_array";
pub(crate) const INT_ARRAY_TOKEN: &str = "__fastnbt_int_array";
pub(crate) const LONG_ARRAY_TOKEN: &str = "__fastnbt_long_array";

/// Serializes a byte slice with `serialize_bytes`.
struct Bytes<'a>(&'a [u8]);

impl Serialize for Bytes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.0)
    }
}

/// An owned byte buffer, accepted from the same inputs as `serde_bytes::ByteBuf`.
pub(crate) struct ByteBuf(pub(crate) Vec<u8>);

impl<'de> Deserialize<'de> for ByteBuf {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_byte_buf(ByteBufVisitor)
    }
}

struct ByteBufVisitor;

impl<'de> Visitor<'de> for ByteBufVisitor {
    type Value = ByteBuf;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("byte array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ByteBuf, A::Error> {
        let mut bytes = Vec::new();
        while let Some(b) = seq.next_element()? {
            bytes.push(b);
        }
        Ok(ByteBuf(bytes))
    }

    fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<ByteBuf, E> {
        Ok(ByteBuf(v.to_vec()))
    }

    fn visit_byte_buf<E: de::Error>(self, v: Vec<u8>) -> Result<ByteBuf, E> {
        Ok(ByteBuf(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<ByteBuf, E> {
        Ok(ByteBuf(v.as_bytes().to_vec()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<ByteBuf, E> {
        Ok(ByteBuf(v.into_bytes()))
    }
}

/// Reads the wrapper map fastnbt's array types expect.
fn deserialize_array<'de, M: MapAccess<'de>>(
    mut map: M,
    token: &'static str,
) -> Result<Vec<u8>, M::Error> {
    let key = map
        .next_key::<&str>()?
        .ok_or_else(|| de::Error::custom("expected NBT array token, but got empty map"))?;
    let data = map.next_value::<ByteBuf>()?;
    if key == token {
        Ok(data.0)
    } else {
        Err(de::Error::custom("expected NBT array token"))
    }
}

fn serialize_array<S: Serializer>(
    serializer: S,
    token: &'static str,
    payload: &[u8],
) -> Result<S::Ok, S::Error> {
    let mut wrapper = serializer.serialize_struct("Inner", 1)?;
    wrapper.serialize_field(token, &Bytes(payload))?;
    wrapper.end()
}

macro_rules! array {
    ($(#[$doc:meta])* $name:ident($element:ty, $token:ident, $expecting:literal)) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
        pub struct $name {
            data: Vec<$element>,
        }

        impl $name {
            pub const fn new(data: Vec<$element>) -> Self {
                Self { data }
            }

            pub fn into_inner(self) -> Vec<$element> {
                self.data
            }

            /// Reads big-endian elements, ignoring a trailing partial one.
            pub(crate) fn from_be_bytes(bytes: &[u8]) -> Self {
                const SIZE: usize = size_of::<$element>();
                let data = bytes
                    .chunks_exact(SIZE)
                    .map(|chunk| {
                        let mut array = [0; SIZE];
                        array.copy_from_slice(chunk);
                        <$element>::from_be_bytes(array)
                    })
                    .collect();
                Self { data }
            }

            /// The elements as big-endian bytes, as NBT stores them.
            pub(crate) fn to_be_bytes(&self) -> Vec<u8> {
                const SIZE: usize = size_of::<$element>();
                let mut bytes = Vec::with_capacity(self.data.len() * SIZE);
                for element in &self.data {
                    bytes.extend_from_slice(&element.to_be_bytes());
                }
                bytes
            }
        }

        impl Deref for $name {
            type Target = [$element];

            fn deref(&self) -> &[$element] {
                &self.data
            }
        }

        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut [$element] {
                &mut self.data
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serialize_array(serializer, $token, &self.to_be_bytes())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct ArrayVisitor;

                impl<'de> Visitor<'de> for ArrayVisitor {
                    type Value = $name;

                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str($expecting)
                    }

                    fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<$name, M::Error> {
                        let bytes = deserialize_array(map, $token)?;
                        Ok($name::from_be_bytes(&bytes))
                    }
                }

                deserializer.deserialize_map(ArrayVisitor)
            }
        }
    };
}

array! {
    /// An NBT byte array.
    ByteArray(i8, BYTE_ARRAY_TOKEN, "byte array")
}

array! {
    /// An NBT int array.
    IntArray(i32, INT_ARRAY_TOKEN, "int array")
}

array! {
    /// An NBT long array.
    LongArray(i64, LONG_ARRAY_TOKEN, "long array")
}
