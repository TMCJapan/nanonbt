//! A complete NBT tree, like `serde_json::Value`.

#[cfg(feature = "serde")]
mod from;
#[cfg(feature = "serde")]
mod serde_impl;
#[cfg(feature = "serde")]
pub(crate) use serde_impl::{from_value_serde, to_value_serde};

use alloc::{
    borrow::Cow,
    collections::{BTreeMap, btree_map},
    slice,
    string::String,
    vec::Vec,
};

use crate::{
    DeOpts,
    arrays::{ByteArray, IntArray, LongArray},
    error::{Error, Result},
    impls::{read_list, write_list},
    read::{FromNBT, Read},
    tag::{
        TAG_BYTE, TAG_BYTE_ARRAY, TAG_COMPOUND, TAG_DOUBLE, TAG_END, TAG_FLOAT, TAG_INT,
        TAG_INT_ARRAY, TAG_LIST, TAG_LONG, TAG_LONG_ARRAY, TAG_SHORT, TAG_STRING,
    },
    write::{ToNBT, Write},
};

/// Any NBT value, owning its data.
///
/// Compounds are ordered by key, where fastnbt's are in hash order; the name
/// of the root compound is not kept.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    ByteArray(ByteArray),
    IntArray(IntArray),
    LongArray(LongArray),
    List(Vec<Self>),
    Compound(BTreeMap<String, Self>),
}

impl ToNBT for Value {
    fn tag(&self) -> u8 {
        match self {
            Self::Byte(_) => TAG_BYTE,
            Self::Short(_) => TAG_SHORT,
            Self::Int(_) => TAG_INT,
            Self::Long(_) => TAG_LONG,
            Self::Float(_) => TAG_FLOAT,
            Self::Double(_) => TAG_DOUBLE,
            Self::String(_) => TAG_STRING,
            Self::ByteArray(_) => TAG_BYTE_ARRAY,
            Self::IntArray(_) => TAG_INT_ARRAY,
            Self::LongArray(_) => TAG_LONG_ARRAY,
            Self::List(_) => TAG_LIST,
            Self::Compound(_) => TAG_COMPOUND,
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            Self::Byte(v) => writer.write_i8(*v),
            Self::Short(v) => writer.write_i16(*v),
            Self::Int(v) => writer.write_i32(*v),
            Self::Long(v) => writer.write_i64(*v),
            Self::Float(v) => writer.write_f32(*v),
            Self::Double(v) => writer.write_f64(*v),
            Self::String(v) => writer.write_str(v),
            Self::ByteArray(v) => v.write(writer),
            Self::IntArray(v) => v.write(writer),
            Self::LongArray(v) => v.write(writer),
            Self::List(v) => write_list(v, writer),
            Self::Compound(v) => {
                for (name, value) in v {
                    value.write_entry(name, writer)?;
                }
                writer.write_end()
            }
        }
    }
}

impl<'de> FromNBT<'de> for Value {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        match tag {
            TAG_BYTE => Ok(Self::Byte(reader.read_i8()?)),
            TAG_SHORT => Ok(Self::Short(reader.read_i16()?)),
            TAG_INT => Ok(Self::Int(reader.read_i32()?)),
            TAG_LONG => Ok(Self::Long(reader.read_i64()?)),
            TAG_FLOAT => Ok(Self::Float(reader.read_f32()?)),
            TAG_DOUBLE => Ok(Self::Double(reader.read_f64()?)),
            TAG_STRING => Ok(Self::String(reader.read_str()?.into_owned())),
            TAG_BYTE_ARRAY => Ok(Self::ByteArray(ByteArray::read(tag, reader)?)),
            TAG_INT_ARRAY => Ok(Self::IntArray(IntArray::read(tag, reader)?)),
            TAG_LONG_ARRAY => Ok(Self::LongArray(LongArray::read(tag, reader)?)),
            TAG_LIST => Ok(Self::List(read_list(reader)?)),
            TAG_COMPOUND => {
                let mut map = BTreeMap::new();
                reader.nest(|reader| {
                    loop {
                        let tag = reader.read_tag()?;
                        if tag == TAG_END {
                            return Ok(());
                        }
                        let name = reader.read_name()?;
                        map.insert(name.into_owned(), Self::read(tag, reader)?);
                    }
                })?;
                Ok(Self::Compound(map))
            }
            _ => Err(Error::invalid_tag(tag)),
        }
    }
}

impl Value {
    /// Any number, cast to `i64` the way `as` does.
    #[allow(clippy::cast_possible_truncation)] // `as` semantics, as fastnbt
    pub const fn as_i64(&self) -> Option<i64> {
        match *self {
            Self::Byte(v) => Some(v as i64),
            Self::Short(v) => Some(v as i64),
            Self::Int(v) => Some(v as i64),
            Self::Long(v) => Some(v),
            Self::Float(v) => Some(v as i64),
            Self::Double(v) => Some(v as i64),
            _ => None,
        }
    }

    /// Any number, cast to `u64` the way `as` does.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // `as` semantics, as fastnbt
    pub const fn as_u64(&self) -> Option<u64> {
        match *self {
            Self::Byte(v) => Some(v as u64),
            Self::Short(v) => Some(v as u64),
            Self::Int(v) => Some(v as u64),
            Self::Long(v) => Some(v as u64),
            Self::Float(v) => Some(v as u64),
            Self::Double(v) => Some(v as u64),
            _ => None,
        }
    }

    /// Any number, cast to `f64` the way `as` does.
    #[allow(clippy::cast_precision_loss)] // `as` semantics, as fastnbt
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Self::Byte(v) => Some(f64::from(v)),
            Self::Short(v) => Some(f64::from(v)),
            Self::Int(v) => Some(f64::from(v)),
            Self::Long(v) => Some(v as f64),
            Self::Float(v) => Some(f64::from(v)),
            Self::Double(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }
}

/// Interprets a [`Value`] as a `T`.
///
/// Strings and compound names borrow from the tree, so a `&'de str` field
/// stays zero-copy.
pub fn from_value<'de, T: FromNBT<'de>>(value: &'de Value) -> Result<T> {
    let mut reader = ValueReader::new(value);
    T::read(value.tag(), &mut reader)
}

/// Converts any writable `value` into a [`Value`].
///
/// The value is written to a byte buffer first and read back as a tree, which
/// keeps one implementation of the format instead of a second tree builder.
/// One consequence: a [`Value::List`] of mixed element types is not valid NBT
/// and is refused, where the serde conversion kept it.
pub fn to_value<T: ToNBT + ?Sized>(value: &T) -> Result<Value> {
    let mut bytes = Vec::new();
    let mut writer = crate::write::Writer::new(&mut bytes);
    writer.write_tag(value.tag())?;
    value.write(&mut writer)?;
    let mut reader = crate::read::Reader::new(&bytes, DeOpts::new());
    let tag = reader.read_tag()?;
    Value::read(tag, &mut reader)
}

/// A [`Read`] that walks a tree instead of a byte slice.
struct ValueReader<'de> {
    stack: Vec<Frame<'de>>,
    /// The value whose payload is about to be read.
    current: Option<&'de Value>,
    /// The name of the entry `current` came from.
    pending_name: Option<&'de str>,
    /// The big-endian payload an array's `read_len` prepared.
    array_bytes: Option<Vec<u8>>,
    depth: usize,
    max_depth: usize,
}

enum Frame<'de> {
    Compound(btree_map::Iter<'de, String, Value>),
    List(slice::Iter<'de, Value>),
}

/// One step of walking the tree in [`ValueReader::read_tag`].
enum Step<'de> {
    Entry(&'de str, &'de Value),
    Value(&'de Value),
    End,
    Pop,
}

impl<'de> ValueReader<'de> {
    const fn new(value: &'de Value) -> Self {
        Self {
            stack: Vec::new(),
            current: Some(value),
            pending_name: None,
            array_bytes: None,
            depth: 0,
            max_depth: DeOpts::new().max_depth,
        }
    }

    /// The next value to read a payload from: the current one, or the next
    /// element of the list on top. Exhausted list frames are dropped.
    fn take_value(&mut self) -> Result<&'de Value> {
        if let Some(value) = self.current.take() {
            return Ok(value);
        }
        loop {
            let next = match self.stack.last_mut() {
                Some(Frame::List(elements)) => elements.next(),
                _ => return Err(Error::unexpected_eof()),
            };
            match next {
                Some(value) => return Ok(value),
                None => {
                    self.stack.pop();
                }
            }
        }
    }

    /// Enters a compound whose entries are read from here on.
    fn descend_compound(&mut self, value: &'de Value) -> Result<u8> {
        match value {
            Value::Compound(map) => {
                self.stack.push(Frame::Compound(map.iter()));
                self.next_compound_tag()
            }
            other => Err(Error::invalid_tag(other.tag())),
        }
    }

    /// The next entry's tag, or End when the compound ends.
    fn next_compound_tag(&mut self) -> Result<u8> {
        let next = match self.stack.last_mut() {
            Some(Frame::Compound(entries)) => entries.next(),
            _ => return Err(Error::unexpected_eof()),
        };
        if let Some((name, value)) = next {
            self.pending_name = Some(name);
            self.current = Some(value);
            Ok(value.tag())
        } else {
            self.stack.pop();
            Ok(TAG_END)
        }
    }
}

impl<'de> Read<'de> for ValueReader<'de> {
    fn read_tag(&mut self) -> Result<u8> {
        if let Some(value) = self.current.take() {
            return self.descend_compound(value);
        }
        loop {
            let step = match self.stack.last_mut() {
                Some(Frame::Compound(entries)) => match entries.next() {
                    Some((name, value)) => Step::Entry(name, value),
                    None => Step::End,
                },
                Some(Frame::List(elements)) => elements.next().map_or(Step::Pop, Step::Value),
                None => return Err(Error::unexpected_eof()),
            };
            match step {
                Step::Entry(name, value) => {
                    self.pending_name = Some(name);
                    self.current = Some(value);
                    return Ok(value.tag());
                }
                Step::End => {
                    self.stack.pop();
                    return Ok(TAG_END);
                }
                Step::Value(value) => return self.descend_compound(value),
                Step::Pop => {
                    self.stack.pop();
                }
            }
        }
    }

    fn read_name(&mut self) -> Result<Cow<'de, str>> {
        self.pending_name
            .take()
            .map(Cow::Borrowed)
            .ok_or_else(Error::unexpected_eof)
    }

    fn read_str(&mut self) -> Result<Cow<'de, str>> {
        match self.take_value()? {
            Value::String(text) => Ok(Cow::Borrowed(text)),
            other => Err(Error::invalid_tag(other.tag())),
        }
    }

    fn read_i8(&mut self) -> Result<i8> {
        match self.take_value()? {
            Value::Byte(v) => Ok(*v),
            other => Err(Error::invalid_tag(other.tag())),
        }
    }

    fn read_i16(&mut self) -> Result<i16> {
        match self.take_value()? {
            Value::Short(v) => Ok(*v),
            other => Err(Error::invalid_tag(other.tag())),
        }
    }

    fn read_i32(&mut self) -> Result<i32> {
        match self.take_value()? {
            Value::Int(v) => Ok(*v),
            other => Err(Error::invalid_tag(other.tag())),
        }
    }

    fn read_i64(&mut self) -> Result<i64> {
        match self.take_value()? {
            Value::Long(v) => Ok(*v),
            other => Err(Error::invalid_tag(other.tag())),
        }
    }

    fn read_f32(&mut self) -> Result<f32> {
        match self.take_value()? {
            Value::Float(v) => Ok(*v),
            other => Err(Error::invalid_tag(other.tag())),
        }
    }

    fn read_f64(&mut self) -> Result<f64> {
        match self.take_value()? {
            Value::Double(v) => Ok(*v),
            other => Err(Error::invalid_tag(other.tag())),
        }
    }

    fn read_list_header(&mut self) -> Result<(u8, usize)> {
        match self.take_value()? {
            Value::List(list) => {
                let element = list.first().map_or(TAG_END, ToNBT::tag);
                self.stack.push(Frame::List(list.iter()));
                Ok((element, list.len()))
            }
            other => Err(Error::invalid_tag(other.tag())),
        }
    }

    fn read_len(&mut self) -> Result<usize> {
        let value = self.take_value()?;
        let (len, bytes) = match value {
            Value::ByteArray(array) => (array.len(), array.to_be_bytes()),
            Value::IntArray(array) => (array.len(), array.to_be_bytes()),
            Value::LongArray(array) => (array.len(), array.to_be_bytes()),
            other => return Err(Error::invalid_tag(other.tag())),
        };
        self.array_bytes = Some(bytes);
        Ok(len)
    }

    fn read_bytes(&mut self, len: usize) -> Result<Cow<'de, [u8]>> {
        let bytes = self.array_bytes.take().ok_or_else(Error::unexpected_eof)?;
        if bytes.len() != len {
            return Err(Error::unexpected_eof());
        }
        Ok(Cow::Owned(bytes))
    }

    fn skip(&mut self, _tag: u8) -> Result<()> {
        if self.current.take().is_none() {
            self.take_value()?;
        }
        Ok(())
    }

    fn nest<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.depth >= self.max_depth {
            return Err(Error::too_deep());
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }
}
