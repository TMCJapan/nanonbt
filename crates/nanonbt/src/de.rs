//! A serde deserializer reading NBT.

use alloc::borrow::Cow;

use serde::de::{self, Visitor};

use nanocesu8::Cesu8;

use crate::{
    DeOpts,
    error::{Error, Result},
    serde_arrays::ArrayKind,
    tag::{
        TAG_BYTE, TAG_BYTE_ARRAY, TAG_COMPOUND, TAG_DOUBLE, TAG_END, TAG_FLOAT, TAG_INT,
        TAG_INT_ARRAY, TAG_LIST, TAG_LONG, TAG_LONG_ARRAY, TAG_MAX, TAG_SHORT, TAG_STRING,
    },
};

/// Reads a document from a byte slice, borrowing from it where possible.
pub struct Deserializer<'de> {
    input: &'de [u8],
    seen_root: bool,
    /// How many lists and compounds are open, which bounds the recursion.
    depth: usize,
    opts: DeOpts,
}

impl<'de> Deserializer<'de> {
    pub const fn from_bytes(input: &'de [u8], opts: DeOpts) -> Self {
        Self {
            input,
            seen_root: false,
            depth: 0,
            opts,
        }
    }

    /// Reads a list or compound one level deeper, refusing input that nests
    /// too deeply.
    ///
    /// Reading recurses once per level, and a stack overflow is an abort no
    /// `Result` can carry, so the depth is bounded like the length is. Every
    /// descent goes through here, so the level is given back however `f`
    /// ends, and opening one without counting it is not expressible.
    fn nested<R>(&mut self, f: impl FnOnce(&mut Self) -> Result<R>) -> Result<R> {
        if self.depth >= self.opts.max_depth {
            return Err(Error::too_deep());
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    const fn take(&mut self, n: usize) -> Result<&'de [u8]> {
        if n > self.input.len() {
            return Err(Error::unexpected_eof());
        }
        let (taken, rest) = self.input.split_at(n);
        self.input = rest;
        Ok(taken)
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.take(N)?;
        let mut array = [0; N];
        array.copy_from_slice(bytes);
        Ok(array)
    }

    fn tag(&mut self) -> Result<u8> {
        let [tag] = self.take_array()?;
        if tag > TAG_MAX {
            return Err(Error::invalid_tag(tag));
        }
        Ok(tag)
    }

    /// An array's `i32` length, which must be non-negative and within bounds.
    fn array_len(&mut self) -> Result<usize> {
        let len = i32::from_be_bytes(self.take_array()?);
        let len = usize::try_from(len).map_err(|_| Error::negative_len())?;
        if len > self.opts.max_seq_len {
            return Err(Error::seq_too_long());
        }
        Ok(len)
    }

    /// `len` elements of `size` bytes each.
    fn take_elements(&mut self, len: usize, size: usize) -> Result<&'de [u8]> {
        let n = len.checked_mul(size).ok_or_else(Error::array_too_large)?;
        self.take(n)
    }

    /// Skips a string without decoding it.
    fn skip_str(&mut self) -> Result<()> {
        let len = u16::from_be_bytes(self.take_array()?);
        self.take(usize::from(len)).map(drop)
    }

    /// Skips a value by its structure alone, decoding nothing.
    ///
    /// Unlike reading a value, lengths are not bounded by `max_seq_len`, and
    /// a negative list length skips no elements.
    fn skip_value(&mut self, tag: u8) -> Result<()> {
        match tag {
            TAG_BYTE => self.take(1).map(drop),
            TAG_SHORT => self.take(2).map(drop),
            TAG_INT | TAG_FLOAT => self.take(4).map(drop),
            TAG_LONG | TAG_DOUBLE => self.take(8).map(drop),
            TAG_STRING => self.skip_str(),
            TAG_BYTE_ARRAY => self.skip_array(1),
            TAG_INT_ARRAY => self.skip_array(4),
            TAG_LONG_ARRAY => self.skip_array(8),
            TAG_COMPOUND => self.nested(|de| {
                loop {
                    let tag = de.tag()?;
                    if tag == TAG_END {
                        return Ok(());
                    }
                    de.skip_str()?;
                    de.skip_value(tag)?;
                }
            }),
            TAG_LIST => match self.tag()? {
                TAG_BYTE => self.nested(|de| de.skip_array(1)),
                TAG_SHORT => self.nested(|de| de.skip_array(2)),
                TAG_INT | TAG_FLOAT => self.nested(|de| de.skip_array(4)),
                TAG_LONG | TAG_DOUBLE => self.nested(|de| de.skip_array(8)),
                element => self.nested(|de| {
                    let len = i32::from_be_bytes(de.take_array()?);
                    for _ in 0..len {
                        de.skip_value(element)?;
                    }
                    Ok(())
                }),
            },
            // fastnbt panics here, skipping a list of End with elements.
            _ => Err(Error::list_of_end()),
        }
    }

    fn skip_array(&mut self, size: usize) -> Result<()> {
        let len = i32::from_be_bytes(self.take_array()?);
        let len = usize::try_from(len).map_err(|_| Error::negative_len())?;
        self.take_elements(len, size).map(drop)
    }

    fn str(&mut self) -> Result<Cow<'de, str>> {
        let len = u16::from_be_bytes(self.take_array()?);
        let bytes = self.take(usize::from(len))?;
        Cesu8::new(bytes)
            .map(Cesu8::decode)
            .map_err(|_| Error::nonunicode_string())
    }
}

impl<'de> de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_map(visitor)
    }

    /// Everything starts at the root compound, whatever the visitor wants.
    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if !self.seen_root {
            if self.tag()? != TAG_COMPOUND {
                return Err(Error::no_root_compound());
            }
            if self.opts.expect_compound_names {
                self.skip_str()?;
            }
            self.seen_root = true;
        }
        self.nested(|de| visitor.visit_map(Compound { de, tag: TAG_END }))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct struct enum identifier ignored_any
    }
}

/// The entries of a compound, up to its End tag.
struct Compound<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    tag: u8,
}

impl<'de> de::MapAccess<'de> for Compound<'_, 'de> {
    type Error = Error;

    fn next_key_seed<K: de::DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        self.tag = self.de.tag()?;
        if self.tag == TAG_END {
            return Ok(None);
        }
        seed.deserialize(Key { de: self.de }).map(Some)
    }

    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        seed.deserialize(Payload::new(self.de, self.tag))
    }
}

/// A compound entry's name.
struct Key<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'de> de::Deserializer<'de> for Key<'_, 'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.de.str()? {
            Cow::Borrowed(name) => visitor.visit_borrowed_str(name),
            Cow::Owned(name) => visitor.visit_str(&name),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

/// A value whose tag has already been read.
struct Payload<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    tag: u8,
}

impl<'a, 'de> Payload<'a, 'de> {
    const fn new(de: &'a mut Deserializer<'de>, tag: u8) -> Self {
        Self { de, tag }
    }
}

impl<'de> de::Deserializer<'de> for Payload<'_, 'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let de = self.de;
        match self.tag {
            TAG_BYTE => visitor.visit_i8(i8::from_be_bytes(de.take_array()?)),
            TAG_SHORT => visitor.visit_i16(i16::from_be_bytes(de.take_array()?)),
            TAG_INT => visitor.visit_i32(i32::from_be_bytes(de.take_array()?)),
            TAG_LONG => visitor.visit_i64(i64::from_be_bytes(de.take_array()?)),
            TAG_FLOAT => visitor.visit_f32(f32::from_be_bytes(de.take_array()?)),
            TAG_DOUBLE => visitor.visit_f64(f64::from_be_bytes(de.take_array()?)),
            TAG_STRING => match de.str()? {
                Cow::Borrowed(text) => visitor.visit_borrowed_str(text),
                Cow::Owned(text) => visitor.visit_str(&text),
            },
            TAG_LIST => {
                let tag = de.tag()?;
                // Sign-extended, so a negative length is too long below.
                #[allow(clippy::cast_sign_loss)] // fastnbt reads it this way
                let remaining = i32::from_be_bytes(de.take_array()?) as usize;
                // Old chunks store empty lists as lists of End; longer ones
                // would be a cheap way to allocate without bound.
                if tag == TAG_END && remaining != 0 {
                    return Err(Error::list_of_end());
                }
                if remaining > de.opts.max_seq_len {
                    return Err(Error::seq_too_long());
                }
                de.nested(|de| visitor.visit_seq(List { de, tag, remaining }))
            }
            TAG_COMPOUND => de.nested(|de| visitor.visit_map(Compound { de, tag: TAG_END })),
            TAG_BYTE_ARRAY | TAG_INT_ARRAY | TAG_LONG_ARRAY => {
                let size = match self.tag {
                    TAG_INT_ARRAY => 4,
                    TAG_LONG_ARRAY => 8,
                    _ => 1,
                };
                let len = de.array_len()?;
                visitor.visit_borrowed_bytes(de.take_elements(len, size)?)
            }
            _ => Err(Error::expected_value()),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_some(self)
    }

    /// Any value fills a unit, which only checks that it is there.
    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.de.skip_value(self.tag)?;
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_unit(visitor)
    }

    /// A `#[serde(with = ...)]` array module hands its visitor the array's
    /// payload; every other newtype struct deserializes as its inner value.
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        let Some(kind) = ArrayKind::from_token(name) else {
            return visitor.visit_newtype_struct(self);
        };
        if self.tag != kind.tag() {
            return Err(Error::invalid_tag(self.tag));
        }
        let len = self.de.array_len()?;
        visitor.visit_borrowed_bytes(self.de.take_elements(len, kind.size())?)
    }

    /// Only unit variants, named by the value.
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_enum(UnitVariant {
            de: self.de,
            tag: self.tag,
        })
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_any(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_seq(visitor)
    }

    /// The raw payload of strings, arrays and integer lists.
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let de = self.de;
        let bytes = match self.tag {
            TAG_STRING => {
                let len = u16::from_be_bytes(de.take_array()?);
                de.take(usize::from(len))?
            }
            TAG_LIST => {
                let tag = de.tag()?;
                let len = de.array_len()?;
                let size = match tag {
                    TAG_BYTE => 1,
                    TAG_SHORT => 2,
                    TAG_INT => 4,
                    TAG_LONG => 8,
                    _ => return Err(Error::not_bytes()),
                };
                de.take_elements(len, size)?
            }
            TAG_BYTE_ARRAY => {
                let len = de.array_len()?;
                de.take_elements(len, 1)?
            }
            TAG_INT_ARRAY => {
                let len = de.array_len()?;
                de.take_elements(len, 4)?
            }
            TAG_LONG_ARRAY => {
                let len = de.array_len()?;
                de.take_elements(len, 8)?
            }
            _ => return Err(Error::not_bytes()),
        };
        visitor.visit_borrowed_bytes(bytes)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_bytes(visitor)
    }

    /// A UUID-style int array of length 4, most significant int first.
    fn deserialize_i128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i128(self.int_array_128()?)
    }

    #[allow(clippy::cast_sign_loss)] // the same bits, as fastnbt reads them
    fn deserialize_u128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u128(self.int_array_128()? as u128)
    }

    /// Any integer is a bool: zero is false.
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let de = &mut *self.de;
        match self.tag {
            TAG_BYTE => visitor.visit_bool(de.take_array::<1>()? != [0]),
            TAG_SHORT => visitor.visit_bool(de.take_array::<2>()? != [0; 2]),
            TAG_INT => visitor.visit_bool(de.take_array::<4>()? != [0; 4]),
            TAG_LONG => visitor.visit_bool(de.take_array::<8>()? != [0; 8]),
            _ => self.deserialize_any(visitor),
        }
    }

    serde::forward_to_deserialize_any! {
        i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        tuple map struct identifier
    }
}

/// An enum variant named by a value; only unit variants can be read.
struct UnitVariant<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    tag: u8,
}

impl<'de> de::EnumAccess<'de> for UnitVariant<'_, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: de::DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self)> {
        let variant = seed.deserialize(Payload::new(self.de, self.tag))?;
        Ok((variant, self))
    }
}

impl<'de> de::VariantAccess<'de> for UnitVariant<'_, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(self, _seed: T) -> Result<T::Value> {
        Err(de::Error::invalid_type(
            de::Unexpected::UnitVariant,
            &"newtype variant",
        ))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value> {
        Err(de::Error::invalid_type(
            de::Unexpected::TupleVariant,
            &"tuple variant",
        ))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value> {
        Err(de::Error::invalid_type(
            de::Unexpected::StructVariant,
            &"struct variant",
        ))
    }
}

impl Payload<'_, '_> {
    fn int_array_128(self) -> Result<i128> {
        if self.tag != TAG_INT_ARRAY {
            return Err(Error::expected_int_array());
        }
        let len = self.de.array_len()?;
        let bytes = self.de.take_elements(len, 4)?;
        let bytes = bytes.try_into().map_err(|_| Error::expected_int_array())?;
        Ok(i128::from_be_bytes(bytes))
    }
}

/// The elements of a list, which all share one tag.
struct List<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    tag: u8,
    remaining: usize,
}

impl<'de> de::SeqAccess<'de> for List<'_, 'de> {
    type Error = Error;

    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        seed.deserialize(Payload::new(self.de, self.tag)).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}
