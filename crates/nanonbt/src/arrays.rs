//! The NBT array types, which serde's data model has no place for.
//!
//! With the `serde` feature, each array passes through serde as a
//! single-entry map whose key is a reserved token and whose value is the
//! big-endian payload as bytes. The tokens are fastnbt's, so its array types
//! and these are interchangeable.
//!
//! [`ArrayOf`] writes and reads a slice of elements directly, in either
//! spelling of the element, without building an array type or copying the
//! elements; a derived field with `#[nbt(array = "...")]` goes through it.
//!
//! Conversions cover both spellings of an element: `ByteArray` comes from
//! `&[i8]` and `&[u8]`, `IntArray` from `&[i32]` and `&[u32]`, and
//! `LongArray` from `&[i64]` and `&[u64]`. An unsigned element shares its
//! signed element's tag and reinterprets the bits, as it does everywhere
//! else in the crate. The reverse conversions go to `Vec<T>`; a fixed array
//! is `TryFrom` and checks the length.
use crate::be::{as_i8, as_u8};

#[cfg(feature = "serde")]
use alloc::string::String;
use alloc::vec::Vec;
#[cfg(feature = "serde")]
use core::fmt;
use core::ops::{Deref, DerefMut};
#[cfg(feature = "serde")]
use serde::{
    de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor},
    ser::{Serialize, SerializeStruct, Serializer},
};

#[cfg(feature = "serde")]
pub(crate) const BYTE_ARRAY_TOKEN: &str = "__fastnbt_byte_array";
#[cfg(feature = "serde")]
pub(crate) const INT_ARRAY_TOKEN: &str = "__fastnbt_int_array";
#[cfg(feature = "serde")]
pub(crate) const LONG_ARRAY_TOKEN: &str = "__fastnbt_long_array";

/// Serializes a byte slice with `serialize_bytes`.
#[cfg(feature = "serde")]
struct Bytes<'a>(&'a [u8]);
#[cfg(feature = "serde")]
impl Serialize for Bytes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.0)
    }
}
/// An owned byte buffer, accepted from the same inputs as `serde_bytes::ByteBuf`.
#[cfg(feature = "serde")]
pub(crate) struct ByteBuf(pub(crate) Vec<u8>);
#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ByteBuf {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_byte_buf(ByteBufVisitor)
    }
}
#[cfg(feature = "serde")]
struct ByteBufVisitor;
#[cfg(feature = "serde")]
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
#[cfg(feature = "serde")]
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
#[cfg(feature = "serde")]
fn serialize_array<S: Serializer>(
    serializer: S,
    token: &'static str,
    payload: &[u8],
) -> Result<S::Ok, S::Error> {
    let mut wrapper = serializer.serialize_struct("Inner", 1)?;
    wrapper.serialize_field(token, &Bytes(payload))?;
    wrapper.end()
}
/// An NBT array type holding elements of `E`.
///
/// [`ByteArray`] holds `i8` and `u8`, [`IntArray`] `i32` and `u32`, and
/// [`LongArray`] `i64` and `u64`. An unsigned element shares its signed
/// element's tag and reinterprets the bits, as it does everywhere else in
/// the crate.
///
/// The methods work on a slice of elements directly, so a derived field with
/// `#[nbt(array = "...")]` neither builds an array type nor copies its
/// elements to write them.
///
/// ```
/// use nanonbt::{ArrayOf, LongArray, TAG_LONG_ARRAY, Writer};
///
/// let mut out = Vec::new();
/// let mut writer = Writer::new(&mut out);
/// <LongArray as ArrayOf<u64>>::write_entry(&[1, u64::MAX], "data", &mut writer).unwrap();
/// assert_eq!(out[0], TAG_LONG_ARRAY);
/// ```
pub trait ArrayOf<E>: Sized {
    /// The tag byte that precedes this array.
    const TAG: u8;
    /// Writes the payload: the `i32` length, then the elements, big-endian.
    fn write_payload<W: crate::Write>(elements: &[E], writer: &mut W) -> crate::Result<()>;
    /// Reads the payload, `len` elements, big-endian.
    fn read_payload<'de, R: crate::Read<'de>>(len: usize, reader: &mut R) -> crate::Result<Vec<E>>;
    /// Writes a compound entry: the tag, the name, then the payload.
    fn write_entry<W: crate::Write>(
        elements: &[E],
        name: &str,
        writer: &mut W,
    ) -> crate::Result<()> {
        writer.write_tag(Self::TAG)?;
        writer.write_name(name)?;
        Self::write_payload(elements, writer)
    }
    /// Reads an array, refusing a tag other than [`Self::TAG`].
    fn read<'de, R: crate::Read<'de>>(tag: u8, reader: &mut R) -> crate::Result<Vec<E>> {
        if tag != Self::TAG {
            return Err(crate::Error::invalid_tag(tag));
        }
        let len = reader.read_len()?;
        Self::read_payload(len, reader)
    }
}
/// Decodes `len` big-endian elements of `SIZE` bytes each.
fn read_be<'de, T, const SIZE: usize, R: crate::Read<'de>>(
    len: usize,
    reader: &mut R,
    decode: fn([u8; SIZE]) -> T,
) -> crate::Result<Vec<T>> {
    let n = len
        .checked_mul(SIZE)
        .ok_or_else(crate::Error::array_too_large)?;
    let bytes = reader.read_bytes(n)?;
    Ok(bytes
        .as_chunks::<SIZE>()
        .0
        .iter()
        .map(|chunk| decode(*chunk))
        .collect())
}
/// How one spelling of an element is written and read as an array's payload.
///
/// One byte elements have no endianness to settle, so a byte array's payload
/// is the slice as it is, in one write. The wider elements go through the
/// writer one at a time, which is what a list does too.
trait Elements: Copy + Sized {
    /// Writes the `i32` length, then the elements, big-endian.
    fn write<W: crate::Write>(elements: &[Self], writer: &mut W) -> crate::Result<()>;
    /// Reads `len` elements of the payload, big-endian.
    fn read<'de, R: crate::Read<'de>>(len: usize, reader: &mut R) -> crate::Result<Vec<Self>>;
}
impl Elements for i8 {
    fn write<W: crate::Write>(elements: &[Self], writer: &mut W) -> crate::Result<()> {
        writer.write_len(elements.len())?;
        writer.write_bytes(as_u8(elements))
    }
    fn read<'de, R: crate::Read<'de>>(len: usize, reader: &mut R) -> crate::Result<Vec<Self>> {
        Ok(as_i8(&reader.read_bytes(len)?).to_vec())
    }
}
impl Elements for u8 {
    fn write<W: crate::Write>(elements: &[Self], writer: &mut W) -> crate::Result<()> {
        writer.write_len(elements.len())?;
        writer.write_bytes(elements)
    }
    fn read<'de, R: crate::Read<'de>>(len: usize, reader: &mut R) -> crate::Result<Vec<Self>> {
        Ok(reader.read_bytes(len)?.into_owned())
    }
}
impl Elements for i32 {
    fn write<W: crate::Write>(elements: &[Self], writer: &mut W) -> crate::Result<()> {
        writer.write_len(elements.len())?;
        for element in elements {
            writer.write_i32(*element)?;
        }
        Ok(())
    }
    fn read<'de, R: crate::Read<'de>>(len: usize, reader: &mut R) -> crate::Result<Vec<Self>> {
        read_be(len, reader, <Self>::from_be_bytes)
    }
}
impl Elements for u32 {
    fn write<W: crate::Write>(elements: &[Self], writer: &mut W) -> crate::Result<()> {
        writer.write_len(elements.len())?;
        for element in elements {
            writer.write_i32((*element).cast_signed())?;
        }
        Ok(())
    }
    fn read<'de, R: crate::Read<'de>>(len: usize, reader: &mut R) -> crate::Result<Vec<Self>> {
        read_be(len, reader, <Self>::from_be_bytes)
    }
}
impl Elements for i64 {
    fn write<W: crate::Write>(elements: &[Self], writer: &mut W) -> crate::Result<()> {
        writer.write_len(elements.len())?;
        for element in elements {
            writer.write_i64(*element)?;
        }
        Ok(())
    }
    fn read<'de, R: crate::Read<'de>>(len: usize, reader: &mut R) -> crate::Result<Vec<Self>> {
        read_be(len, reader, <Self>::from_be_bytes)
    }
}
impl Elements for u64 {
    fn write<W: crate::Write>(elements: &[Self], writer: &mut W) -> crate::Result<()> {
        writer.write_len(elements.len())?;
        for element in elements {
            writer.write_i64((*element).cast_signed())?;
        }
        Ok(())
    }
    fn read<'de, R: crate::Read<'de>>(len: usize, reader: &mut R) -> crate::Result<Vec<Self>> {
        read_be(len, reader, <Self>::from_be_bytes)
    }
}
/// An NBT byte array.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ByteArray {
    data: Vec<i8>,
}
impl ByteArray {
    pub const fn new(data: Vec<i8>) -> Self {
        Self { data }
    }
    pub fn into_inner(self) -> Vec<i8> {
        self.data
    }
    /// Reads big-endian elements, ignoring a trailing partial one.
    #[cfg(feature = "serde")]
    pub(crate) fn from_be_bytes(bytes: &[u8]) -> Self {
        const SIZE: usize = size_of::<i8>();
        let data = bytes
            .as_chunks::<SIZE>()
            .0
            .iter()
            .map(|chunk| <i8>::from_be_bytes(*chunk))
            .collect();
        Self { data }
    }
    /// The elements as big-endian bytes, as NBT stores them.
    #[allow(dead_code)]
    pub(crate) fn to_be_bytes(&self) -> Vec<u8> {
        const SIZE: usize = size_of::<i8>();
        let mut bytes = Vec::with_capacity(self.data.len() * SIZE);
        for element in &self.data {
            bytes.extend_from_slice(&element.to_be_bytes());
        }
        bytes
    }
}
impl Deref for ByteArray {
    type Target = [i8];
    fn deref(&self) -> &[i8] {
        &self.data
    }
}
impl DerefMut for ByteArray {
    fn deref_mut(&mut self) -> &mut [i8] {
        &mut self.data
    }
}
impl crate::ToNBT for ByteArray {
    fn tag(&self) -> u8 {
        crate::tag::TAG_BYTE_ARRAY
    }
    fn write<W: crate::Write>(&self, writer: &mut W) -> crate::Result<()> {
        <Self as crate::ArrayOf<i8>>::write_payload(&self.data, writer)
    }
}
impl<'de> crate::FromNBT<'de> for ByteArray {
    fn read<R: crate::Read<'de>>(tag: u8, reader: &mut R) -> crate::Result<Self> {
        Ok(Self::new(<Self as crate::ArrayOf<i8>>::read(tag, reader)?))
    }
}
impl crate::ArrayOf<i8> for ByteArray {
    const TAG: u8 = crate::tag::TAG_BYTE_ARRAY;
    fn write_payload<W: crate::Write>(elements: &[i8], writer: &mut W) -> crate::Result<()> {
        <i8 as Elements>::write(elements, writer)
    }
    fn read_payload<'de, R: crate::Read<'de>>(
        len: usize,
        reader: &mut R,
    ) -> crate::Result<Vec<i8>> {
        <i8 as Elements>::read(len, reader)
    }
}
/// The unsigned spelling of the element, sharing the same bits.
impl crate::ArrayOf<u8> for ByteArray {
    const TAG: u8 = crate::tag::TAG_BYTE_ARRAY;
    fn write_payload<W: crate::Write>(elements: &[u8], writer: &mut W) -> crate::Result<()> {
        <u8 as Elements>::write(elements, writer)
    }
    fn read_payload<'de, R: crate::Read<'de>>(
        len: usize,
        reader: &mut R,
    ) -> crate::Result<Vec<u8>> {
        <u8 as Elements>::read(len, reader)
    }
}
impl From<&[i8]> for ByteArray {
    fn from(items: &[i8]) -> Self {
        Self::new(items.to_vec())
    }
}
/// The unsigned spelling of the element, sharing the same bits.
impl From<&[u8]> for ByteArray {
    fn from(items: &[u8]) -> Self {
        Self::new(items.iter().map(|&item| item.cast_signed()).collect())
    }
}
impl From<ByteArray> for Vec<i8> {
    fn from(array: ByteArray) -> Self {
        array.into_inner()
    }
}
/// The unsigned spelling of the element, sharing the same bits.
impl From<ByteArray> for Vec<u8> {
    fn from(array: ByteArray) -> Self {
        array
            .into_inner()
            .into_iter()
            .map(<i8>::cast_unsigned)
            .collect()
    }
}
impl<const N: usize> TryFrom<ByteArray> for [i8; N] {
    type Error = crate::Error;
    fn try_from(array: ByteArray) -> crate::Result<Self> {
        array
            .into_inner()
            .try_into()
            .map_err(|_| crate::Error::wrong_len())
    }
}
impl<const N: usize> TryFrom<ByteArray> for [u8; N] {
    type Error = crate::Error;
    fn try_from(array: ByteArray) -> crate::Result<Self> {
        Vec::<u8>::from(array)
            .try_into()
            .map_err(|_| crate::Error::wrong_len())
    }
}
#[cfg(feature = "serde")]
impl Serialize for ByteArray {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_array(serializer, BYTE_ARRAY_TOKEN, &self.to_be_bytes())
    }
}
#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ByteArray {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ArrayVisitor;
        impl<'de> Visitor<'de> for ArrayVisitor {
            type Value = ByteArray;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("byte array")
            }
            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<ByteArray, M::Error> {
                let bytes = deserialize_array(map, BYTE_ARRAY_TOKEN)?;
                Ok(ByteArray::from_be_bytes(&bytes))
            }
        }
        deserializer.deserialize_map(ArrayVisitor)
    }
}
/// An NBT int array.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct IntArray {
    data: Vec<i32>,
}
impl IntArray {
    pub const fn new(data: Vec<i32>) -> Self {
        Self { data }
    }
    pub fn into_inner(self) -> Vec<i32> {
        self.data
    }
    /// Reads big-endian elements, ignoring a trailing partial one.
    #[cfg(feature = "serde")]
    pub(crate) fn from_be_bytes(bytes: &[u8]) -> Self {
        const SIZE: usize = size_of::<i32>();
        let data = bytes
            .as_chunks::<SIZE>()
            .0
            .iter()
            .map(|chunk| <i32>::from_be_bytes(*chunk))
            .collect();
        Self { data }
    }
    /// The elements as big-endian bytes, as NBT stores them.
    #[allow(dead_code)]
    pub(crate) fn to_be_bytes(&self) -> Vec<u8> {
        const SIZE: usize = size_of::<i32>();
        let mut bytes = Vec::with_capacity(self.data.len() * SIZE);
        for element in &self.data {
            bytes.extend_from_slice(&element.to_be_bytes());
        }
        bytes
    }
}
impl Deref for IntArray {
    type Target = [i32];
    fn deref(&self) -> &[i32] {
        &self.data
    }
}
impl DerefMut for IntArray {
    fn deref_mut(&mut self) -> &mut [i32] {
        &mut self.data
    }
}
impl crate::ToNBT for IntArray {
    fn tag(&self) -> u8 {
        crate::tag::TAG_INT_ARRAY
    }
    fn write<W: crate::Write>(&self, writer: &mut W) -> crate::Result<()> {
        <Self as crate::ArrayOf<i32>>::write_payload(&self.data, writer)
    }
}
impl<'de> crate::FromNBT<'de> for IntArray {
    fn read<R: crate::Read<'de>>(tag: u8, reader: &mut R) -> crate::Result<Self> {
        Ok(Self::new(<Self as crate::ArrayOf<i32>>::read(tag, reader)?))
    }
}
impl crate::ArrayOf<i32> for IntArray {
    const TAG: u8 = crate::tag::TAG_INT_ARRAY;
    fn write_payload<W: crate::Write>(elements: &[i32], writer: &mut W) -> crate::Result<()> {
        <i32 as Elements>::write(elements, writer)
    }
    fn read_payload<'de, R: crate::Read<'de>>(
        len: usize,
        reader: &mut R,
    ) -> crate::Result<Vec<i32>> {
        <i32 as Elements>::read(len, reader)
    }
}
/// The unsigned spelling of the element, sharing the same bits.
impl crate::ArrayOf<u32> for IntArray {
    const TAG: u8 = crate::tag::TAG_INT_ARRAY;
    fn write_payload<W: crate::Write>(elements: &[u32], writer: &mut W) -> crate::Result<()> {
        <u32 as Elements>::write(elements, writer)
    }
    fn read_payload<'de, R: crate::Read<'de>>(
        len: usize,
        reader: &mut R,
    ) -> crate::Result<Vec<u32>> {
        <u32 as Elements>::read(len, reader)
    }
}
impl From<&[i32]> for IntArray {
    fn from(items: &[i32]) -> Self {
        Self::new(items.to_vec())
    }
}
/// The unsigned spelling of the element, sharing the same bits.
impl From<&[u32]> for IntArray {
    fn from(items: &[u32]) -> Self {
        Self::new(items.iter().map(|&item| item.cast_signed()).collect())
    }
}
impl From<IntArray> for Vec<i32> {
    fn from(array: IntArray) -> Self {
        array.into_inner()
    }
}
/// The unsigned spelling of the element, sharing the same bits.
impl From<IntArray> for Vec<u32> {
    fn from(array: IntArray) -> Self {
        array
            .into_inner()
            .into_iter()
            .map(<i32>::cast_unsigned)
            .collect()
    }
}
impl<const N: usize> TryFrom<IntArray> for [i32; N] {
    type Error = crate::Error;
    fn try_from(array: IntArray) -> crate::Result<Self> {
        array
            .into_inner()
            .try_into()
            .map_err(|_| crate::Error::wrong_len())
    }
}
impl<const N: usize> TryFrom<IntArray> for [u32; N] {
    type Error = crate::Error;
    fn try_from(array: IntArray) -> crate::Result<Self> {
        Vec::<u32>::from(array)
            .try_into()
            .map_err(|_| crate::Error::wrong_len())
    }
}
#[cfg(feature = "serde")]
impl Serialize for IntArray {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_array(serializer, INT_ARRAY_TOKEN, &self.to_be_bytes())
    }
}
#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for IntArray {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ArrayVisitor;
        impl<'de> Visitor<'de> for ArrayVisitor {
            type Value = IntArray;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("int array")
            }
            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<IntArray, M::Error> {
                let bytes = deserialize_array(map, INT_ARRAY_TOKEN)?;
                Ok(IntArray::from_be_bytes(&bytes))
            }
        }
        deserializer.deserialize_map(ArrayVisitor)
    }
}
/// An NBT long array.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct LongArray {
    data: Vec<i64>,
}
impl LongArray {
    pub const fn new(data: Vec<i64>) -> Self {
        Self { data }
    }
    pub fn into_inner(self) -> Vec<i64> {
        self.data
    }
    /// Reads big-endian elements, ignoring a trailing partial one.
    #[cfg(feature = "serde")]
    pub(crate) fn from_be_bytes(bytes: &[u8]) -> Self {
        const SIZE: usize = size_of::<i64>();
        let data = bytes
            .as_chunks::<SIZE>()
            .0
            .iter()
            .map(|chunk| <i64>::from_be_bytes(*chunk))
            .collect();
        Self { data }
    }
    /// The elements as big-endian bytes, as NBT stores them.
    #[allow(dead_code)]
    pub(crate) fn to_be_bytes(&self) -> Vec<u8> {
        const SIZE: usize = size_of::<i64>();
        let mut bytes = Vec::with_capacity(self.data.len() * SIZE);
        for element in &self.data {
            bytes.extend_from_slice(&element.to_be_bytes());
        }
        bytes
    }
}
impl Deref for LongArray {
    type Target = [i64];
    fn deref(&self) -> &[i64] {
        &self.data
    }
}
impl DerefMut for LongArray {
    fn deref_mut(&mut self) -> &mut [i64] {
        &mut self.data
    }
}
impl crate::ToNBT for LongArray {
    fn tag(&self) -> u8 {
        crate::tag::TAG_LONG_ARRAY
    }
    fn write<W: crate::Write>(&self, writer: &mut W) -> crate::Result<()> {
        <Self as crate::ArrayOf<i64>>::write_payload(&self.data, writer)
    }
}
impl<'de> crate::FromNBT<'de> for LongArray {
    fn read<R: crate::Read<'de>>(tag: u8, reader: &mut R) -> crate::Result<Self> {
        Ok(Self::new(<Self as crate::ArrayOf<i64>>::read(tag, reader)?))
    }
}
impl crate::ArrayOf<i64> for LongArray {
    const TAG: u8 = crate::tag::TAG_LONG_ARRAY;
    fn write_payload<W: crate::Write>(elements: &[i64], writer: &mut W) -> crate::Result<()> {
        <i64 as Elements>::write(elements, writer)
    }
    fn read_payload<'de, R: crate::Read<'de>>(
        len: usize,
        reader: &mut R,
    ) -> crate::Result<Vec<i64>> {
        <i64 as Elements>::read(len, reader)
    }
}
/// The unsigned spelling of the element, sharing the same bits.
impl crate::ArrayOf<u64> for LongArray {
    const TAG: u8 = crate::tag::TAG_LONG_ARRAY;
    fn write_payload<W: crate::Write>(elements: &[u64], writer: &mut W) -> crate::Result<()> {
        <u64 as Elements>::write(elements, writer)
    }
    fn read_payload<'de, R: crate::Read<'de>>(
        len: usize,
        reader: &mut R,
    ) -> crate::Result<Vec<u64>> {
        <u64 as Elements>::read(len, reader)
    }
}
impl From<&[i64]> for LongArray {
    fn from(items: &[i64]) -> Self {
        Self::new(items.to_vec())
    }
}
/// The unsigned spelling of the element, sharing the same bits.
impl From<&[u64]> for LongArray {
    fn from(items: &[u64]) -> Self {
        Self::new(items.iter().map(|&item| item.cast_signed()).collect())
    }
}
impl From<LongArray> for Vec<i64> {
    fn from(array: LongArray) -> Self {
        array.into_inner()
    }
}
/// The unsigned spelling of the element, sharing the same bits.
impl From<LongArray> for Vec<u64> {
    fn from(array: LongArray) -> Self {
        array
            .into_inner()
            .into_iter()
            .map(<i64>::cast_unsigned)
            .collect()
    }
}
impl<const N: usize> TryFrom<LongArray> for [i64; N] {
    type Error = crate::Error;
    fn try_from(array: LongArray) -> crate::Result<Self> {
        array
            .into_inner()
            .try_into()
            .map_err(|_| crate::Error::wrong_len())
    }
}
impl<const N: usize> TryFrom<LongArray> for [u64; N] {
    type Error = crate::Error;
    fn try_from(array: LongArray) -> crate::Result<Self> {
        Vec::<u64>::from(array)
            .try_into()
            .map_err(|_| crate::Error::wrong_len())
    }
}
#[cfg(feature = "serde")]
impl Serialize for LongArray {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_array(serializer, LONG_ARRAY_TOKEN, &self.to_be_bytes())
    }
}
#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for LongArray {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ArrayVisitor;
        impl<'de> Visitor<'de> for ArrayVisitor {
            type Value = LongArray;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("long array")
            }
            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<LongArray, M::Error> {
                let bytes = deserialize_array(map, LONG_ARRAY_TOKEN)?;
                Ok(LongArray::from_be_bytes(&bytes))
            }
        }
        deserializer.deserialize_map(ArrayVisitor)
    }
}
