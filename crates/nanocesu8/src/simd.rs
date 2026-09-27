//! The vectorized encode scan behind the `simd` feature.
//!
//! [`contains_null_or_utf8_4_byte_char_header`] decides whether UTF-8 bytes
//! are modified UTF-8 as they are written — which lets `Cesu8::new` accept
//! them after the UTF-8 check alone — and whether encoding a `str` can
//! borrow its bytes, a vector at a time instead of one byte at a time,
//! following the design of the `simd_cesu8` crate. UTF-8 validation is
//! delegated to the `simdutf8` crate instead, through the `utf8` module.
//!
//! The scan is written once against `pulp`'s [`Simd`] trait and dispatched
//! to the widest vector the target has when it runs. The Kani proofs cover
//! the scalar paths only; the differential tests are what pin this down.

use pulp::{Arch, Simd, WithSimd};

/// Whether `bytes` holds a NUL or the lead of a four-byte UTF-8 sequence,
/// the two things that stop UTF-8 bytes from already being modified UTF-8.
pub(crate) fn contains_null_or_utf8_4_byte_char_header(bytes: &[u8]) -> bool {
    Arch::new().dispatch(Scan { bytes })
}

/// The scan itself, vectorized once `with_simd` has the target's width.
struct Scan<'a> {
    bytes: &'a [u8],
}

impl WithSimd for Scan<'_> {
    type Output = bool;

    #[inline]
    fn with_simd<S: Simd>(self, simd: S) -> Self::Output {
        let zero = simd.splat_u8s(0);
        let mask = simd.splat_u8s(0b1111_1000);
        let header = simd.splat_u8s(0b1111_0000);
        let (chunks, rest) = S::as_simd_u8s(self.bytes);
        for chunk in chunks {
            let hits = simd.or_m8s(
                simd.equal_u8s(*chunk, zero),
                simd.equal_u8s(simd.and_u8s(*chunk, mask), header),
            );
            // `first_true_m8s` is the lane count when no lane is set.
            if simd.first_true_m8s(hits) < S::U8_LANES {
                return true;
            }
        }
        rest.iter()
            .any(|&byte| byte == 0 || byte & 0b1111_1000 == 0b1111_0000)
    }
}

#[cfg(test)]
mod tests {
    use pulp::{Scalar, WithSimd};

    use super::Scan;

    /// The scalar backend, the one that runs where no vector is available,
    /// must find a NUL or a four-byte lead wherever it sits.
    #[test]
    fn the_scalar_backend_finds_the_same_bytes() {
        assert!(!Scan { bytes: &[] }.with_simd(Scalar));
        for len in 1..=16usize {
            for value in 0..=255u8 {
                for pos in 0..len {
                    let bytes: [u8; 17] =
                        core::array::from_fn(|i| if i < len && i == pos { value } else { b'a' });
                    let found = Scan {
                        bytes: &bytes[..len],
                    }
                    .with_simd(Scalar);
                    assert_eq!(
                        found,
                        value == 0 || value & 0b1111_1000 == 0b1111_0000,
                        "{value:#04x} at {pos} of {len}"
                    );
                }
            }
        }
    }
}
