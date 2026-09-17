//! The serde implementations of [`Value`], kept behind the `serde` feature.

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;

use serde::{
    de::{self, Deserialize, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor},
    ser::{self, Impossible, Serialize, Serializer},
};

use crate::{
    arrays::{
        BYTE_ARRAY_TOKEN, ByteArray, ByteBuf, INT_ARRAY_TOKEN, IntArray, LONG_ARRAY_TOKEN,
        LongArray,
    },
    error::{Error, Result},
    ser::refuse,
    value::Value,
};

/// Interprets a [`Value`] as a `T`, through the serde implementations.
pub(crate) fn from_value_serde<'de, T: Deserialize<'de>>(value: &'de Value) -> Result<T> {
    T::deserialize(value)
}

/// Converts any serializable `value` into a [`Value`].
pub(crate) fn to_value_serde<T: Serialize + ?Sized>(value: &T) -> Result<Value> {
    value.serialize(ValueSerializer)
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error> {
        match self {
            Self::Byte(v) => serializer.serialize_i8(*v),
            Self::Short(v) => serializer.serialize_i16(*v),
            Self::Int(v) => serializer.serialize_i32(*v),
            Self::Long(v) => serializer.serialize_i64(*v),
            Self::Float(v) => serializer.serialize_f32(*v),
            Self::Double(v) => serializer.serialize_f64(*v),
            Self::String(v) => serializer.serialize_str(v),
            Self::ByteArray(v) => v.serialize(serializer),
            Self::IntArray(v) => v.serialize(serializer),
            Self::LongArray(v) => v.serialize(serializer),
            Self::List(v) => v.serialize(serializer),
            Self::Compound(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> core::result::Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("valid NBT")
    }

    fn visit_i8<E: de::Error>(self, v: i8) -> core::result::Result<Value, E> {
        Ok(Value::Byte(v))
    }

    fn visit_i16<E: de::Error>(self, v: i16) -> core::result::Result<Value, E> {
        Ok(Value::Short(v))
    }

    fn visit_i32<E: de::Error>(self, v: i32) -> core::result::Result<Value, E> {
        Ok(Value::Int(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> core::result::Result<Value, E> {
        Ok(Value::Long(v))
    }

    fn visit_f32<E: de::Error>(self, v: f32) -> core::result::Result<Value, E> {
        Ok(Value::Float(v))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> core::result::Result<Value, E> {
        Ok(Value::Double(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> core::result::Result<Value, E> {
        Ok(Value::String(String::from(v)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> core::result::Result<Value, A::Error> {
        let mut list = Vec::new();
        while let Some(element) = seq.next_element()? {
            list.push(element);
        }
        Ok(Value::List(list))
    }

    /// A compound, or one of the array wrappers, told apart by the first key.
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> core::result::Result<Value, A::Error> {
        match map.next_key_seed(KeyClassifier)? {
            Some(Key::Compound(first)) => {
                let mut compound = BTreeMap::new();
                compound.insert(first, map.next_value()?);
                while let Some((key, value)) = map.next_entry()? {
                    compound.insert(key, value);
                }
                Ok(Value::Compound(compound))
            }
            Some(Key::ByteArray) => {
                let data = map.next_value::<ByteBuf>()?;
                Ok(Value::ByteArray(ByteArray::from_be_bytes(&data.0)))
            }
            Some(Key::IntArray) => {
                let data = map.next_value::<ByteBuf>()?;
                Ok(Value::IntArray(IntArray::from_be_bytes(&data.0)))
            }
            Some(Key::LongArray) => {
                let data = map.next_value::<ByteBuf>()?;
                Ok(Value::LongArray(LongArray::from_be_bytes(&data.0)))
            }
            None => Ok(Value::Compound(BTreeMap::new())),
        }
    }
}

enum Key {
    Compound(String),
    ByteArray,
    IntArray,
    LongArray,
}

struct KeyClassifier;

impl<'de> DeserializeSeed<'de> for KeyClassifier {
    type Value = Key;

    fn deserialize<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> core::result::Result<Key, D::Error> {
        deserializer.deserialize_str(self)
    }
}

impl Visitor<'_> for KeyClassifier {
    type Value = Key;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an nbt field string")
    }

    fn visit_str<E: de::Error>(self, s: &str) -> core::result::Result<Key, E> {
        Ok(match s {
            BYTE_ARRAY_TOKEN => Key::ByteArray,
            INT_ARRAY_TOKEN => Key::IntArray,
            LONG_ARRAY_TOKEN => Key::LongArray,
            _ => Key::Compound(String::from(s)),
        })
    }

    fn visit_string<E: de::Error>(self, s: String) -> core::result::Result<Key, E> {
        Ok(match s.as_str() {
            BYTE_ARRAY_TOKEN => Key::ByteArray,
            INT_ARRAY_TOKEN => Key::IntArray,
            LONG_ARRAY_TOKEN => Key::LongArray,
            _ => Key::Compound(s),
        })
    }
}

/// The UUID-style int array fastnbt makes of 128-bit integers.
#[allow(clippy::cast_possible_truncation)] // each int takes 32 bits
fn wide(v: u128) -> Value {
    Value::IntArray(IntArray::new(alloc::vec![
        (v >> 96) as i32,
        (v >> 64) as i32,
        (v >> 32) as i32,
        v as i32,
    ]))
}

struct ValueSerializer;

impl ser::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = SerializeList;
    type SerializeTuple = SerializeList;
    type SerializeTupleStruct = SerializeList;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeCompound;
    type SerializeStruct = SerializeCompound;
    type SerializeStructVariant = SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Value> {
        Ok(Value::Byte(i8::from(v)))
    }

    fn serialize_i8(self, v: i8) -> Result<Value> {
        Ok(Value::Byte(v))
    }

    fn serialize_i16(self, v: i16) -> Result<Value> {
        Ok(Value::Short(v))
    }

    fn serialize_i32(self, v: i32) -> Result<Value> {
        Ok(Value::Int(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Value> {
        Ok(Value::Long(v))
    }

    #[allow(clippy::cast_sign_loss)] // the same bits
    fn serialize_i128(self, v: i128) -> Result<Value> {
        Ok(wide(v as u128))
    }

    #[allow(clippy::cast_possible_wrap)] // NBT has only signed integers
    fn serialize_u8(self, v: u8) -> Result<Value> {
        Ok(Value::Byte(v as i8))
    }

    #[allow(clippy::cast_possible_wrap)] // NBT has only signed integers
    fn serialize_u16(self, v: u16) -> Result<Value> {
        Ok(Value::Short(v as i16))
    }

    #[allow(clippy::cast_possible_wrap)] // NBT has only signed integers
    fn serialize_u32(self, v: u32) -> Result<Value> {
        Ok(Value::Int(v as i32))
    }

    #[allow(clippy::cast_possible_wrap)] // NBT has only signed integers
    fn serialize_u64(self, v: u64) -> Result<Value> {
        Ok(Value::Long(v as i64))
    }

    fn serialize_u128(self, v: u128) -> Result<Value> {
        Ok(wide(v))
    }

    fn serialize_f32(self, v: f32) -> Result<Value> {
        Ok(Value::Float(v))
    }

    fn serialize_f64(self, v: f64) -> Result<Value> {
        Ok(Value::Double(v))
    }

    fn serialize_char(self, v: char) -> Result<Value> {
        Ok(Value::Int(v as i32))
    }

    fn serialize_str(self, v: &str) -> Result<Value> {
        Ok(Value::String(String::from(v)))
    }

    #[allow(clippy::cast_possible_wrap)] // NBT bytes are signed
    fn serialize_bytes(self, v: &[u8]) -> Result<Value> {
        Ok(Value::List(
            v.iter().map(|&b| Value::Byte(b as i8)).collect(),
        ))
    }

    fn serialize_none(self) -> Result<Value> {
        Err(Error::unit())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Value> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value> {
        Err(Error::unit())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value> {
        Err(Error::unit())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value> {
        value.serialize(self)
    }

    /// Only a variant named by an array token, holding the array's bytes.
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value> {
        let kind = match variant {
            BYTE_ARRAY_TOKEN => ArrayKind::Byte,
            INT_ARRAY_TOKEN => ArrayKind::Int,
            LONG_ARRAY_TOKEN => ArrayKind::Long,
            _ => return Err(Error::variant()),
        };
        value.serialize(NativeArraySerializer(kind))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<SerializeList> {
        Ok(SerializeList(Vec::new()))
    }

    fn serialize_tuple(self, len: usize) -> Result<SerializeList> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<SerializeList> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<SerializeTupleVariant> {
        Ok(SerializeTupleVariant {
            name: String::from(variant),
            list: Vec::new(),
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<SerializeCompound> {
        Ok(SerializeCompound {
            compound: BTreeMap::new(),
            key: None,
        })
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<SerializeCompound> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<SerializeStructVariant> {
        Ok(SerializeStructVariant {
            name: String::from(variant),
            compound: BTreeMap::new(),
        })
    }
}

struct SerializeList(Vec<Value>);

impl ser::SerializeSeq for SerializeList {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.0.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(Value::List(self.0))
    }
}

impl ser::SerializeTuple for SerializeList {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SerializeList {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value> {
        ser::SerializeSeq::end(self)
    }
}

/// `{variant: [fields]}`
struct SerializeTupleVariant {
    name: String,
    list: Vec<Value>,
}

impl ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.list.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(Value::Compound(BTreeMap::from([(
            self.name,
            Value::List(self.list),
        )])))
    }
}

/// A compound, unless its only key is an array token.
struct SerializeCompound {
    compound: BTreeMap<String, Value>,
    key: Option<String>,
}

impl ser::SerializeMap for SerializeCompound {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        self.key = Some(key.serialize(KeySerializer)?);
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        let key = self.key.take().ok_or_else(Error::value_before_key)?;
        self.compound.insert(key, value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        let mut entries = self.compound.iter();
        let (Some((key, value)), None) = (entries.next(), entries.next()) else {
            return Ok(Value::Compound(self.compound));
        };
        Ok(match key.as_str() {
            BYTE_ARRAY_TOKEN => Value::ByteArray(ByteArray::from_be_bytes(&wrapped_bytes(value)?)),
            INT_ARRAY_TOKEN => Value::IntArray(IntArray::from_be_bytes(&wrapped_bytes(value)?)),
            LONG_ARRAY_TOKEN => Value::LongArray(LongArray::from_be_bytes(&wrapped_bytes(value)?)),
            _ => Value::Compound(self.compound),
        })
    }
}

/// The payload under an array token: a list of numbers, cast to bytes.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // `as` semantics, as fastnbt
fn wrapped_bytes(value: &Value) -> Result<Vec<u8>> {
    let Value::List(list) = value else {
        return Err(Error::array_not_bytes());
    };
    list.iter()
        .map(|element| {
            element
                .as_i64()
                .map(|n| n as u8)
                .ok_or_else(Error::array_not_bytes)
        })
        .collect()
}

impl ser::SerializeStruct for SerializeCompound {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        ser::SerializeMap::serialize_entry(self, key, value)
    }

    fn end(self) -> Result<Value> {
        ser::SerializeMap::end(self)
    }
}

/// `{variant: {fields}}`
struct SerializeStructVariant {
    name: String,
    compound: BTreeMap<String, Value>,
}

impl ser::SerializeStructVariant for SerializeStructVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        self.compound
            .insert(String::from(key), value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(Value::Compound(BTreeMap::from([(
            self.name,
            Value::Compound(self.compound),
        )])))
    }
}

/// Turns a map key into a compound name.
struct KeySerializer;

impl ser::Serializer for KeySerializer {
    type Ok = String;
    type Error = Error;
    type SerializeSeq = Impossible<String, Error>;
    type SerializeTuple = Impossible<String, Error>;
    type SerializeTupleStruct = Impossible<String, Error>;
    type SerializeTupleVariant = Impossible<String, Error>;
    type SerializeMap = Impossible<String, Error>;
    type SerializeStruct = Impossible<String, Error>;
    type SerializeStructVariant = Impossible<String, Error>;

    fn serialize_i8(self, v: i8) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_i16(self, v: i16) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_i32(self, v: i32) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_i64(self, v: i64) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_u8(self, v: u8) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_u16(self, v: u16) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_u32(self, v: u32) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_u64(self, v: u64) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_char(self, v: char) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<String> {
        Ok(String::from(v))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<String> {
        Ok(String::from(variant))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<String> {
        value.serialize(self)
    }

    refuse! { key_not_string:
        serialize_bool(bool) -> String;
        serialize_i128(i128) -> String;
        serialize_u128(u128) -> String;
        serialize_f32(f32) -> String;
        serialize_f64(f64) -> String;
        serialize_bytes(&[u8]) -> String;
        serialize_none() -> String;
        serialize_unit() -> String;
        serialize_unit_struct(&'static str) -> String;
        serialize_seq(Option<usize>) -> Self::SerializeSeq;
        serialize_tuple(usize) -> Self::SerializeTuple;
        serialize_tuple_struct(&'static str, usize) -> Self::SerializeTupleStruct;
        serialize_tuple_variant(&'static str, u32, &'static str, usize) -> Self::SerializeTupleVariant;
        serialize_map(Option<usize>) -> Self::SerializeMap;
        serialize_struct(&'static str, usize) -> Self::SerializeStruct;
        serialize_struct_variant(&'static str, u32, &'static str, usize) -> Self::SerializeStructVariant;
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<String> {
        Err(Error::key_not_string())
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<String> {
        Err(Error::key_not_string())
    }
}

#[derive(Clone, Copy)]
enum ArrayKind {
    Byte,
    Int,
    Long,
}

/// An array from raw bytes in *native* byte order, as fastnbt reads them.
struct NativeArraySerializer(ArrayKind);

impl ser::Serializer for NativeArraySerializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = Impossible<Value, Error>;
    type SerializeTuple = Impossible<Value, Error>;
    type SerializeTupleStruct = Impossible<Value, Error>;
    type SerializeTupleVariant = Impossible<Value, Error>;
    type SerializeMap = Impossible<Value, Error>;
    type SerializeStruct = Impossible<Value, Error>;
    type SerializeStructVariant = Impossible<Value, Error>;

    #[allow(clippy::cast_possible_wrap)] // NBT bytes are signed
    fn serialize_bytes(self, v: &[u8]) -> Result<Value> {
        Ok(match self.0 {
            ArrayKind::Byte => {
                Value::ByteArray(ByteArray::new(v.iter().map(|&b| b as i8).collect()))
            }
            ArrayKind::Int => Value::IntArray(IntArray::new(
                v.as_chunks()
                    .0
                    .iter()
                    .map(|c| i32::from_ne_bytes(*c))
                    .collect(),
            )),
            ArrayKind::Long => Value::LongArray(LongArray::new(
                v.as_chunks()
                    .0
                    .iter()
                    .map(|c| i64::from_ne_bytes(*c))
                    .collect(),
            )),
        })
    }

    // No `serialize_i128`/`serialize_u128`: fastnbt leaves those to serde's
    // default too, so both refuse them with the same message.
    refuse! { array_not_bytes:
        serialize_bool(bool) -> Value;
        serialize_i8(i8) -> Value;
        serialize_i16(i16) -> Value;
        serialize_i32(i32) -> Value;
        serialize_i64(i64) -> Value;
        serialize_u8(u8) -> Value;
        serialize_u16(u16) -> Value;
        serialize_u32(u32) -> Value;
        serialize_u64(u64) -> Value;
        serialize_f32(f32) -> Value;
        serialize_f64(f64) -> Value;
        serialize_char(char) -> Value;
        serialize_str(&str) -> Value;
        serialize_none() -> Value;
        serialize_unit() -> Value;
        serialize_unit_struct(&'static str) -> Value;
        serialize_unit_variant(&'static str, u32, &'static str) -> Value;
        serialize_seq(Option<usize>) -> Self::SerializeSeq;
        serialize_tuple(usize) -> Self::SerializeTuple;
        serialize_tuple_struct(&'static str, usize) -> Self::SerializeTupleStruct;
        serialize_tuple_variant(&'static str, u32, &'static str, usize) -> Self::SerializeTupleVariant;
        serialize_map(Option<usize>) -> Self::SerializeMap;
        serialize_struct(&'static str, usize) -> Self::SerializeStruct;
        serialize_struct_variant(&'static str, u32, &'static str, usize) -> Self::SerializeStructVariant;
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<Value> {
        Err(Error::array_not_bytes())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Value> {
        Err(Error::array_not_bytes())
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Value> {
        Err(Error::array_not_bytes())
    }
}
