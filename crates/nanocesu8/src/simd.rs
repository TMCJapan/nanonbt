//! The vectorized encode scan behind the `simd` feature.
//!
//! [`contains_null_or_utf8_4_byte_char_header`] decides whether encoding a
//! `str` can borrow its UTF-8 bytes, 32 bytes at a time instead of one byte
//! at a time, following the design of the `simd_cesu8` crate. UTF-8
//! validation is delegated to the `simdutf8` crate instead, through the
//! `utf8` module.
//!
//! The Kani proofs cover the scalar paths only; the differential tests are
//! what pin this down.

use wide::{CmpEq, u8x32};

/// The width of one scan block.
const BLOCK: usize = 32;

/// Whether `bytes` holds a NUL or the lead of a four-byte UTF-8 sequence,
/// the two things that stop UTF-8 bytes from already being modified UTF-8.
pub(crate) fn contains_null_or_utf8_4_byte_char_header(bytes: &[u8]) -> bool {
    let zero = u8x32::splat(0);
    let mask = u8x32::splat(0b1111_1000);
    let header = u8x32::splat(0b1111_0000);
    let (chunks, rest) = bytes.as_chunks::<BLOCK>();
    for chunk in chunks {
        let bytes = u8x32::new(*chunk);
        if bytes.cmp_eq(zero).any() || (bytes & mask).cmp_eq(header).any() {
            return true;
        }
    }
    rest.iter()
        .any(|&byte| byte == 0 || byte & 0b1111_1000 == 0b1111_0000)
}
