use core::fmt;

use crate::Tag;

/// A specialized [`Result`](core::result::Result) type for NBT operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Errors produced while decoding NBT data.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The input ended before a complete value could be read.
    UnexpectedEof,
    /// A tag byte was outside the known NBT tag range.
    UnknownTag(u8),
    /// A tag byte did not match the tag the target type expects.
    InvalidTag {
        /// The tag the target type expected.
        expected: Tag,
        /// The tag byte that was found instead.
        actual: u8,
    },
    /// A length was negative, overflowed, or could not be allocated.
    InvalidLength,
    /// The payload violated an NBT invariant (for example a non-empty list
    /// declaring the `End` element tag).
    InvalidData(&'static str),
    /// A string was not valid modified UTF-8 (CESU-8).
    InvalidUtf8,
    /// A required compound field was missing.
    MissingField(&'static str),
    /// A serde `Serialize` or `Deserialize` implementation reported a custom
    /// error, such as a value that does not match the target type.
    #[cfg(feature = "serde")]
    Message(alloc::string::String),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => formatter.write_str("unexpected end of NBT input"),
            Self::UnknownTag(tag) => write!(formatter, "unknown NBT tag {tag}"),
            Self::InvalidTag { expected, actual } => {
                write!(
                    formatter,
                    "expected NBT tag {expected} but found tag {actual}"
                )
            }
            Self::InvalidLength => formatter.write_str("invalid NBT length"),
            Self::InvalidData(reason) => write!(formatter, "invalid NBT data: {reason}"),
            Self::InvalidUtf8 => formatter.write_str("NBT string is not valid modified UTF-8"),
            Self::MissingField(name) => {
                write!(formatter, "missing NBT compound field `{name}`")
            }
            #[cfg(feature = "serde")]
            Self::Message(message) => formatter.write_str(message),
        }
    }
}

impl core::error::Error for Error {}
