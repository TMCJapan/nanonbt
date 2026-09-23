//! The vectorized byte-order decode behind the `simd` feature.
//!
//! NBT stores numbers big-endian, so decoding a whole array or list of them
//! is a byte reversal of every element. The compiler vectorizes a plain
//! `map(from_be_bytes)` loop on its own where it recognizes the shape, but
//! only in an optimized build; this module makes the reversal explicit, so
//! that it happens whatever the optimization level and whatever the target's
//! baseline is.
//!
//! [`decode_be`] writes the swapped elements straight into the `Vec` they
//! become, so a payload is read with one allocation and one pass over it.
//! [`swap_bytes_in_place`] is the same reversal where the bytes are already
//! in the buffer they belong in: the write side stages a chunk of elements
//! on the stack, swaps it there and hands it to the writer.
//!
//! Targets without a vector path fall back to the scalar loop, and the tests
//! hold the two against each other.

use alloc::vec::Vec;
use core::mem::size_of_val;

use crate::{
    error::{Error, Result},
    read::Read,
    write::Write,
};

/// Whether this target has a vector byte-swap path.
const HAS_SIMD: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2"),
    all(target_arch = "aarch64", target_feature = "neon"),
    all(target_arch = "wasm32", target_feature = "simd128"),
));

/// The bytes one vector loop moves at a time.
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "avx2"
))]
const BLOCK: usize = 32;

/// The bytes one vector loop moves at a time.
#[cfg(not(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "avx2"
)))]
const BLOCK: usize = 16;

/// The bytes the write side stages on the stack at a time.
const CHUNK: usize = 256;

/// Reverses the bytes of every `SIZE`-byte element of `bytes`, in place.
///
/// `SIZE` must be 2, 4 or 8, and the length a multiple of it. On a
/// big-endian target the bytes are already in the order a value is read in,
/// and this does nothing.
pub(crate) fn swap_bytes_in_place<const SIZE: usize>(bytes: &mut [u8]) {
    if cfg!(target_endian = "big") || SIZE == 1 {
        return;
    }
    debug_assert!(matches!(SIZE, 2 | 4 | 8));
    debug_assert!(bytes.len().is_multiple_of(SIZE));
    let dst = bytes.as_mut_ptr();
    let src = dst.cast_const();
    let vector = bytes.len() / BLOCK * BLOCK;
    // SAFETY: `src` and `dst` are the same live buffer, and both ranges are
    // inside it: the vector part is `vector` bytes, the tail the rest.
    unsafe {
        swap_blocks::<SIZE>(src, dst, vector);
        scalar::swap_copy::<SIZE>(src.add(vector), dst.add(vector), bytes.len() - vector);
    }
}

/// Decodes `SIZE`-byte big-endian elements into a `Vec`.
///
/// The bytes are copied into the `Vec`'s buffer and swapped there, so the
/// payload is read with one allocation and one pass over it. A trailing
/// partial element is ignored, the way `as_chunks` ignores one. `decode`
/// settles the elements the vector loop does not cover, and is all that runs
/// when the target has no vector path.
pub(crate) fn decode_be<T: Copy, const SIZE: usize>(
    bytes: &[u8],
    decode: fn([u8; SIZE]) -> T,
) -> Vec<T> {
    const { assert!(SIZE == size_of::<T>()) };
    let len = bytes.len() / SIZE;
    let mut out = Vec::with_capacity(len);
    if !HAS_SIMD || cfg!(target_endian = "big") {
        out.extend(
            bytes
                .as_chunks::<SIZE>()
                .0
                .iter()
                .map(|chunk| decode(*chunk)),
        );
        return out;
    }
    let vector = len * SIZE / BLOCK * BLOCK;
    // SAFETY: the `Vec` has room for `len` elements, and the vector loop
    // fills `vector / SIZE` of them from `bytes`, which has `len * SIZE`
    // initialized bytes.
    unsafe {
        let dst = out.as_mut_ptr().cast::<u8>();
        swap_blocks::<SIZE>(bytes.as_ptr(), dst, vector);
        out.set_len(vector / SIZE);
    }
    out.extend(
        bytes[vector..]
            .as_chunks::<SIZE>()
            .0
            .iter()
            .map(|chunk| decode(*chunk)),
    );
    out
}

/// Reads `len` big-endian elements of `SIZE` bytes each: a list's payload,
/// whose header the caller has already read.
pub(crate) fn read_be_elements<'de, T: Copy, const SIZE: usize, R: Read<'de>>(
    len: usize,
    reader: &mut R,
    decode: fn([u8; SIZE]) -> T,
) -> Result<Vec<T>> {
    let n = len.checked_mul(SIZE).ok_or_else(Error::array_too_large)?;
    let bytes = reader.read_bytes(n)?;
    // A reader that hands back fewer bytes than asked for has hit the end of
    // the input; the elements it did lend are not the whole list.
    if bytes.len() != n {
        return Err(Error::unexpected_eof());
    }
    Ok(decode_be::<T, SIZE>(&bytes, decode))
}

/// Writes native-order `SIZE`-byte elements as big-endian, in as few writes
/// as the writer takes them.
pub(crate) fn write_be<T: Copy, const SIZE: usize, W: Write + ?Sized>(
    elements: &[T],
    writer: &mut W,
) -> Result<()> {
    const { assert!(SIZE == size_of::<T>()) };
    let bytes = as_bytes(elements);
    let mut chunk = [0u8; CHUNK];
    for part in bytes.chunks(CHUNK) {
        debug_assert!(part.len().is_multiple_of(SIZE));
        let staged = &mut chunk[..part.len()];
        staged.copy_from_slice(part);
        swap_bytes_in_place::<SIZE>(staged);
        writer.write_bytes(staged)?;
    }
    Ok(())
}

/// The bytes of a slice of scalar elements, as they lie in memory.
///
/// Every bit pattern of the scalar types this is called with is a valid
/// byte, and `T: Copy` has no drop glue, so the reinterpretation is sound.
const fn as_bytes<T: Copy>(elements: &[T]) -> &[u8] {
    // SAFETY: the slice is contiguous, `size_of_val` is the length of the
    // bytes it covers, and every one of them is initialized.
    unsafe { core::slice::from_raw_parts(elements.as_ptr().cast::<u8>(), size_of_val(elements)) }
}

/// Swaps `n` bytes, a vector block at a time, on targets that have one.
///
/// `n` must be a multiple of [`BLOCK`].
///
/// # Safety
/// `src` and `dst` must each be valid for `n` bytes. They may be the same
/// buffer: a block is loaded before it is stored.
unsafe fn swap_blocks<const SIZE: usize>(src: *const u8, dst: *mut u8, n: usize) {
    debug_assert!(n.is_multiple_of(BLOCK));
    if !matches!(SIZE, 2 | 4 | 8) {
        // SAFETY: the caller upholds the contract.
        unsafe { scalar::swap_copy::<SIZE>(src, dst, n) };
        return;
    }
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "x86", target_feature = "sse2")
    ))]
    // SAFETY: the caller upholds the contract.
    unsafe {
        x86::swap_blocks::<SIZE>(src, dst, n);
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    // SAFETY: the caller upholds the contract.
    unsafe {
        neon::swap_blocks::<SIZE>(src, dst, n);
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    // SAFETY: the caller upholds the contract.
    unsafe {
        wasm::swap_blocks::<SIZE>(src, dst, n);
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        all(target_arch = "x86", target_feature = "sse2"),
        all(target_arch = "aarch64", target_feature = "neon"),
        all(target_arch = "wasm32", target_feature = "simd128"),
    )))]
    // SAFETY: the caller upholds the contract.
    unsafe {
        scalar::swap_copy::<SIZE>(src, dst, n);
    }
}

/// The SSE2 path, and `pshufb`/`vpshufb` where the target has SSSE3 or AVX2.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86 {
    #[cfg(target_arch = "x86")]
    use core::arch::x86 as intrinsics;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64 as intrinsics;

    use super::BLOCK;

    /// The `pshufb` control that reverses each `SIZE`-byte element.
    #[inline]
    fn mask<const SIZE: usize>() -> intrinsics::__m128i {
        // SAFETY: SSE2 is on, which every target here has.
        unsafe {
            match SIZE {
                2 => {
                    intrinsics::_mm_setr_epi8(1, 0, 3, 2, 5, 4, 7, 6, 9, 8, 11, 10, 13, 12, 15, 14)
                }
                4 => {
                    intrinsics::_mm_setr_epi8(3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12)
                }
                8 => {
                    intrinsics::_mm_setr_epi8(7, 6, 5, 4, 3, 2, 1, 0, 15, 14, 13, 12, 11, 10, 9, 8)
                }
                _ => intrinsics::_mm_setzero_si128(),
            }
        }
    }

    /// Swaps one 32-byte block, four 16-byte lanes at a time.
    #[cfg(target_feature = "avx2")]
    #[inline]
    fn swap_32<const SIZE: usize>(block: intrinsics::__m256i) -> intrinsics::__m256i {
        // SAFETY: AVX2 is on, which is what `_mm256_shuffle_epi8` requires.
        unsafe {
            let control = intrinsics::_mm256_set_m128i(mask::<SIZE>(), mask::<SIZE>());
            intrinsics::_mm256_shuffle_epi8(block, control)
        }
    }

    /// Swaps one 16-byte block.
    #[cfg(not(target_feature = "avx2"))]
    #[inline]
    fn swap_16<const SIZE: usize>(block: intrinsics::__m128i) -> intrinsics::__m128i {
        match SIZE {
            2 => swap_16_u16(block),
            4 => swap_16_u32(block),
            8 => swap_16_u64(block),
            _ => block,
        }
    }

    #[cfg(not(target_feature = "avx2"))]
    #[inline]
    fn swap_16_u16(block: intrinsics::__m128i) -> intrinsics::__m128i {
        if cfg!(target_feature = "ssse3") {
            // SAFETY: SSSE3 is on.
            unsafe { intrinsics::_mm_shuffle_epi8(block, mask::<2>()) }
        } else {
            // SAFETY: SSE2 is on.
            unsafe {
                intrinsics::_mm_or_si128(
                    intrinsics::_mm_srli_epi16(block, 8),
                    intrinsics::_mm_slli_epi16(block, 8),
                )
            }
        }
    }

    #[cfg(not(target_feature = "avx2"))]
    #[inline]
    fn swap_16_u32(block: intrinsics::__m128i) -> intrinsics::__m128i {
        if cfg!(target_feature = "ssse3") {
            // SAFETY: SSSE3 is on.
            unsafe { intrinsics::_mm_shuffle_epi8(block, mask::<4>()) }
        } else {
            // SAFETY: SSE2 is on. Swap the bytes of each 16-bit word, then
            // the two words of each 32-bit dword.
            let block = swap_16_u16(block);
            unsafe {
                intrinsics::_mm_shufflehi_epi16(
                    intrinsics::_mm_shufflelo_epi16(block, 0b1011_0001),
                    0b1011_0001,
                )
            }
        }
    }

    #[cfg(not(target_feature = "avx2"))]
    #[inline]
    fn swap_16_u64(block: intrinsics::__m128i) -> intrinsics::__m128i {
        if cfg!(target_feature = "ssse3") {
            // SAFETY: SSSE3 is on.
            unsafe { intrinsics::_mm_shuffle_epi8(block, mask::<8>()) }
        } else {
            // SAFETY: SSE2 is on. The 32-bit halves of each 64-bit element
            // are the last two to swap.
            unsafe { intrinsics::_mm_shuffle_epi32(swap_16_u32(block), 0b1011_0001) }
        }
    }

    /// Swaps `n` bytes, 32 at a time.
    ///
    /// # Safety
    /// `src` and `dst` must each be valid for `n` bytes, `n` a multiple of
    /// 32. They may be the same buffer.
    #[cfg(target_feature = "avx2")]
    pub(super) unsafe fn swap_blocks<const SIZE: usize>(src: *const u8, dst: *mut u8, n: usize) {
        let mut i = 0;
        while i + BLOCK <= n {
            // SAFETY: the caller says both pointers are valid for `n` bytes,
            // and `i + BLOCK <= n`.
            unsafe {
                let block = intrinsics::_mm256_loadu_si256(src.add(i).cast());
                intrinsics::_mm256_storeu_si256(dst.add(i).cast(), swap_32::<SIZE>(block));
            }
            i += BLOCK;
        }
    }

    /// Swaps `n` bytes, 16 at a time.
    ///
    /// # Safety
    /// `src` and `dst` must each be valid for `n` bytes, `n` a multiple of
    /// 16. They may be the same buffer.
    #[cfg(not(target_feature = "avx2"))]
    pub(super) unsafe fn swap_blocks<const SIZE: usize>(src: *const u8, dst: *mut u8, n: usize) {
        let mut i = 0;
        while i + BLOCK <= n {
            // SAFETY: the caller says both pointers are valid for `n` bytes,
            // and `i + BLOCK <= n`.
            unsafe {
                let block = intrinsics::_mm_loadu_si128(src.add(i).cast());
                intrinsics::_mm_storeu_si128(dst.add(i).cast(), swap_16::<SIZE>(block));
            }
            i += BLOCK;
        }
    }
}

/// The NEON path, which every aarch64 target has.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod neon {
    use core::arch::aarch64 as intrinsics;

    use super::BLOCK;

    /// Swaps one 16-byte block.
    #[inline]
    fn swap_16<const SIZE: usize>(block: intrinsics::uint8x16_t) -> intrinsics::uint8x16_t {
        // SAFETY: NEON is on for every aarch64 target.
        unsafe {
            match SIZE {
                2 => intrinsics::vrev16q_u8(block),
                4 => intrinsics::vrev32q_u8(block),
                8 => intrinsics::vrev64q_u8(block),
                _ => block,
            }
        }
    }

    /// Swaps `n` bytes, 16 at a time.
    ///
    /// # Safety
    /// `src` and `dst` must each be valid for `n` bytes, `n` a multiple of
    /// 16. They may be the same buffer.
    pub(super) unsafe fn swap_blocks<const SIZE: usize>(src: *const u8, dst: *mut u8, n: usize) {
        let mut i = 0;
        while i + BLOCK <= n {
            // SAFETY: the caller says both pointers are valid for `n` bytes,
            // and `i + BLOCK <= n`.
            unsafe {
                let block = intrinsics::vld1q_u8(src.add(i));
                intrinsics::vst1q_u8(dst.add(i), swap_16::<SIZE>(block));
            }
            i += BLOCK;
        }
    }
}

/// The wasm `simd128` path.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod wasm {
    use core::arch::wasm32 as intrinsics;

    use super::BLOCK;

    /// Swaps one 16-byte block.
    #[inline]
    fn swap_16<const SIZE: usize>(block: intrinsics::v128) -> intrinsics::v128 {
        match SIZE {
            2 => intrinsics::i8x16_shuffle::<1, 0, 3, 2, 5, 4, 7, 6, 9, 8, 11, 10, 13, 12, 15, 14>(
                block, block,
            ),
            4 => intrinsics::i8x16_shuffle::<3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12>(
                block, block,
            ),
            8 => intrinsics::i8x16_shuffle::<7, 6, 5, 4, 3, 2, 1, 0, 15, 14, 13, 12, 11, 10, 9, 8>(
                block, block,
            ),
            _ => block,
        }
    }

    /// Swaps `n` bytes, 16 at a time.
    ///
    /// # Safety
    /// `src` and `dst` must each be valid for `n` bytes, `n` a multiple of
    /// 16. They may be the same buffer.
    pub(super) unsafe fn swap_blocks<const SIZE: usize>(src: *const u8, dst: *mut u8, n: usize) {
        let mut i = 0;
        while i + BLOCK <= n {
            // SAFETY: the caller says both pointers are valid for `n` bytes,
            // and `i + BLOCK <= n`.
            unsafe {
                let block = intrinsics::v128_load(src.add(i).cast());
                intrinsics::v128_store(dst.add(i).cast(), swap_16::<SIZE>(block));
            }
            i += BLOCK;
        }
    }
}

/// The fallback when the target has no vector path.
mod scalar {
    /// Reverses each `SIZE`-byte element of `n` bytes.
    ///
    /// # Safety
    /// `src` and `dst` must each be valid for `n` bytes. They may be the
    /// same buffer: an element is read before it is written.
    pub(super) unsafe fn swap_copy<const SIZE: usize>(src: *const u8, dst: *mut u8, n: usize) {
        let mut i = 0;
        while i + SIZE <= n {
            let mut element = [0u8; SIZE];
            // SAFETY: the caller says both pointers are valid for `n` bytes,
            // and `i + SIZE <= n`.
            unsafe {
                core::ptr::copy_nonoverlapping(src.add(i), element.as_mut_ptr(), SIZE);
                element.reverse();
                core::ptr::copy_nonoverlapping(element.as_ptr(), dst.add(i), SIZE);
            }
            i += SIZE;
        }
    }
}
