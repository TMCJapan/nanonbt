//! The UTF-8 check every entry point starts with, vectorized under the
//! `simd` feature.

/// Borrows `bytes` as a `str` when they are valid UTF-8.
#[inline]
pub(crate) fn to_str(bytes: &[u8]) -> Option<&str> {
    #[cfg(feature = "simd")]
    {
        simdutf8::basic::from_utf8(bytes).ok()
    }
    #[cfg(not(feature = "simd"))]
    {
        core::str::from_utf8(bytes).ok()
    }
}

/// Whether every byte is plain ASCII and none of them is NUL.
///
/// That is exactly Java's spelling of the same text, so the bytes need no
/// check beyond this scan: they are valid UTF-8 and no `C0 80` or surrogate
/// pair can be hiding in them.
#[inline]
pub(crate) fn is_plain_ascii(bytes: &[u8]) -> bool {
    // Written word at a time: a byte of `1..=0x7f` has no high bit, and the
    // borrow of `word - 1` marks a zero byte.
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGHS: u64 = 0x8080_8080_8080_8080;
    let (chunks, tail) = bytes.as_chunks::<8>();
    chunks.iter().all(|chunk| {
        let word = u64::from_ne_bytes(*chunk);
        let high = word & HIGHS;
        let zero = word.wrapping_sub(ONES) & !word & HIGHS;
        high | zero == 0
    }) && tail.iter().all(|&byte| byte != 0 && byte < 0x80)
}
