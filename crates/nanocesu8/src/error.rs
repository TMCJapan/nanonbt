//! The error type of this crate.

use core::fmt;

/// The bytes are neither UTF-8 nor modified UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError;

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid modified UTF-8")
    }
}

impl core::error::Error for DecodeError {}
