//! The error type shared by serialization and deserialization.

#[cfg(feature = "serde")]
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
    MissingField(&'static str),
    #[cfg(feature = "serde")]
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

    /// A tag byte that names no NBT type, or one a value cannot have.
    pub const fn invalid_tag(tag: u8) -> Self {
        Self(Kind::InvalidTag(tag))
    }

    /// A struct field the document has no entry for.
    pub const fn missing_field(name: &'static str) -> Self {
        Self(Kind::MissingField(name))
    }

    /// An enum variant name that matches no variant.
    pub const fn unknown_variant() -> Self {
        Self(Kind::Static("unknown enum variant"))
    }

    pub(crate) const fn borrowed_string() -> Self {
        Self(Kind::Static(
            "string is not plain UTF-8 and cannot be borrowed; use Cow<str> or String",
        ))
    }

    pub(crate) const fn borrowed_cesu8() -> Self {
        Self(Kind::Static(
            "CESU-8 string is owned and cannot be borrowed; use Cow<Cesu8> or Cesu8Buf",
        ))
    }

    pub(crate) const fn borrowed_bytes() -> Self {
        Self(Kind::Static(
            "value is owned and cannot be borrowed; use an owned type",
        ))
    }

    pub(crate) const fn invalid_char() -> Self {
        Self(Kind::Static("integer is not a unicode scalar value"))
    }

    /// A sequence whose length differs from the target fixed-size array.
    pub const fn wrong_len() -> Self {
        Self(Kind::Static(
            "sequence has a different length than the target array",
        ))
    }

    pub(crate) const fn nonunicode_string() -> Self {
        Self(Kind::Static("invalid nbt string: nonunicode"))
    }

    pub(crate) const fn list_of_end() -> Self {
        Self(Kind::Static(
            "unexpected list of type 'end', which is not supported",
        ))
    }

    pub(crate) const fn seq_too_long() -> Self {
        Self(Kind::Static("size greater than max sequence length"))
    }

    pub(crate) const fn too_deep() -> Self {
        Self(Kind::Static("nesting deeper than max depth"))
    }

    #[cfg(feature = "serde")]
    pub(crate) const fn expected_value() -> Self {
        Self(Kind::Static("expected value, found end tag"))
    }

    pub(crate) const fn negative_len() -> Self {
        Self(Kind::Static("negative array length"))
    }

    pub(crate) const fn array_too_large() -> Self {
        Self(Kind::Static("nbt array too large"))
    }

    #[cfg(feature = "serde")]
    pub(crate) const fn not_bytes() -> Self {
        Self(Kind::Static("cannot convert to bytes"))
    }

    pub(crate) const fn expected_int_array() -> Self {
        Self(Kind::Static(
            "deserialize i128: expected int array of length 4",
        ))
    }

    #[cfg(feature = "serde")]
    pub(crate) const fn key_not_string() -> Self {
        Self(Kind::Static("field must be string-like"))
    }

    #[cfg(feature = "serde")]
    pub(crate) const fn value_before_key() -> Self {
        Self(Kind::Static("serialize_value called before serialize_key"))
    }

    #[cfg(feature = "serde")]
    pub(crate) const fn unknown_len() -> Self {
        Self(Kind::Static("sequences must have a known length"))
    }

    pub(crate) const fn len_too_large() -> Self {
        Self(Kind::Static("len too large"))
    }

    #[cfg(feature = "serde")]
    pub(crate) const fn none_in_list() -> Self {
        Self(Kind::Static("cannot serialize None in list"))
    }

    #[cfg(feature = "serde")]
    pub(crate) const fn unit() -> Self {
        Self(Kind::Static("cannot serialize unit"))
    }

    #[cfg(feature = "serde")]
    pub(crate) const fn variant() -> Self {
        Self(Kind::Static("cannot serialize newtype or struct variant"))
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
            Kind::MissingField(name) => write!(f, "missing field `{name}`"),
            #[cfg(feature = "serde")]
            Kind::Custom(message) => f.write_str(message),
        }
    }
}

impl core::error::Error for Error {}

#[cfg(feature = "serde")]
impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(Kind::Custom(msg.to_string().into_boxed_str()))
    }
}

#[cfg(feature = "serde")]
impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(Kind::Custom(msg.to_string().into_boxed_str()))
    }
}
