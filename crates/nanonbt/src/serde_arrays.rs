//! The explicit NBT array spelling of the serde side.
//!
//! serde's data model has no NBT arrays: a `Vec<i64>` serializes as a list of
//! longs, and a document's array reads as its raw bytes. The modules here
//! are the `#[serde(with = ...)]` spellings that write and read the array
//! tags instead, one per kind:
//!
//! - [`byte_array`]: `Vec<i8>` or `Vec<u8>` as a `TAG_Byte_Array`
//! - [`int_array`]: `Vec<i32>` or `Vec<u32>` as a `TAG_Int_Array`
//! - [`long_array`]: `Vec<i64>` or `Vec<u64>` as a `TAG_Long_Array`
//!
//! The unsigned spelling shares the signed spelling's bits, as it does
//! everywhere else in the crate. A plain `Vec` field without the module keeps
//! its serde meaning and stays a list; a document's array read into one
//! without the module is refused.
//!
//! The modules talk to the serializer and deserializer of this crate through
//! a reserved newtype-struct name: the serializer sees a value that offers
//! itself as bytes and writes the array's tag, length and payload, and the
//! deserializer sees the token and hands the visitor the payload's bytes,
//! which the module decodes. Nothing of that is public; a module is the whole
//! interface.

use alloc::vec::Vec;
use core::fmt;

use serde::{
    Serialize, Serializer,
    de::{self, Visitor},
};

use crate::be::as_bytes;

/// The newtype-struct name that marks a byte array on the serde side.
const BYTE_ARRAY_TOKEN: &str = "$nanonbt::byte_array";

/// The newtype-struct name that marks an int array on the serde side.
const INT_ARRAY_TOKEN: &str = "$nanonbt::int_array";

/// The newtype-struct name that marks a long array on the serde side.
const LONG_ARRAY_TOKEN: &str = "$nanonbt::long_array";

/// The three NBT array kinds, as the serde side names them.
///
/// A `#[serde(with = ...)]` module wraps its value in a newtype struct named
/// by the kind's token; the serializer and the deserializer recognize the
/// name and write or read the array tag, length and payload.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArrayKind {
    Byte,
    Int,
    Long,
}

impl ArrayKind {
    /// The kind a `#[serde(with = ...)]` module wraps a value in, or `None`
    /// for every other newtype-struct name.
    pub(crate) fn from_token(name: &str) -> Option<Self> {
        match name {
            BYTE_ARRAY_TOKEN => Some(Self::Byte),
            INT_ARRAY_TOKEN => Some(Self::Int),
            LONG_ARRAY_TOKEN => Some(Self::Long),
            _ => None,
        }
    }

    /// The tag written before the array's payload.
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Byte => crate::tag::TAG_BYTE_ARRAY,
            Self::Int => crate::tag::TAG_INT_ARRAY,
            Self::Long => crate::tag::TAG_LONG_ARRAY,
        }
    }

    /// The bytes one element takes.
    pub(crate) const fn size(self) -> usize {
        match self {
            Self::Byte => 1,
            Self::Int => 4,
            Self::Long => 8,
        }
    }
}

/// An element spelling the array modules are built on, sealed to the scalar
/// integers.
///
/// Every byte of these is initialized, so a slice of them is a byte slice
/// even when the host keeps them little-endian; the serializer copies their
/// bytes and swaps each element to NBT's big-endian order.
#[doc(hidden)]
pub trait ArrayElement: Copy {}

impl ArrayElement for i8 {}
impl ArrayElement for u8 {}
impl ArrayElement for i32 {}
impl ArrayElement for u32 {}
impl ArrayElement for i64 {}
impl ArrayElement for u64 {}

/// A slice of elements, offered to the serializer as the bytes NBT stores.
struct ArrayBytes<'a, E> {
    elements: &'a [E],
}

impl<'a, E> ArrayBytes<'a, E> {
    const fn new(elements: &'a [E]) -> Self {
        Self { elements }
    }
}

impl<E: ArrayElement> Serialize for ArrayBytes<'_, E> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(as_bytes(self.elements))
    }
}

/// The visitor that turns an array's payload bytes into its elements.
///
/// The bytes are already big-endian, as NBT stores them; `decode` settles
/// their order and `size` refuses a payload that is not a whole number of
/// elements, which a deserializer other than this crate's could hand in.
struct Elements<E> {
    decode: fn(&[u8]) -> Vec<E>,
    expecting: &'static str,
    size: usize,
}

impl<E> Elements<E> {
    const fn new(decode: fn(&[u8]) -> Vec<E>, expecting: &'static str, size: usize) -> Self {
        Self {
            decode,
            expecting,
            size,
        }
    }

    fn bytes<B: de::Error>(&self, bytes: &[u8]) -> Result<Vec<E>, B> {
        if !bytes.len().is_multiple_of(self.size) {
            return Err(B::invalid_length(bytes.len(), &self.expecting));
        }
        Ok((self.decode)(bytes))
    }
}

impl<'de, E> Visitor<'de> for Elements<E> {
    type Value = Vec<E>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.expecting)
    }

    fn visit_borrowed_bytes<B: de::Error>(self, v: &'de [u8]) -> Result<Self::Value, B> {
        self.bytes(v)
    }

    fn visit_bytes<B: de::Error>(self, v: &[u8]) -> Result<Self::Value, B> {
        self.bytes(v)
    }

    fn visit_byte_buf<B: de::Error>(self, v: Vec<u8>) -> Result<Self::Value, B> {
        self.bytes(&v)
    }
}

/// `#[serde(with = ...)]` support for `TAG_Byte_Array` fields.
///
/// A `Vec<i8>` or `Vec<u8>` field annotated with
/// `#[serde(with = "nanonbt::serde_compat::byte_array")]` writes a byte array
/// and reads one back, where the plain `Serialize` and `Deserialize` derives
/// write and read a list. Bytes have no endianness, so nothing is converted.
///
/// ```
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct Section {
///     #[serde(with = "nanonbt::serde_compat::byte_array")]
///     light: Vec<i8>,
/// }
///
/// let section = Section {
///     light: vec![0, -1],
/// };
/// let bytes = nanonbt::serde_compat::to_bytes(&section).unwrap();
/// let back: Section = nanonbt::serde_compat::from_bytes(&bytes).unwrap();
/// assert_eq!(back, section);
/// ```
pub mod byte_array {
    use alloc::vec::Vec;

    use serde::{Deserializer, Serializer};

    use super::{ArrayBytes, BYTE_ARRAY_TOKEN, ByteArrayElement, ByteArrayValue, Elements};

    /// Serializes a `Vec<i8>` or `Vec<u8>` as a `TAG_Byte_Array`.
    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: ByteArrayValue + ?Sized,
    {
        serializer.serialize_newtype_struct(BYTE_ARRAY_TOKEN, &ArrayBytes::new(value.as_elements()))
    }

    /// Deserializes a `TAG_Byte_Array` into a `Vec<i8>` or `Vec<u8>`.
    ///
    /// Any other tag — a list of bytes included — is refused, as a derived
    /// `#[nbt(array = "byte")]` field refuses it.
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: ByteArrayValue,
    {
        let elements: Vec<T::Element> = deserializer.deserialize_newtype_struct(
            BYTE_ARRAY_TOKEN,
            Elements::new(decode, "an NBT byte array", 1),
        )?;
        Ok(T::from_elements(elements))
    }

    /// Reinterprets the payload as its elements.
    fn decode<E: ByteArrayElement>(bytes: &[u8]) -> Vec<E> {
        bytes.iter().map(|byte| E::from_be_bytes([*byte])).collect()
    }
}

/// One element spelling [`byte_array`] accepts, `i8` or `u8`.
#[doc(hidden)]
pub trait ByteArrayElement: ArrayElement {
    /// The element the byte encodes.
    #[doc(hidden)]
    fn from_be_bytes(bytes: [u8; 1]) -> Self;
}

impl ByteArrayElement for i8 {
    fn from_be_bytes(bytes: [u8; 1]) -> Self {
        Self::from_be_bytes(bytes)
    }
}

impl ByteArrayElement for u8 {
    fn from_be_bytes(bytes: [u8; 1]) -> Self {
        Self::from_be_bytes(bytes)
    }
}

/// A field type [`byte_array`] writes and reads, `Vec<i8>` or `Vec<u8>`.
#[doc(hidden)]
pub trait ByteArrayValue {
    /// The element spelling the field holds.
    type Element: ByteArrayElement;

    /// The elements to write, as the field holds them.
    #[doc(hidden)]
    fn as_elements(&self) -> &[Self::Element];

    /// The field the elements were read into.
    #[doc(hidden)]
    fn from_elements(elements: Vec<Self::Element>) -> Self;
}

impl<E: ByteArrayElement> ByteArrayValue for Vec<E> {
    type Element = E;

    fn as_elements(&self) -> &[E] {
        self
    }

    fn from_elements(elements: Self) -> Self {
        elements
    }
}

/// `#[serde(with = ...)]` support for `TAG_Int_Array` fields.
///
/// A `Vec<i32>` or `Vec<u32>` field annotated with
/// `#[serde(with = "nanonbt::serde_compat::int_array")]` writes an int array
/// and reads one back, where the plain `Serialize` and `Deserialize` derives
/// write and read a list. The unsigned spelling shares the signed spelling's
/// bits.
///
/// ```
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct Player {
///     #[serde(with = "nanonbt::serde_compat::int_array")]
///     uuid: Vec<i32>,
/// }
///
/// let player = Player {
///     uuid: vec![1, -1],
/// };
/// let bytes = nanonbt::serde_compat::to_bytes(&player).unwrap();
/// let back: Player = nanonbt::serde_compat::from_bytes(&bytes).unwrap();
/// assert_eq!(back, player);
/// ```
pub mod int_array {
    use alloc::vec::Vec;

    use serde::{Deserializer, Serializer};

    use super::{ArrayBytes, Elements, INT_ARRAY_TOKEN, IntArrayElement, IntArrayValue};

    /// Serializes a `Vec<i32>` or `Vec<u32>` as a `TAG_Int_Array`.
    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: IntArrayValue + ?Sized,
    {
        serializer.serialize_newtype_struct(INT_ARRAY_TOKEN, &ArrayBytes::new(value.as_elements()))
    }

    /// Deserializes a `TAG_Int_Array` into a `Vec<i32>` or `Vec<u32>`.
    ///
    /// Any other tag — a list of ints included — is refused, as a derived
    /// `#[nbt(array = "int")]` field refuses it.
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: IntArrayValue,
    {
        let elements: Vec<T::Element> = deserializer.deserialize_newtype_struct(
            INT_ARRAY_TOKEN,
            Elements::new(decode, "an NBT int array", 4),
        )?;
        Ok(T::from_elements(elements))
    }

    /// Decodes the four-byte big-endian elements, vectorized under `simd`.
    fn decode<E: IntArrayElement>(bytes: &[u8]) -> Vec<E> {
        #[cfg(feature = "simd")]
        {
            crate::simd::decode_be::<E, 4>(bytes, E::from_be_bytes)
        }
        #[cfg(not(feature = "simd"))]
        {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|chunk| E::from_be_bytes(*chunk))
                .collect()
        }
    }
}

/// One element spelling [`int_array`] accepts, `i32` or `u32`.
#[doc(hidden)]
pub trait IntArrayElement: ArrayElement {
    /// The element the four big-endian bytes encode.
    #[doc(hidden)]
    fn from_be_bytes(bytes: [u8; 4]) -> Self;
}

impl IntArrayElement for i32 {
    fn from_be_bytes(bytes: [u8; 4]) -> Self {
        Self::from_be_bytes(bytes)
    }
}

impl IntArrayElement for u32 {
    fn from_be_bytes(bytes: [u8; 4]) -> Self {
        Self::from_be_bytes(bytes)
    }
}

/// A field type [`int_array`] writes and reads, `Vec<i32>` or `Vec<u32>`.
#[doc(hidden)]
pub trait IntArrayValue {
    /// The element spelling the field holds.
    type Element: IntArrayElement;

    /// The elements to write, as the field holds them.
    #[doc(hidden)]
    fn as_elements(&self) -> &[Self::Element];

    /// The field the elements were read into.
    #[doc(hidden)]
    fn from_elements(elements: Vec<Self::Element>) -> Self;
}

impl<E: IntArrayElement> IntArrayValue for Vec<E> {
    type Element = E;

    fn as_elements(&self) -> &[E] {
        self
    }

    fn from_elements(elements: Self) -> Self {
        elements
    }
}

/// `#[serde(with = ...)]` support for `TAG_Long_Array` fields.
///
/// A `Vec<i64>` or `Vec<u64>` field annotated with
/// `#[serde(with = "nanonbt::serde_compat::long_array")]` writes a long array
/// and reads one back, where the plain `Serialize` and `Deserialize` derives
/// write and read a list. The unsigned spelling shares the signed spelling's
/// bits.
///
/// ```
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct BlockStates {
///     #[serde(with = "nanonbt::serde_compat::long_array")]
///     data: Vec<i64>,
/// }
///
/// let states = BlockStates {
///     data: vec![1, i64::MIN],
/// };
/// let bytes = nanonbt::serde_compat::to_bytes(&states).unwrap();
/// let back: BlockStates = nanonbt::serde_compat::from_bytes(&bytes).unwrap();
/// assert_eq!(back, states);
/// ```
pub mod long_array {
    use alloc::vec::Vec;

    use serde::{Deserializer, Serializer};

    use super::{ArrayBytes, Elements, LONG_ARRAY_TOKEN, LongArrayElement, LongArrayValue};

    /// Serializes a `Vec<i64>` or `Vec<u64>` as a `TAG_Long_Array`.
    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: LongArrayValue + ?Sized,
    {
        serializer.serialize_newtype_struct(LONG_ARRAY_TOKEN, &ArrayBytes::new(value.as_elements()))
    }

    /// Deserializes a `TAG_Long_Array` into a `Vec<i64>` or `Vec<u64>`.
    ///
    /// Any other tag — a list of longs included — is refused, as a derived
    /// `#[nbt(array = "long")]` field refuses it.
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: LongArrayValue,
    {
        let elements: Vec<T::Element> = deserializer.deserialize_newtype_struct(
            LONG_ARRAY_TOKEN,
            Elements::new(decode, "an NBT long array", 8),
        )?;
        Ok(T::from_elements(elements))
    }

    /// Decodes the eight-byte big-endian elements, vectorized under `simd`.
    fn decode<E: LongArrayElement>(bytes: &[u8]) -> Vec<E> {
        #[cfg(feature = "simd")]
        {
            crate::simd::decode_be::<E, 8>(bytes, E::from_be_bytes)
        }
        #[cfg(not(feature = "simd"))]
        {
            bytes
                .as_chunks::<8>()
                .0
                .iter()
                .map(|chunk| E::from_be_bytes(*chunk))
                .collect()
        }
    }
}

/// One element spelling [`long_array`] accepts, `i64` or `u64`.
#[doc(hidden)]
pub trait LongArrayElement: ArrayElement {
    /// The element the eight big-endian bytes encode.
    #[doc(hidden)]
    fn from_be_bytes(bytes: [u8; 8]) -> Self;
}

impl LongArrayElement for i64 {
    fn from_be_bytes(bytes: [u8; 8]) -> Self {
        Self::from_be_bytes(bytes)
    }
}

impl LongArrayElement for u64 {
    fn from_be_bytes(bytes: [u8; 8]) -> Self {
        Self::from_be_bytes(bytes)
    }
}

/// A field type [`long_array`] writes and reads, `Vec<i64>` or `Vec<u64>`.
#[doc(hidden)]
pub trait LongArrayValue {
    /// The element spelling the field holds.
    type Element: LongArrayElement;

    /// The elements to write, as the field holds them.
    #[doc(hidden)]
    fn as_elements(&self) -> &[Self::Element];

    /// The field the elements were read into.
    #[doc(hidden)]
    fn from_elements(elements: Vec<Self::Element>) -> Self;
}

impl<E: LongArrayElement> LongArrayValue for Vec<E> {
    type Element = E;

    fn as_elements(&self) -> &[E] {
        self
    }

    fn from_elements(elements: Self) -> Self {
        elements
    }
}
