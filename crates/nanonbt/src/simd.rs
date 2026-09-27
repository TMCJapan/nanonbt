//! The vectorized byte-order decode behind the `simd` feature.
//!
//! NBT stores numbers big-endian, so decoding a whole array or list of them
//! is a byte reversal of every element. The compiler vectorizes a plain
//! `map(from_be_bytes)` loop on its own where it recognizes the shape, but
//! only in an optimized build; this module makes the reversal explicit with
//! `pulp`, which picks the widest vector the target has when it runs,
//! whatever the optimization level and whatever the target's baseline is.
//!
//! [`decode_be`] copies a payload into the `Vec` it becomes and swaps the
//! bytes there, so a payload is read with one allocation. The write side
//! stages a chunk of elements on the stack, swaps it there and hands it to
//! the writer, so writing one does not allocate either.
//!
//! Every vector path works on 32-bit lanes, the widest integral lane `pulp`
//! can shift by a runtime amount. Two- and four-byte elements reverse within
//! a lane; an eight-byte element is two lanes, reversed within each and then
//! exchanged with each other. A buffer that is not four-byte aligned is
//! staged through an aligned stack buffer, and elements outside the vector
//! blocks fall back to the scalar reversal.

use alloc::vec::Vec;

use pulp::{Arch, Simd, WithSimd};

use crate::{
    be::{as_bytes, as_bytes_mut},
    error::{Error, Result},
    read::Read,
    write::Write,
};

/// The bytes the write side stages on the stack at a time.
const CHUNK: usize = 256;

/// The most 32-bit lanes a vector register can hold, `pulp`'s register size
/// bound in lanes.
const MAX_LANES: usize = 64;

/// The even lanes, the ones an eight-byte element's halves leave in place,
/// as the lane-sized mask `select` takes.
const EVEN_LANES: [u32; MAX_LANES] = {
    let mut lanes = [0; MAX_LANES];
    let mut i = 0;
    while i < MAX_LANES {
        lanes[i] = if i.is_multiple_of(2) { u32::MAX } else { 0 };
        i += 1;
    }
    lanes
};

/// A stack buffer aligned for the 32-bit lane view.
#[repr(align(4))]
struct Stage([u8; CHUNK]);

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
    if bytes.is_empty() {
        return;
    }
    if bytes.as_ptr().addr().is_multiple_of(align_of::<u32>()) {
        swap_aligned::<SIZE>(bytes);
    } else {
        // Stage the bytes through a buffer the lane view can use. `CHUNK`
        // and so every chunk but the last is a whole number of elements,
        // and so is the length overall.
        let mut stage = Stage([0; CHUNK]);
        for part in bytes.chunks_mut(CHUNK) {
            debug_assert!(part.len().is_multiple_of(SIZE));
            let staged = &mut stage.0[..part.len()];
            staged.copy_from_slice(part);
            swap_aligned::<SIZE>(staged);
            part.copy_from_slice(staged);
        }
    }
}

/// [`swap_bytes_in_place`] where the buffer starts four-byte aligned, so its
/// whole 32-bit lanes can go to the vector loop.
#[allow(clippy::cast_ptr_alignment)] // the callers check the alignment the lane view needs
fn swap_aligned<const SIZE: usize>(bytes: &mut [u8]) {
    debug_assert!(bytes.as_ptr().cast::<u32>().is_aligned());
    debug_assert!(bytes.len().is_multiple_of(SIZE));
    // The whole lanes, and the less-than-four bytes beyond them, which are
    // one two-byte element and nothing else.
    let (lanes, tail) = bytes.split_at_mut(bytes.len() / 4 * 4);
    // SAFETY: the buffer starts four-byte aligned and the lanes are a whole
    // number of four-byte lanes, every byte of them initialized.
    let lanes = unsafe {
        core::slice::from_raw_parts_mut(lanes.as_mut_ptr().cast::<u32>(), lanes.len() / 4)
    };
    Arch::new().dispatch(SwapLanes::<SIZE> { lanes });
    tail.reverse();
}

/// The vector loop over the whole 32-bit lanes of a buffer and the scalar
/// reversal of what is left of them.
struct SwapLanes<'a, const SIZE: usize> {
    lanes: &'a mut [u32],
}

impl<const SIZE: usize> WithSimd for SwapLanes<'_, SIZE> {
    type Output = ();

    #[inline]
    fn with_simd<S: Simd>(self, simd: S) -> Self::Output {
        if S::IS_SCALAR {
            // A single lane has no other lane to exchange an eight-byte
            // element's halves with.
            swap_lanes::<SIZE>(self.lanes);
            return;
        }
        let (lanes, tail) = S::as_mut_simd_u32s(self.lanes);
        match SIZE {
            2 => swap_u16s_in(simd, lanes),
            4 => swap_u32s_in(simd, lanes),
            8 => swap_u64s_in(simd, lanes),
            _ => {}
        }
        // Every vector width is a whole number of elements, so what is left
        // holds whole elements, laid out the same way.
        swap_lanes::<SIZE>(tail);
    }
}

/// Reverses each 16-bit element of the whole lanes.
#[inline]
fn swap_u16s_in<S: Simd>(simd: S, lanes: &mut [S::u32s]) {
    let low = simd.splat_u32s(0x00FF_00FF);
    let high = simd.splat_u32s(0xFF00_FF00);
    let eight = simd.splat_u32s(8);
    for lane in lanes {
        *lane = simd.or_u32s(
            simd.and_u32s(simd.wrapping_dyn_shr_u32s(*lane, eight), low),
            simd.and_u32s(simd.wrapping_dyn_shl_u32s(*lane, eight), high),
        );
    }
}

/// Reverses each 32-bit element of the whole lanes.
#[inline]
fn swap_u32s_in<S: Simd>(simd: S, lanes: &mut [S::u32s]) {
    let eight = simd.splat_u32s(8);
    let sixteen = simd.splat_u32s(16);
    for lane in lanes {
        *lane = swap_u32_lane(simd, *lane, eight, sixteen);
    }
}

/// Reverses each 64-bit element of the whole lanes: both 32-bit halves swap
/// their bytes, then each pair of lanes swaps around.
#[inline]
fn swap_u64s_in<S: Simd>(simd: S, lanes: &mut [S::u32s]) {
    let even = simd.equal_u32s(
        simd.partial_load_u32s(&EVEN_LANES),
        simd.splat_u32s(u32::MAX),
    );
    let eight = simd.splat_u32s(8);
    let sixteen = simd.splat_u32s(16);
    for lane in lanes {
        let swapped = swap_u32_lane(simd, *lane, eight, sixteen);
        *lane = simd.select_u32s(
            even,
            simd.rotate_left_u32s(swapped, 1),
            simd.rotate_right_u32s(swapped, 1),
        );
    }
}

/// Reverses the four bytes of one 32-bit lane, a shift and a mask per
/// 16-bit half and one across them.
#[inline]
fn swap_u32_lane<S: Simd>(simd: S, lane: S::u32s, eight: S::u32s, sixteen: S::u32s) -> S::u32s {
    let low = simd.splat_u32s(0x00FF_00FF);
    let high = simd.splat_u32s(0xFF00_FF00);
    let halves = simd.or_u32s(
        simd.and_u32s(simd.wrapping_dyn_shr_u32s(lane, eight), low),
        simd.and_u32s(simd.wrapping_dyn_shl_u32s(lane, eight), high),
    );
    simd.or_u32s(
        simd.wrapping_dyn_shr_u32s(halves, sixteen),
        simd.wrapping_dyn_shl_u32s(halves, sixteen),
    )
}

/// Reverses the elements covered by whole lanes one lane at a time.
fn swap_lanes<const SIZE: usize>(lanes: &mut [u32]) {
    match SIZE {
        2 => {
            for lane in lanes {
                *lane = swap_u16s(*lane);
            }
        }
        4 => {
            for lane in lanes {
                *lane = lane.swap_bytes();
            }
        }
        8 => {
            // An eight-byte element is two lanes; the pair holds them in
            // the other order, each reversed.
            debug_assert!(lanes.len().is_multiple_of(2));
            for pair in lanes.as_chunks_mut::<2>().0 {
                pair.swap(0, 1);
                pair[0] = pair[0].swap_bytes();
                pair[1] = pair[1].swap_bytes();
            }
        }
        _ => {}
    }
}

/// Reverses the two bytes of each 16-bit half of a lane.
const fn swap_u16s(lane: u32) -> u32 {
    (lane & 0x00FF_00FF) << 8 | (lane & 0xFF00_FF00) >> 8
}

/// Decodes `SIZE`-byte big-endian elements into a `Vec`.
///
/// The bytes are copied into the `Vec`'s buffer and swapped there, so the
/// payload is read with one allocation and one pass over it per element. A
/// trailing partial element is ignored, the way `as_chunks` ignores one.
pub(crate) fn decode_be<T: Copy, const SIZE: usize>(bytes: &[u8]) -> Vec<T> {
    const { assert!(SIZE == size_of::<T>()) };
    let len = bytes.len() / SIZE;
    let mut out = Vec::with_capacity(len);
    let head = &bytes[..len * SIZE];
    if head.is_empty() {
        return out;
    }
    // SAFETY: the `Vec` has room for `len` elements, which is exactly the
    // bytes of `head`; every one of them is initialized, and `T` is a
    // scalar whose every bit pattern is valid.
    unsafe {
        core::ptr::copy_nonoverlapping(head.as_ptr(), out.as_mut_ptr().cast::<u8>(), head.len());
        out.set_len(len);
    }
    swap_bytes_in_place::<SIZE>(as_bytes_mut(&mut out));
    out
}

/// Reads `len` big-endian elements of `SIZE` bytes each: a list's payload,
/// whose header the caller has already read.
pub(crate) fn read_be_elements<'de, T: Copy, const SIZE: usize, R: Read<'de>>(
    len: usize,
    reader: &mut R,
) -> Result<Vec<T>> {
    let n = len.checked_mul(SIZE).ok_or_else(Error::array_too_large)?;
    let bytes = reader.read_bytes(n)?;
    // A reader that hands back fewer bytes than asked for has hit the end of
    // the input; the elements it did lend are not the whole list.
    if bytes.len() != n {
        return Err(Error::unexpected_eof());
    }
    Ok(decode_be::<T, SIZE>(&bytes))
}

/// Writes native-order `SIZE`-byte elements as big-endian, in as few writes
/// as the writer takes them.
pub(crate) fn write_be<T: Copy, const SIZE: usize, W: Write + ?Sized>(
    elements: &[T],
    writer: &mut W,
) -> Result<()> {
    const { assert!(SIZE == size_of::<T>()) };
    let bytes = as_bytes(elements);
    let mut stage = Stage([0; CHUNK]);
    for part in bytes.chunks(CHUNK) {
        debug_assert!(part.len().is_multiple_of(SIZE));
        let staged = &mut stage.0[..part.len()];
        staged.copy_from_slice(part);
        swap_bytes_in_place::<SIZE>(staged);
        writer.write_bytes(staged)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use pulp::{Scalar, WithSimd};

    use super::SwapLanes;

    /// The scalar backend, the one that runs where no vector is available,
    /// must reverse the elements exactly as the dispatched backends do.
    #[test]
    fn the_scalar_backend_reverses_every_element_width() {
        // Whole lanes of each width, covering both byte ends of every half.
        let words = [0x0000_0000u32, 0x0102_0304, 0xFFFF_FFFF, 0x00FF_FF00];
        scalar_swap_matches::<2>(words);
        scalar_swap_matches::<4>(words);
        scalar_swap_matches::<8>(words);
    }

    /// [`SwapLanes`] over `Scalar` in place, against reversing the bytes of
    /// each `SIZE`-byte element by hand.
    #[track_caller]
    fn scalar_swap_matches<const SIZE: usize>(words: [u32; 4]) {
        let mut native = [0u8; 16];
        for (lane, bytes) in words.iter().zip(native.as_chunks_mut::<4>().0) {
            bytes.copy_from_slice(&lane.to_ne_bytes());
        }
        let mut expected = [0u8; 16];
        for (from, to) in native
            .as_chunks::<SIZE>()
            .0
            .iter()
            .zip(expected.as_chunks_mut::<SIZE>().0)
        {
            for (to, from) in to.iter_mut().zip(from.iter().rev()) {
                *to = *from;
            }
        }

        let mut lanes = words;
        SwapLanes::<SIZE> { lanes: &mut lanes }.with_simd(Scalar);

        let mut got = [0u8; 16];
        for (lane, bytes) in lanes.iter().zip(got.as_chunks_mut::<4>().0) {
            bytes.copy_from_slice(&lane.to_ne_bytes());
        }
        assert_eq!(got, expected, "size {SIZE}");
    }
}
