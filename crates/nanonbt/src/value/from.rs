//! A [`Value`] as a serde deserializer.

use alloc::{borrow::Cow, collections::btree_map, vec::Vec};

use serde::de::{
    self, DeserializeSeed, Deserializer, EnumAccess, Expected, IntoDeserializer, MapAccess,
    SeqAccess, Unexpected, VariantAccess, Visitor,
    value::{BorrowedStrDeserializer, BytesDeserializer},
};

use super::Value;
use crate::{
    arrays::{BYTE_ARRAY_TOKEN, INT_ARRAY_TOKEN, LONG_ARRAY_TOKEN},
    error::{Error, Result},
};

impl Value {
    #[cold]
    fn invalid_type<E: de::Error>(&self, expected: &dyn Expected) -> E {
        E::invalid_type(self.unexpected(), expected)
    }

    #[cold]
    fn unexpected(&self) -> Unexpected<'_> {
        match self {
            Self::Byte(v) => Unexpected::Signed(i64::from(*v)),
            Self::Short(v) => Unexpected::Signed(i64::from(*v)),
            Self::Int(v) => Unexpected::Signed(i64::from(*v)),
            Self::Long(v) => Unexpected::Signed(*v),
            Self::Float(v) => Unexpected::Float(f64::from(*v)),
            Self::Double(v) => Unexpected::Float(*v),
            Self::String(v) => Unexpected::Str(v),
            Self::ByteArray(_) | Self::IntArray(_) | Self::LongArray(_) | Self::List(_) => {
                Unexpected::Seq
            }
            Self::Compound(_) => Unexpected::Map,
        }
    }
}

fn visit_list<'de, V: Visitor<'de>>(list: &'de [Value], visitor: V) -> Result<V::Value> {
    let mut elements = Elements(list.iter());
    let seq = visitor.visit_seq(&mut elements)?;
    if elements.0.len() == 0 {
        Ok(seq)
    } else {
        Err(de::Error::invalid_length(
            list.len(),
            &"fewer elements in list",
        ))
    }
}

fn visit_compound<'de, V: Visitor<'de>>(
    compound: &'de alloc::collections::BTreeMap<alloc::string::String, Value>,
    visitor: V,
) -> Result<V::Value> {
    let mut entries = Entries {
        iter: compound.iter(),
        value: None,
    };
    let map = visitor.visit_map(&mut entries)?;
    if entries.iter.len() == 0 {
        Ok(map)
    } else {
        Err(de::Error::invalid_length(
            compound.len(),
            &"fewer elements in map",
        ))
    }
}

macro_rules! number {
    ($method:ident, $visit:ident, $primitive:ty, $variant:ident) => {
        #[allow(clippy::cast_sign_loss)] // `as` semantics, as fastnbt
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            match self {
                Value::$variant(v) => visitor.$visit(*v as $primitive),
                _ => Err(self.invalid_type(&visitor)),
            }
        }
    };
}

impl<'de> Deserializer<'de> for &'de Value {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            Value::Byte(v) => visitor.visit_i8(*v),
            Value::Short(v) => visitor.visit_i16(*v),
            Value::Int(v) => visitor.visit_i32(*v),
            Value::Long(v) => visitor.visit_i64(*v),
            Value::Float(v) => visitor.visit_f32(*v),
            Value::Double(v) => visitor.visit_f64(*v),
            Value::String(v) => visitor.visit_borrowed_str(v),
            Value::ByteArray(v) => {
                visitor.visit_map(ArrayAccess::new(BYTE_ARRAY_TOKEN, v.to_be_bytes()))
            }
            Value::IntArray(v) => {
                visitor.visit_map(ArrayAccess::new(INT_ARRAY_TOKEN, v.to_be_bytes()))
            }
            Value::LongArray(v) => {
                visitor.visit_map(ArrayAccess::new(LONG_ARRAY_TOKEN, v.to_be_bytes()))
            }
            Value::List(v) => visit_list(v, visitor),
            Value::Compound(v) => visit_compound(v, visitor),
        }
    }

    number!(deserialize_i8, visit_i8, i8, Byte);
    number!(deserialize_i16, visit_i16, i16, Short);
    number!(deserialize_i32, visit_i32, i32, Int);
    number!(deserialize_i64, visit_i64, i64, Long);
    number!(deserialize_u8, visit_u8, u8, Byte);
    number!(deserialize_u16, visit_u16, u16, Short);
    number!(deserialize_u32, visit_u32, u32, Int);
    number!(deserialize_u64, visit_u64, u64, Long);
    number!(deserialize_f32, visit_f32, f32, Float);
    number!(deserialize_f64, visit_f64, f64, Double);

    fn deserialize_i128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i128(wide(self)?)
    }

    #[allow(clippy::cast_sign_loss)] // the same bits
    fn deserialize_u128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u128(wide(self)? as u128)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_some(self)
    }

    /// A string names a unit variant; a single-entry compound, any variant.
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let (variant, value) = match self {
            Value::Compound(compound) => {
                let mut entries = compound.iter();
                match (entries.next(), entries.next()) {
                    (Some((variant, value)), None) => (variant, Some(value)),
                    _ => {
                        return Err(de::Error::invalid_value(
                            Unexpected::Map,
                            &"map with a single key",
                        ));
                    }
                }
            }
            Value::String(variant) => (variant, None),
            other => return Err(other.invalid_type(&"string or map")),
        };
        visitor.visit_enum(Variant { variant, value })
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            Value::Byte(v) => visitor.visit_bool(*v != 0),
            Value::Short(v) => visitor.visit_bool(*v != 0),
            Value::Int(v) => visitor.visit_bool(*v != 0),
            Value::Long(v) => visitor.visit_bool(*v != 0),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    /// A character code, or the first character of a string.
    #[allow(clippy::cast_sign_loss)] // `as` semantics, as fastnbt
    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            Value::Int(v) => char::from_u32(*v as u32).map_or_else(
                || {
                    Err(de::Error::invalid_value(
                        self.unexpected(),
                        &"invalid character code",
                    ))
                },
                |c| visitor.visit_char(c),
            ),
            Value::String(v) => v.chars().next().map_or_else(
                || {
                    Err(de::Error::invalid_value(
                        self.unexpected(),
                        &"string contains no character",
                    ))
                },
                |c| visitor.visit_char(c),
            ),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            Value::String(v) => visitor.visit_borrowed_str(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_byte_buf(visitor)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            Value::String(v) => visitor.visit_borrowed_str(v),
            Value::List(v) => visit_list(v, visitor),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            Value::List(v) => visit_list(v, visitor),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_any(visitor)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        match self {
            Value::List(v) => visit_list(v, visitor),
            Value::Compound(v) => visit_compound(v, visitor),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_string(visitor)
    }

    /// Any value fills a unit.
    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_unit()
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_unit()
    }
}

/// A UUID-style int array of length 4, most significant int first.
fn wide(value: &Value) -> Result<i128> {
    match value {
        Value::IntArray(ints) => {
            let ints: [i32; 4] = (**ints)
                .try_into()
                .map_err(|_| Error::expected_int_array())?;
            let mut bytes = [0; 16];
            for (chunk, int) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(ints) {
                *chunk = int.to_be_bytes();
            }
            Ok(i128::from_be_bytes(bytes))
        }
        _ => Err(Error::expected_int_array()),
    }
}

/// An array, as the single-entry map its wrapper type expects.
///
/// fastnbt's version never runs out of entries; this one has exactly one.
struct ArrayAccess {
    token: Option<&'static str>,
    data: Vec<u8>,
}

impl ArrayAccess {
    /// Holds the payload, so that the kind cannot be mismatched later.
    const fn new(token: &'static str, data: Vec<u8>) -> Self {
        Self {
            token: Some(token),
            data,
        }
    }
}

impl<'de> MapAccess<'de> for ArrayAccess {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        self.token.take().map_or(Ok(None), |token| {
            seed.deserialize(BorrowedStrDeserializer::new(token))
                .map(Some)
        })
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        seed.deserialize(BytesDeserializer::new(&self.data))
    }
}

struct Elements<'de>(core::slice::Iter<'de, Value>);

impl<'de> SeqAccess<'de> for Elements<'de> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
        self.0
            .next()
            .map_or(Ok(None), |value| seed.deserialize(value).map(Some))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.0.len())
    }
}

struct Entries<'de> {
    iter: btree_map::Iter<'de, alloc::string::String, Value>,
    value: Option<&'de Value>,
}

impl<'de> MapAccess<'de> for Entries<'de> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(KeyDeserializer(key)).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        self.value.take().map_or_else(
            || Err(de::Error::custom("value is missing")),
            |value| seed.deserialize(value),
        )
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

/// A compound name, which may also stand for an integer.
struct KeyDeserializer<'de>(&'de str);

macro_rules! integer_key {
    ($method:ident => $visit:ident) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            match self.0.parse() {
                Ok(integer) => visitor.$visit(integer),
                Err(_) => visitor.visit_borrowed_str(self.0),
            }
        }
    };
}

impl<'de> Deserializer<'de> for KeyDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_borrowed_str(self.0)
    }

    integer_key!(deserialize_i8 => visit_i8);
    integer_key!(deserialize_i16 => visit_i16);
    integer_key!(deserialize_i32 => visit_i32);
    integer_key!(deserialize_i64 => visit_i64);
    integer_key!(deserialize_i128 => visit_i128);
    integer_key!(deserialize_u8 => visit_u8);
    integer_key!(deserialize_u16 => visit_u16);
    integer_key!(deserialize_u32 => visit_u32);
    integer_key!(deserialize_u64 => visit_u64);
    integer_key!(deserialize_u128 => visit_u128);

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let key: Cow<'de, str> = Cow::Borrowed(self.0);
        IntoDeserializer::<Error>::into_deserializer(key).deserialize_enum(name, variants, visitor)
    }

    serde::forward_to_deserialize_any! {
        bool f32 f64 char str string bytes byte_buf unit unit_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

/// An enum variant's name, and its content if it has any.
struct Variant<'de> {
    variant: &'de str,
    value: Option<&'de Value>,
}

impl<'de> EnumAccess<'de> for Variant<'de> {
    type Error = Error;
    type Variant = Content<'de>;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Content<'de>)> {
        let name = IntoDeserializer::<Error>::into_deserializer(self.variant);
        seed.deserialize(name).map(|v| (v, Content(self.value)))
    }
}

struct Content<'de>(Option<&'de Value>);

impl<'de> VariantAccess<'de> for Content<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        self.0.map_or_else(
            || {
                Err(de::Error::invalid_type(
                    Unexpected::UnitVariant,
                    &"newtype variant",
                ))
            },
            |value| seed.deserialize(value),
        )
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
        match self.0 {
            Some(Value::List(v)) if v.is_empty() => visitor.visit_unit(),
            Some(Value::List(v)) => visit_list(v, visitor),
            Some(other) => Err(other.invalid_type(&"tuple variant")),
            None => Err(de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"tuple variant",
            )),
        }
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        match self.0 {
            Some(Value::Compound(v)) => visit_compound(v, visitor),
            Some(other) => Err(other.invalid_type(&"struct variant")),
            None => Err(de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"struct variant",
            )),
        }
    }
}
