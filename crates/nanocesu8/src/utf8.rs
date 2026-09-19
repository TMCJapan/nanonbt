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
