//! The error type shared by serialization and deserialization.

use alloc::{boxed::Box, string::ToString};
use core::fmt;

/// Why a value could not be serialized or deserialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(Kind);

/// Errors of this crate's own need no allocation; serde's messages do.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Static(&'static str),
    InvalidTag(u8),
    Custom(Box<str>),
}

pub type Result<T> = core::result::Result<T, Error>;

impl Error {
    pub(crate) const fn no_root_compound() -> Self {
        Self(Kind::Static("invalid nbt: no root compound"))
    }

    pub(crate) const fn unexpected_eof() -> Self {
        Self(Kind::Static("eof: unexpectedly ran out of input"))
    }

    pub(crate) const fn invalid_tag(tag: u8) -> Self {
        Self(Kind::InvalidTag(tag))
    }

    pub(crate) const fn nonunicode_string() -> Self {
        Self(Kind::Static("invalid nbt string: nonunicode"))
    }

    pub(crate) const fn array_token_as_key() -> Self {
        Self(Kind::Static("compound using special fastnbt array tokens"))
    }

    pub(crate) const fn list_of_end() -> Self {
        Self(Kind::Static(
            "unexpected list of type 'end', which is not supported",
        ))
    }

    pub(crate) const fn seq_too_long() -> Self {
        Self(Kind::Static("size greater than max sequence length"))
    }

    pub(crate) const fn expected_value() -> Self {
        Self(Kind::Static("expected value, found end tag"))
    }

    pub(crate) const fn negative_len() -> Self {
        Self(Kind::Static("negative array length"))
    }

    pub(crate) const fn array_too_large() -> Self {
        Self(Kind::Static("nbt array too large"))
    }

    pub(crate) const fn array_as_seq() -> Self {
        Self(Kind::Static(
            "expected NBT Array, found seq: use ByteArray, IntArray or LongArray types",
        ))
    }

    pub(crate) const fn not_bytes() -> Self {
        Self(Kind::Static("cannot convert to bytes"))
    }

    pub(crate) const fn expected_int_array() -> Self {
        Self(Kind::Static(
            "deserialize i128: expected IntArray of length 4",
        ))
    }

    pub(crate) const fn key_not_string() -> Self {
        Self(Kind::Static("field must be string-like"))
    }

    pub(crate) const fn value_before_key() -> Self {
        Self(Kind::Static("serialize_value called before serialize_key"))
    }

    pub(crate) const fn unknown_len() -> Self {
        Self(Kind::Static("sequences must have a known length"))
    }

    pub(crate) const fn len_too_large() -> Self {
        Self(Kind::Static("len too large"))
    }

    pub(crate) const fn none_in_list() -> Self {
        Self(Kind::Static("cannot serialize None in list"))
    }

    pub(crate) const fn unit() -> Self {
        Self(Kind::Static("cannot serialize unit"))
    }

    pub(crate) const fn variant() -> Self {
        Self(Kind::Static("cannot serialize newtype or struct variant"))
    }

    pub(crate) const fn array_not_bytes() -> Self {
        Self(Kind::Static(
            "expected NBT Array: use ByteArray, IntArray or LongArray types",
        ))
    }

    pub(crate) const fn string_too_long() -> Self {
        Self(Kind::Static("string longer than 65535 bytes"))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Kind::Static(message) => f.write_str(message),
            Kind::InvalidTag(tag) => write!(f, "invalid nbt tag value: {tag}"),
            Kind::Custom(message) => f.write_str(message),
        }
    }
}

impl core::error::Error for Error {}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(Kind::Custom(msg.to_string().into_boxed_str()))
    }
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(Kind::Custom(msg.to_string().into_boxed_str()))
    }
}
