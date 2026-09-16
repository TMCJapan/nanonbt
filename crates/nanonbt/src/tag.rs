//! The NBT type tags, which the reader and the writer must agree on.

pub(crate) const TAG_END: u8 = 0;
pub(crate) const TAG_BYTE: u8 = 1;
pub(crate) const TAG_SHORT: u8 = 2;
pub(crate) const TAG_INT: u8 = 3;
pub(crate) const TAG_LONG: u8 = 4;
pub(crate) const TAG_FLOAT: u8 = 5;
pub(crate) const TAG_DOUBLE: u8 = 6;
pub(crate) const TAG_BYTE_ARRAY: u8 = 7;
pub(crate) const TAG_STRING: u8 = 8;
pub(crate) const TAG_LIST: u8 = 9;
pub(crate) const TAG_COMPOUND: u8 = 10;
pub(crate) const TAG_INT_ARRAY: u8 = 11;
pub(crate) const TAG_LONG_ARRAY: u8 = 12;

/// The largest tag there is, past which a byte names no type.
pub(crate) const TAG_MAX: u8 = TAG_LONG_ARRAY;
