//! The vectorized encode scan behind the `simd` feature.
//!
//! [`contains_null_or_utf8_4_byte_char_header`] decides whether UTF-8 bytes
//! are modified UTF-8 as they are written — which lets `Cesu8::new` accept
//! them after the UTF-8 check alone — and whether encoding a `str` can
//! borrow its bytes, a vector at a time instead of one byte at a time,
//! following the design of the `simd_cesu8` crate. UTF-8 validation is
//! delegated to the `simdutf8` crate instead, through the `utf8` module.
//!
//! The scan runs a loop of its own on every target whose vector the
//! compiler already has — NEON on aarch64, `simd128` on wasm, SSE2 on
//! x86 — so a call costs nothing but the scan. Only an input long enough
//! to amortize `pulp`'s one-off run-time detection, x86 and its AVX2, goes
//! through [`Arch::dispatch`](pulp::Arch::dispatch); everywhere else
//! `pulp` dispatches to its scalar backend.
//!
//! The Kani proofs cover the scalar paths only; the differential tests are
//! what pin this down.

/// Whether `bytes` holds a NUL or the lead of a four-byte UTF-8 sequence,
/// the two things that stop UTF-8 bytes from already being modified UTF-8.
pub(crate) fn contains_null_or_utf8_4_byte_char_header(bytes: &[u8]) -> bool {
    backend::scan(bytes)
}

/// The byte-at-a-time predicate every vector block spells out: whether one
/// byte, by itself, forces the modified spelling.
#[inline]
const fn forces_encoding(byte: u8) -> bool {
    byte == 0 || byte & 0b1111_1000 == 0b1111_0000
}

/// The aarch64 backend: NEON is part of the baseline, so the scan needs no
/// run-time detection and no dispatch, and its horizontal check is the one
/// instruction `vmaxvq_u8` — where the generic `pulp` scan would walk a
/// mask lane by lane through the stack.
#[cfg(target_arch = "aarch64")]
mod backend {
    use core::arch::aarch64::uint8x16_t;

    use pulp::core_arch::aarch64::Neon;

    /// The bytes one loop iteration scans: two registers sharing one
    /// horizontal check.
    const BLOCK: usize = 32;

    /// Whether one 16-byte block holds a NUL or a four-byte lead.
    ///
    /// # Safety
    ///
    /// The 16 bytes at `ptr` must be readable.
    #[allow(clippy::inline_always)] // the block helpers must fold into the loops that call them
    #[inline(always)]
    unsafe fn hits16(neon: Neon, ptr: *const u8, mask: uint8x16_t, header: uint8x16_t) -> bool {
        // SAFETY: the caller promises the block is readable.
        unsafe {
            let block = neon.vld1q_u8(ptr);
            let zeros = neon.vceqzq_u8(block);
            let leads = neon.vceqq_u8(neon.vandq_u8(block, mask), header);
            neon.vmaxvq_u8(neon.vorrq_u8(zeros, leads)) != 0
        }
    }

    /// Whether one 32-byte block holds a NUL or a four-byte lead: two
    /// registers under one horizontal check.
    ///
    /// # Safety
    ///
    /// The 32 bytes at `ptr` must be readable.
    #[allow(clippy::inline_always)] // as in [`hits16`]
    #[inline(always)]
    unsafe fn hits32(neon: Neon, ptr: *const u8, mask: uint8x16_t, header: uint8x16_t) -> bool {
        // SAFETY: the caller promises the block is readable.
        unsafe {
            let lo = neon.vld1q_u8(ptr);
            let hi = neon.vld1q_u8(ptr.add(16));
            let zeros = neon.vorrq_u8(neon.vceqzq_u8(lo), neon.vceqzq_u8(hi));
            let leads = neon.vorrq_u8(
                neon.vceqq_u8(neon.vandq_u8(lo, mask), header),
                neon.vceqq_u8(neon.vandq_u8(hi, mask), header),
            );
            neon.vmaxvq_u8(neon.vorrq_u8(zeros, leads)) != 0
        }
    }

    pub(super) fn scan(bytes: &[u8]) -> bool {
        if bytes.len() < 16 {
            // Short inputs — an `NBT` string, a tag name — are the common
            // case, and a byte loop is what any vector block leaves for its
            // tail anyway; keeping the shortest one here is what leaves
            // callers a small scan to inline.
            return bytes.iter().any(|&byte| super::forces_encoding(byte));
        }
        // SAFETY: NEON is part of the aarch64 baseline, so the target always
        // has it.
        let neon = unsafe { Neon::new_unchecked() };
        let mask = neon.vdupq_n_u8(0b1111_1000);
        let header = neon.vdupq_n_u8(0b1111_0000);
        if bytes.len() < BLOCK {
            // Between one register and two: the pair overlaps on the middle.
            // SAFETY: both loads are inside the slice.
            return unsafe {
                hits16(neon, bytes.as_ptr(), mask, header)
                    || hits16(neon, bytes[bytes.len() - 16..].as_ptr(), mask, header)
            };
        }
        // The first block is checked here, and a second overlapping one
        // covers inputs of up to two blocks, so a string of 64 bytes or
        // less — an `NBT` value, even a long tag name — never pays the
        // call into the outlined loop.
        // SAFETY: the slice holds at least one whole block.
        if unsafe { hits32(neon, bytes.as_ptr(), mask, header) } {
            return true;
        }
        if bytes.len() <= 2 * BLOCK {
            if bytes.len() == BLOCK {
                return false;
            }
            // The overlapping block reaches every byte the first did not.
            // SAFETY: the slice is longer than one whole block.
            return unsafe { hits32(neon, bytes[bytes.len() - BLOCK..].as_ptr(), mask, header) };
        }
        scan_blocks(bytes, neon, mask, header)
    }

    /// The block loop over what is left after the caller checked the first
    /// block. Out of line, so callers inline the short scans alone; past one
    /// block the call is noise.
    #[inline(never)]
    fn scan_blocks(bytes: &[u8], neon: Neon, mask: uint8x16_t, header: uint8x16_t) -> bool {
        debug_assert!(bytes.len() > BLOCK);
        let (chunks, rest) = bytes[BLOCK..].as_chunks::<BLOCK>();
        for chunk in chunks {
            // SAFETY: a chunk is one whole 32-byte block of the slice.
            if unsafe { hits32(neon, chunk.as_ptr().cast::<u8>(), mask, header) } {
                return true;
            }
        }
        if !rest.is_empty() {
            // The last block rereads bytes whole blocks already saw — a
            // second look changes nothing — and saves the scalar walk of
            // the tail.
            let last = bytes[bytes.len() - BLOCK..].as_ptr();
            // SAFETY: the slice is longer than a whole block.
            return unsafe { hits32(neon, last, mask, header) };
        }
        false
    }
}

/// The wasm backend: `simd128` is the target feature the crate is compiled
/// with, so like NEON it needs no detection, and `v128_any_true` is its one
/// instruction horizontal check.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod backend {
    use core::arch::wasm32::v128;

    use pulp::core_arch::wasm::Simd128;

    /// The bytes one loop iteration scans: two `v128`s sharing one
    /// horizontal check.
    const BLOCK: usize = 32;

    /// The hit mask of one 16-byte block: the lanes that are a NUL, and the
    /// lanes that lead a four-byte sequence.
    ///
    /// # Safety
    ///
    /// The 16 bytes at `ptr` must be readable.
    #[allow(clippy::inline_always)] // the block helpers must fold into the loops that call them
    #[inline(always)]
    unsafe fn hits16(simd: Simd128, ptr: *const u8, zero: v128, mask: v128, header: v128) -> v128 {
        // SAFETY: the caller promises the block is readable.
        unsafe {
            let block = simd.v128_load(ptr.cast());
            simd.v128_or(
                simd.i8x16_eq(block, zero),
                simd.i8x16_eq(simd.v128_and(block, mask), header),
            )
        }
    }

    /// [`hits16`] over two registers with one horizontal check between them.
    ///
    /// # Safety
    ///
    /// The 32 bytes at `ptr` must be readable.
    #[allow(clippy::inline_always)] // as in [`hits16`]
    #[inline(always)]
    unsafe fn hits32(simd: Simd128, ptr: *const u8, zero: v128, mask: v128, header: v128) -> bool {
        // SAFETY: the caller promises the block is readable.
        unsafe {
            let lo = hits16(simd, ptr, zero, mask, header);
            let hi = hits16(simd, ptr.add(16), zero, mask, header);
            simd.v128_any_true(simd.v128_or(lo, hi))
        }
    }

    pub(super) fn scan(bytes: &[u8]) -> bool {
        if bytes.len() < 16 {
            // Short inputs — an `NBT` string, a tag name — are the common
            // case, and a byte loop is what any vector block leaves for its
            // tail anyway; keeping the shortest one here is what leaves
            // callers a small scan to inline.
            return bytes.iter().any(|&byte| super::forces_encoding(byte));
        }
        scan_blocks(bytes)
    }

    /// The block loop over an input of at least one register. Out of line,
    /// so callers inline the short scan alone; at these lengths the one
    /// call is noise.
    #[inline(never)]
    fn scan_blocks(bytes: &[u8]) -> bool {
        debug_assert!(bytes.len() >= 16);
        let simd = Simd128::new_unchecked();
        let zero = simd.i8x16_splat(0);
        // `0b1111_1000` and `0b1111_0000` in the signed lane the splat takes.
        let mask = simd.i8x16_splat(-8);
        let header = simd.i8x16_splat(-16);
        if bytes.len() < BLOCK {
            // Between one register and two: the pair overlaps on the middle.
            // SAFETY: both loads are inside the slice.
            return unsafe {
                simd.v128_any_true(simd.v128_or(
                    hits16(simd, bytes.as_ptr(), zero, mask, header),
                    hits16(simd, bytes[bytes.len() - 16..].as_ptr(), zero, mask, header),
                ))
            };
        }
        let (chunks, rest) = bytes.as_chunks::<BLOCK>();
        for chunk in chunks {
            // SAFETY: a chunk is one whole 32-byte block of the slice.
            if unsafe { hits32(simd, chunk.as_ptr(), zero, mask, header) } {
                return true;
            }
        }
        if !rest.is_empty() {
            // The last block rereads bytes whole blocks already saw — a
            // second look changes nothing — and saves the scalar walk of
            // the tail.
            // SAFETY: the slice is at least a whole block long.
            return unsafe {
                hits32(
                    simd,
                    bytes[bytes.len() - BLOCK..].as_ptr(),
                    zero,
                    mask,
                    header,
                )
            };
        }
        false
    }
}

/// The x86 backend: SSE2 is the x86-64 baseline, so short inputs — the
/// common case, an `NBT` string — scan through it with no detection and no
/// dispatch at all, and only an input long enough to amortize the one-off
/// detection goes through `pulp`'s run-time dispatch to AVX2.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod backend {
    #[cfg(target_arch = "x86")]
    use core::arch::x86::__m128i;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::__m128i;

    use pulp::core_arch::x86::Sse2;

    /// The bytes at which the dispatched AVX2 scan starts to pay for the
    /// one-off detection of the first dispatch in a process: below it the
    /// SSE2 loop scans the same bytes for less than the detection costs,
    /// above it AVX2's double width wins.
    const DISPATCH_THRESHOLD: usize = 2048;

    /// The hit mask of one 16-byte block: the lanes that are a NUL, and the
    /// lanes that lead a four-byte sequence.
    ///
    /// # Safety
    ///
    /// The 16 bytes at `ptr` must be readable.
    #[allow(clippy::inline_always)] // the block helpers must fold into the loops that call them
    #[inline(always)]
    unsafe fn hits16(
        sse2: Sse2,
        ptr: *const u8,
        zero: __m128i,
        mask: __m128i,
        header: __m128i,
    ) -> __m128i {
        // SAFETY: the caller promises the block is readable.
        unsafe {
            let block = sse2._mm_loadu_si128(ptr.cast());
            let leads = sse2._mm_cmpeq_epi8(sse2._mm_and_si128(block, mask), header);
            sse2._mm_or_si128(sse2._mm_cmpeq_epi8(block, zero), leads)
        }
    }

    /// [`hits16`] over two registers with one move out of the vector unit
    /// between them: whether one 32-byte block holds either.
    ///
    /// # Safety
    ///
    /// The 32 bytes at `ptr` must be readable.
    #[allow(clippy::inline_always)] // as in [`hits16`]
    #[inline(always)]
    unsafe fn hits32(
        sse2: Sse2,
        ptr: *const u8,
        zero: __m128i,
        mask: __m128i,
        header: __m128i,
    ) -> bool {
        // SAFETY: the caller promises the block is readable.
        unsafe {
            let lo = hits16(sse2, ptr, zero, mask, header);
            let hi = hits16(sse2, ptr.add(16), zero, mask, header);
            sse2._mm_movemask_epi8(sse2._mm_or_si128(lo, hi)) != 0
        }
    }

    pub(super) fn scan(bytes: &[u8]) -> bool {
        if bytes.len() < DISPATCH_THRESHOLD {
            if let Some(found) = scan_sse2(bytes) {
                return found;
            }
            return bytes.iter().any(|&byte| super::forces_encoding(byte));
        }
        pulp::Arch::new().dispatch(super::Scan { bytes })
    }

    /// The SSE2 scan, or `None` where the target does not have SSE2 — the
    /// 32-bit x86 baseline does not.
    fn scan_sse2(bytes: &[u8]) -> Option<bool> {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: SSE2 is part of the x86-64 baseline, so the target always
        // has it.
        let sse2 = Some(unsafe { Sse2::new_unchecked() });
        #[cfg(target_arch = "x86")]
        let sse2 = Sse2::try_new();
        sse2.map(|sse2| {
            let zero = sse2._mm_set1_epi8(0);
            // `0b1111_1000` and `0b1111_0000` in the signed lane the splat
            // takes.
            let mask = sse2._mm_set1_epi8(-8);
            let header = sse2._mm_set1_epi8(-16);
            if bytes.len() >= 32 {
                let (chunks, rest) = bytes.as_chunks::<32>();
                for chunk in chunks {
                    // SAFETY: a chunk is one whole 32-byte block of the
                    // slice.
                    if unsafe { hits32(sse2, chunk.as_ptr(), zero, mask, header) } {
                        return true;
                    }
                }
                if !rest.is_empty() {
                    // The last block rereads bytes whole blocks already saw
                    // — a second look changes nothing — and saves the scalar
                    // walk of the tail.
                    // SAFETY: the slice is at least a whole block long.
                    return unsafe {
                        hits32(sse2, bytes[bytes.len() - 32..].as_ptr(), zero, mask, header)
                    };
                }
                return false;
            }
            if bytes.len() >= 16 {
                // Between one register and two: the pair overlaps on the
                // middle. SAFETY: both loads are inside the slice.
                return unsafe {
                    sse2._mm_movemask_epi8(sse2._mm_or_si128(
                        hits16(sse2, bytes.as_ptr(), zero, mask, header),
                        hits16(sse2, bytes[bytes.len() - 16..].as_ptr(), zero, mask, header),
                    )) != 0
                };
            }
            bytes.iter().any(|&byte| super::forces_encoding(byte))
        })
    }
}

/// Every other target: `pulp`'s dispatch, which is its scalar backend where
/// it has no vector.
#[cfg(not(any(
    target_arch = "aarch64",
    target_arch = "x86",
    target_arch = "x86_64",
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
mod backend {
    pub(super) fn scan(bytes: &[u8]) -> bool {
        pulp::Arch::new().dispatch(super::Scan { bytes })
    }
}

/// The scan itself, vectorized once `with_simd` has the target's width.
///
/// This is the body the run-time dispatch runs: the AVX2 scan on an x86 CPU
/// that has it, and `pulp`'s scalar backend everywhere the dispatch has
/// nothing wider. The targets whose vector is known at compile time run
/// their `backend` loop instead.
#[cfg(not(any(
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
struct Scan<'a> {
    bytes: &'a [u8],
}

#[cfg(not(any(
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
impl pulp::WithSimd for Scan<'_> {
    type Output = bool;

    #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
    #[inline(always)]
    fn with_simd<S: pulp::Simd>(self, simd: S) -> Self::Output {
        if S::IS_SCALAR {
            // One lane at a time is the byte loop with extra steps, and the
            // horizontal `first_true_m8s` is a scalar walk of the mask.
            return self.bytes.iter().any(|&byte| forces_encoding(byte));
        }
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
        rest.iter().any(|&byte| forces_encoding(byte))
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    use pulp::{Scalar, WithSimd};

    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    use super::Scan;
    use super::{backend, contains_null_or_utf8_4_byte_char_header, forces_encoding};

    /// The longest input the backend tests walk: past two 32-byte blocks
    /// and their seam.
    const LONGEST: usize = 72;

    /// The scalar backend, the one that runs where no vector is available,
    /// must find a NUL or a four-byte lead wherever it sits.
    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
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
                        forces_encoding(value),
                        "{value:#04x} at {pos} of {len}"
                    );
                }
            }
        }
    }

    /// The backend this target actually runs must agree with the byte
    /// predicate: every edge byte at every position across the vector
    /// blocks and their seams.
    #[test]
    fn the_target_backend_finds_the_same_bytes() {
        for value in [0x00u8, 0x01, 0x7f, 0x80, 0xef, 0xf0, 0xf7, 0xf8, 0xff] {
            for len in 0..=LONGEST {
                let plain = [b'a'; LONGEST];
                assert!(
                    !backend::scan(&plain[..len]),
                    "{len} ASCII bytes hold nothing to find"
                );
                for pos in 0..len {
                    let bytes: [u8; LONGEST] =
                        core::array::from_fn(|i| if i == pos { value } else { b'a' });
                    let bytes = &bytes[..len];
                    assert_eq!(
                        backend::scan(bytes),
                        forces_encoding(value),
                        "{value:#04x} at {pos} of {len}"
                    );
                    assert_eq!(
                        contains_null_or_utf8_4_byte_char_header(bytes),
                        bytes.iter().any(|&byte| forces_encoding(byte)),
                        "{value:#04x} at {pos} of {len} through the entry point"
                    );
                }
            }
        }
    }
}
