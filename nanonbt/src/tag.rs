use core::fmt;

use crate::{Error, Result};

/// The NBT tag bytes defined by the NBT specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Tag {
    /// `TAG_End` (0): terminates compounds.
    End = 0,
    /// `TAG_Byte` (1).
    Byte = 1,
    /// `TAG_Short` (2).
    Short = 2,
    /// `TAG_Int` (3).
    Int = 3,
    /// `TAG_Long` (4).
    Long = 4,
    /// `TAG_Float` (5).
    Float = 5,
    /// `TAG_Double` (6).
    Double = 6,
    /// `TAG_Byte_Array` (7).
    ByteArray = 7,
    /// `TAG_String` (8).
    String = 8,
    /// `TAG_List` (9).
    List = 9,
    /// `TAG_Compound` (10).
    Compound = 10,
    /// `TAG_Int_Array` (11).
    IntArray = 11,
    /// `TAG_Long_Array` (12).
    LongArray = 12,
}

impl Tag {
    /// Converts a raw tag byte into a [`Tag`].
    #[inline]
    pub const fn from_u8(byte: u8) -> Result<Self> {
        match byte {
            0 => Ok(Self::End),
            1 => Ok(Self::Byte),
            2 => Ok(Self::Short),
            3 => Ok(Self::Int),
            4 => Ok(Self::Long),
            5 => Ok(Self::Float),
            6 => Ok(Self::Double),
            7 => Ok(Self::ByteArray),
            8 => Ok(Self::String),
            9 => Ok(Self::List),
            10 => Ok(Self::Compound),
            11 => Ok(Self::IntArray),
            12 => Ok(Self::LongArray),
            _ => Err(Error::UnknownTag(byte)),
        }
    }

    /// Returns the raw tag byte.
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns the canonical name of the tag, for example `"IntArray"`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::End => "End",
            Self::Byte => "Byte",
            Self::Short => "Short",
            Self::Int => "Int",
            Self::Long => "Long",
            Self::Float => "Float",
            Self::Double => "Double",
            Self::ByteArray => "ByteArray",
            Self::String => "String",
            Self::List => "List",
            Self::Compound => "Compound",
            Self::IntArray => "IntArray",
            Self::LongArray => "LongArray",
        }
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}
