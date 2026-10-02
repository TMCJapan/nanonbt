//! The vectorized byte-order decode behind the `simd` feature.
//!
//! NBT stores numbers big-endian, so decoding a whole array or list of them
//! is a byte reversal of every element. The compiler vectorizes a plain
//! `map(from_be_bytes)` loop on its own where it recognizes the shape, but
//! only in an optimized build; this module makes the reversal explicit,
//! whatever the optimization level and whatever the target's baseline is.
//!
//! [`decode_be`] reverses a payload straight into the `Vec` it becomes where
//! the payload and the `Vec` start on the same lane boundary, and copies the
//! payload in and swaps it there otherwise, so a payload is read with one
//! allocation. The write side stages a chunk of elements on the stack, swaps
//! it there and hands it to the writer, so writing one does not allocate
//! either.
//!
//! Every backend works on 32-bit lanes. Two- and four-byte elements reverse
//! within a lane; an eight-byte element is two lanes, reversed within each
//! and then exchanged with each other. A buffer that is not four-byte
//! aligned is staged through an aligned stack buffer, and elements outside
//! the whole lanes fall back to the scalar reversal.
//!
//! The aarch64 and wasm backends are their target's one-instruction
//! reversal — `vrev16`/`vrev32`/`vrev64` and `i8x16.shuffle` — reached
//! through `pulp`'s backend types. Everywhere else the reversal is written
//! once against [`pulp::Simd`], over the operations that trait offers, and
//! [`pulp::Arch::dispatch`] picks the widest vector the CPU has when it
//! runs. The aarch64 and wasm paths need no run-time choice: NEON is part of
//! the aarch64 baseline, and the wasm backend is compiled only where
//! `simd128` is enabled.

use alloc::vec::Vec;

use crate::{
    be::{as_bytes, as_bytes_mut},
    error::{Error, Result},
    read::Read,
    write::Write,
};

/// The bytes the write side stages on the stack at a time.
const CHUNK: usize = 256;

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
    backend::swap::<SIZE>(lanes);
    tail.reverse();
}

/// Reverses the elements covered by whole lanes one lane at a time.
///
/// This is what a backend leaves once its vector blocks are done, and all
/// that runs where the target has no vector path.
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

/// [`swap_lanes`] from `src` into `dst`.
fn swap_lanes_to<const SIZE: usize>(src: &[u32], dst: &mut [u32]) {
    match SIZE {
        2 => {
            for (dst, &lane) in dst.iter_mut().zip(src) {
                *dst = swap_u16s(lane);
            }
        }
        4 => {
            for (dst, &lane) in dst.iter_mut().zip(src) {
                *dst = lane.swap_bytes();
            }
        }
        8 => {
            // An eight-byte element is two lanes; the pair holds them in
            // the other order, each reversed.
            debug_assert!(src.len().is_multiple_of(2));
            let (src, _) = src.as_chunks::<2>();
            for (pair, &[first, second]) in dst.as_chunks_mut::<2>().0.iter_mut().zip(src) {
                *pair = [second.swap_bytes(), first.swap_bytes()];
            }
        }
        _ => {}
    }
}

/// The aarch64 backend: one `rev` instruction per vector block.
#[cfg(target_arch = "aarch64")]
mod backend {
    use pulp::core_arch::aarch64::Neon;

    use super::{swap_lanes, swap_lanes_to};

    /// The 32-bit lanes one NEON register holds.
    const LANES: usize = 4;

    /// Reverses each whole 16-byte block with the `rev` for the element
    /// width, then the lanes the blocks do not cover with the scalar loop.
    pub(super) fn swap<const SIZE: usize>(lanes: &mut [u32]) {
        // SAFETY: NEON is part of the aarch64 baseline, so the target always
        // has it.
        let neon = unsafe { Neon::new_unchecked() };
        let (whole, tail) = lanes.split_at_mut(lanes.len() / LANES * LANES);
        for chunk in whole.as_chunks_mut::<LANES>().0 {
            // SAFETY: a chunk is one 16-byte NEON register, which
            // `vld1q_u8` and `vst1q_u8` read and write exactly.
            unsafe {
                let ptr = chunk.as_mut_ptr().cast::<u8>();
                let block = neon.vld1q_u8(ptr);
                let swapped = match SIZE {
                    2 => neon.vrev16q_u8(block),
                    4 => neon.vrev32q_u8(block),
                    8 => neon.vrev64q_u8(block),
                    _ => block,
                };
                neon.vst1q_u8(ptr, swapped);
            }
        }
        swap_lanes::<SIZE>(tail);
    }

    /// [`swap`] reading `src` and writing `dst`, block for block.
    pub(super) fn copy<const SIZE: usize>(src: &[u32], dst: &mut [u32]) {
        debug_assert_eq!(src.len(), dst.len());
        // SAFETY: as in [`swap`].
        let neon = unsafe { Neon::new_unchecked() };
        let whole = src.len() / LANES * LANES;
        let (src, src_tail) = src.split_at(whole);
        let (dst, dst_tail) = dst.split_at_mut(whole);
        for (src, dst) in src
            .as_chunks::<LANES>()
            .0
            .iter()
            .zip(dst.as_chunks_mut::<LANES>().0.iter_mut())
        {
            // SAFETY: each pair is one 16-byte NEON register on either
            // side, which the load and store read and write exactly.
            unsafe {
                let block = neon.vld1q_u8(src.as_ptr().cast::<u8>());
                let swapped = match SIZE {
                    2 => neon.vrev16q_u8(block),
                    4 => neon.vrev32q_u8(block),
                    8 => neon.vrev64q_u8(block),
                    _ => block,
                };
                neon.vst1q_u8(dst.as_mut_ptr().cast::<u8>(), swapped);
            }
        }
        swap_lanes_to::<SIZE>(src_tail, dst_tail);
    }
}

/// The wasm `simd128` backend: one byte shuffle per vector block.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod backend {
    use pulp::core_arch::wasm::Simd128;

    use super::{swap_lanes, swap_lanes_to};

    /// The 32-bit lanes one `v128` holds.
    const LANES: usize = 4;

    /// Reverses each whole 16-byte block with the `shuffle` for the element
    /// width, then the lanes the blocks do not cover with the scalar loop.
    pub(super) fn swap<const SIZE: usize>(lanes: &mut [u32]) {
        let simd = Simd128::new_unchecked();
        let (whole, tail) = lanes.split_at_mut(lanes.len() / LANES * LANES);
        for chunk in whole.as_chunks_mut::<LANES>().0 {
            // SAFETY: a chunk is one `v128`, which `v128_load` and
            // `v128_store` read and write exactly.
            unsafe {
                let ptr = chunk.as_mut_ptr().cast();
                let block = simd.v128_load(ptr);
                let swapped = match SIZE {
                    2 => simd
                        .i8x16_shuffle::<1, 0, 3, 2, 5, 4, 7, 6, 9, 8, 11, 10, 13, 12, 15, 14>(
                            block, block,
                        ),
                    4 => simd
                        .i8x16_shuffle::<3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12>(
                            block, block,
                        ),
                    8 => simd
                        .i8x16_shuffle::<7, 6, 5, 4, 3, 2, 1, 0, 15, 14, 13, 12, 11, 10, 9, 8>(
                            block, block,
                        ),
                    _ => block,
                };
                simd.v128_store(ptr, swapped);
            }
        }
        swap_lanes::<SIZE>(tail);
    }

    /// [`swap`] reading `src` and writing `dst`, block for block.
    pub(super) fn copy<const SIZE: usize>(src: &[u32], dst: &mut [u32]) {
        debug_assert_eq!(src.len(), dst.len());
        let simd = Simd128::new_unchecked();
        let whole = src.len() / LANES * LANES;
        let (src, src_tail) = src.split_at(whole);
        let (dst, dst_tail) = dst.split_at_mut(whole);
        for (src, dst) in src
            .as_chunks::<LANES>()
            .0
            .iter()
            .zip(dst.as_chunks_mut::<LANES>().0.iter_mut())
        {
            // SAFETY: each pair is one `v128` on either side, which the
            // load and store read and write exactly.
            unsafe {
                let block = simd.v128_load(src.as_ptr().cast());
                let swapped = match SIZE {
                    2 => simd
                        .i8x16_shuffle::<1, 0, 3, 2, 5, 4, 7, 6, 9, 8, 11, 10, 13, 12, 15, 14>(
                            block, block,
                        ),
                    4 => simd
                        .i8x16_shuffle::<3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12>(
                            block, block,
                        ),
                    8 => simd
                        .i8x16_shuffle::<7, 6, 5, 4, 3, 2, 1, 0, 15, 14, 13, 12, 11, 10, 9, 8>(
                            block, block,
                        ),
                    _ => block,
                };
                simd.v128_store(dst.as_mut_ptr().cast(), swapped);
            }
        }
        swap_lanes_to::<SIZE>(src_tail, dst_tail);
    }
}

/// The portable backend: `pulp`'s generic operations, dispatched at run
/// time to the widest vector the target has.
#[cfg(not(any(
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
mod backend {
    use pulp::{Arch, Simd, WithSimd};

    use super::{swap_lanes, swap_lanes_to};

    /// The most 32-bit lanes a vector register can hold, `pulp`'s register
    /// size bound in lanes.
    const MAX_LANES: usize = 64;

    /// The even lanes, the ones an eight-byte element's halves leave in
    /// place, as the lane-sized mask `select` takes.
    const EVEN_LANES: [u32; MAX_LANES] = {
        let mut lanes = [0; MAX_LANES];
        let mut i = 0;
        while i < MAX_LANES {
            lanes[i] = if i.is_multiple_of(2) { u32::MAX } else { 0 };
            i += 1;
        }
        lanes
    };

    /// Reverses the whole lanes with the widest vector the target has.
    pub(super) fn swap<const SIZE: usize>(lanes: &mut [u32]) {
        Arch::new().dispatch(SwapLanes::<SIZE> { lanes });
    }

    /// [`swap`] from `src` into `dst`, lane for lane.
    pub(super) fn copy<const SIZE: usize>(src: &[u32], dst: &mut [u32]) {
        debug_assert_eq!(src.len(), dst.len());
        Arch::new().dispatch(SwapCopy::<SIZE> { src, dst });
    }

    /// The vector loop over the whole 32-bit lanes of a buffer and the
    /// scalar reversal of what is left of them.
    pub(super) struct SwapLanes<'a, const SIZE: usize> {
        pub(super) lanes: &'a mut [u32],
    }

    impl<const SIZE: usize> WithSimd for SwapLanes<'_, SIZE> {
        type Output = ();

        #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
        #[inline(always)]
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
            // Every vector width is a whole number of elements, so what is
            // left holds whole elements, laid out the same way.
            swap_lanes::<SIZE>(tail);
        }
    }

    /// [`SwapLanes`]'s counterpart when the reversal lands in another buffer:
    /// each whole lane of `src` is read once and stored swapped to the lane of
    /// `dst` with the same index.
    pub(super) struct SwapCopy<'a, 'b, const SIZE: usize> {
        pub(super) src: &'a [u32],
        pub(super) dst: &'b mut [u32],
    }

    impl<const SIZE: usize> WithSimd for SwapCopy<'_, '_, SIZE> {
        type Output = ();

        #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
        #[inline(always)]
        fn with_simd<S: Simd>(self, simd: S) -> Self::Output {
            if S::IS_SCALAR {
                // A single lane has no other lane to exchange an eight-byte
                // element's halves with.
                swap_lanes_to::<SIZE>(self.src, self.dst);
                return;
            }
            let (src, src_tail) = S::as_simd_u32s(self.src);
            let (dst, dst_tail) = S::as_mut_simd_u32s(self.dst);
            debug_assert_eq!(src.len(), dst.len());
            match SIZE {
                2 => copy_u16s_in(simd, src, dst),
                4 => copy_u32s_in(simd, src, dst),
                8 => copy_u64s_in(simd, src, dst),
                _ => {}
            }
            // Every vector width is a whole number of elements, so what is
            // left holds whole elements, laid out the same way.
            swap_lanes_to::<SIZE>(src_tail, dst_tail);
        }
    }

    /// Reverses each 16-bit element of the whole lanes, in place.
    #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
    #[inline(always)]
    fn swap_u16s_in<S: Simd>(simd: S, lanes: &mut [S::u32s]) {
        for lane in lanes {
            *lane = swap_u16_lane(simd, *lane);
        }
    }

    /// [`swap_u16s_in`] reading `src` and writing `dst`, lane for lane.
    #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
    #[inline(always)]
    fn copy_u16s_in<S: Simd>(simd: S, src: &[S::u32s], dst: &mut [S::u32s]) {
        for (dst, &lane) in dst.iter_mut().zip(src) {
            *dst = swap_u16_lane(simd, lane);
        }
    }

    /// Reverses each 32-bit element of the whole lanes, in place.
    #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
    #[inline(always)]
    fn swap_u32s_in<S: Simd>(simd: S, lanes: &mut [S::u32s]) {
        let eight = simd.splat_u32s(8);
        let sixteen = simd.splat_u32s(16);
        for lane in lanes {
            *lane = swap_u32_lane(simd, *lane, eight, sixteen);
        }
    }

    /// [`swap_u32s_in`] reading `src` and writing `dst`, lane for lane.
    #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
    #[inline(always)]
    fn copy_u32s_in<S: Simd>(simd: S, src: &[S::u32s], dst: &mut [S::u32s]) {
        let eight = simd.splat_u32s(8);
        let sixteen = simd.splat_u32s(16);
        for (dst, &lane) in dst.iter_mut().zip(src) {
            *dst = swap_u32_lane(simd, lane, eight, sixteen);
        }
    }

    /// Reverses each 64-bit element of the whole lanes, in place: both 32-bit
    /// halves swap their bytes, then each pair of lanes swaps around.
    #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
    #[inline(always)]
    fn swap_u64s_in<S: Simd>(simd: S, lanes: &mut [S::u32s]) {
        let even = simd.equal_u32s(
            simd.partial_load_u32s(&EVEN_LANES),
            simd.splat_u32s(u32::MAX),
        );
        let eight = simd.splat_u32s(8);
        let sixteen = simd.splat_u32s(16);
        for lane in lanes {
            *lane = swap_u64_lane(simd, *lane, even, eight, sixteen);
        }
    }

    /// [`swap_u64s_in`] reading `src` and writing `dst`, lane for lane.
    #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
    #[inline(always)]
    fn copy_u64s_in<S: Simd>(simd: S, src: &[S::u32s], dst: &mut [S::u32s]) {
        let even = simd.equal_u32s(
            simd.partial_load_u32s(&EVEN_LANES),
            simd.splat_u32s(u32::MAX),
        );
        let eight = simd.splat_u32s(8);
        let sixteen = simd.splat_u32s(16);
        for (dst, &lane) in dst.iter_mut().zip(src) {
            *dst = swap_u64_lane(simd, lane, even, eight, sixteen);
        }
    }

    /// Reverses the two bytes of each 16-bit half of a lane.
    #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
    #[inline(always)]
    fn swap_u16_lane<S: Simd>(simd: S, lane: S::u32s) -> S::u32s {
        let low = simd.splat_u32s(0x00FF_00FF);
        let high = simd.splat_u32s(0xFF00_FF00);
        let eight = simd.splat_u32s(8);
        simd.or_u32s(
            simd.and_u32s(simd.wrapping_dyn_shr_u32s(lane, eight), low),
            simd.and_u32s(simd.wrapping_dyn_shl_u32s(lane, eight), high),
        )
    }

    /// Reverses the bytes of one lane of a 64-bit element's pair: the lane's
    /// own bytes, then the pair's lanes exchange around.
    #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
    #[inline(always)]
    fn swap_u64_lane<S: Simd>(
        simd: S,
        lane: S::u32s,
        even: S::m32s,
        eight: S::u32s,
        sixteen: S::u32s,
    ) -> S::u32s {
        let swapped = swap_u32_lane(simd, lane, eight, sixteen);
        simd.select_u32s(
            even,
            simd.rotate_left_u32s(swapped, 1),
            simd.rotate_right_u32s(swapped, 1),
        )
    }

    /// Reverses the four bytes of one 32-bit lane, a shift and a mask per
    /// 16-bit half and one across them.
    #[allow(clippy::inline_always)] // pulp only vectorizes the call when this function is inlined
    #[inline(always)]
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
}

/// Copies `n` bytes from `src` to `dst`, a fixed block at a time.
///
/// A `copy_nonoverlapping` of a runtime length reaches the libc `memcpy`,
/// which for medium sizes picks `rep movsb` on some x86 targets; that runs
/// one instruction per byte under callgrind and shows up as a whole extra
/// pass over the payload. A loop of fixed-size unaligned reads and writes
/// moves the same bytes with a few instructions per block instead.
///
/// `n` must not exceed `isize::MAX`. A block of 64 bytes is four of the
/// widest moves the x86-64 baseline has, and the compiler emits them inline;
/// the remainder, under one block, still goes through `copy_nonoverlapping`,
/// whose `memcpy` call is exact-sized there.
///
/// Only the targets where the libc `memcpy` is the problem use this: wasm
/// lowers `copy_nonoverlapping` to a bulk `memory.copy`, one instruction
/// for the whole run, which no block loop can beat.
///
/// # Safety
/// `src` must be valid for `n` reads and `dst` for `n` writes; the two must
/// not overlap.
#[cfg(not(target_family = "wasm"))]
const unsafe fn copy_blocks(src: *const u8, dst: *mut u8, n: usize) {
    const BLOCK: usize = 64;
    let mut i = 0;
    while i + BLOCK <= n {
        // SAFETY: the caller promises both pointers are valid up to `n`, and
        // `i + BLOCK <= n`.
        unsafe {
            let block = src.add(i).cast::<[u8; BLOCK]>().read_unaligned();
            dst.add(i).cast::<[u8; BLOCK]>().write_unaligned(block);
        }
        i += BLOCK;
    }
    // SAFETY: the rest of the range is inside the `n` bytes the caller
    // promises, and the two buffers do not overlap.
    unsafe { core::ptr::copy_nonoverlapping(src.add(i), dst.add(i), n - i) };
}

/// Decodes `SIZE`-byte big-endian elements into a `Vec`.
///
/// Where the payload and the `Vec`'s buffer start on the same four-byte lane
/// boundary, every lane is read once and stored reversed, so the payload is
/// read with one allocation. Otherwise the bytes are copied into the `Vec`
/// and swapped there. A trailing partial element is ignored, the way
/// `as_chunks` ignores one.
#[allow(clippy::cast_ptr_alignment)] // the checks above pin the alignment the lane view needs
pub(crate) fn decode_be<T: Copy, const SIZE: usize>(bytes: &[u8]) -> Vec<T> {
    const { assert!(SIZE == size_of::<T>()) };
    let len = bytes.len() / SIZE;
    let mut out = Vec::with_capacity(len);
    let head = &bytes[..len * SIZE];
    if head.is_empty() {
        return out;
    }
    if !cfg!(target_endian = "big")
        && head.as_ptr().addr().is_multiple_of(align_of::<u32>())
        && out.as_ptr().addr().is_multiple_of(align_of::<u32>())
    {
        // The whole lanes, and the less-than-four bytes beyond them, which
        // are one two-byte element and nothing else.
        let lanes = head.len() / 4;
        // SAFETY: both buffers start four-byte aligned, `lanes` is
        // `head.len()/4`, and the `Vec` has room for all of `head`'s bytes;
        // `backend::copy` fills the whole lanes and the copy fills the rest,
        // after which every element of `out` is initialized.
        unsafe {
            backend::copy::<SIZE>(
                core::slice::from_raw_parts(head.as_ptr().cast::<u32>(), lanes),
                core::slice::from_raw_parts_mut(out.as_mut_ptr().cast::<u32>(), lanes),
            );
            core::ptr::copy_nonoverlapping(
                head.as_ptr().add(lanes * 4),
                out.as_mut_ptr().cast::<u8>().add(lanes * 4),
                head.len() - lanes * 4,
            );
            out.set_len(len);
        }
        // The bytes after the whole lanes hold one two-byte element, whose
        // bytes still swap.
        for element in as_bytes_mut(&mut out)[lanes * 4..]
            .as_chunks_mut::<SIZE>()
            .0
        {
            element.reverse();
        }
        return out;
    }
    // SAFETY: the `Vec` has room for `len` elements, which is exactly the
    // bytes of `head`; the copy initializes every one of them, and `T`
    // is a scalar whose every bit pattern is valid.
    unsafe {
        #[cfg(target_family = "wasm")]
        core::ptr::copy_nonoverlapping(head.as_ptr(), out.as_mut_ptr().cast::<u8>(), head.len());
        #[cfg(not(target_family = "wasm"))]
        copy_blocks(head.as_ptr(), out.as_mut_ptr().cast::<u8>(), head.len());
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
        // The stage is aligned, and never empty, so it can go straight to
        // the lane loop without the checks a caller-owned buffer needs.
        swap_aligned::<SIZE>(staged);
        writer.write_bytes(staged)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    use pulp::{Scalar, WithSimd};

    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    use super::backend::{SwapCopy, SwapLanes};
    use super::{decode_be, swap_lanes, swap_lanes_to};
    use crate::be::as_bytes;

    /// The portable backend's scalar path, the one that runs where no vector
    /// is available, must reverse the elements exactly as the vector paths
    /// do, in place and into another buffer.
    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    #[test]
    fn the_scalar_backend_reverses_every_element_width() {
        // Whole lanes of each width, covering both byte ends of every half.
        let words = [0x0000_0000u32, 0x0102_0304, 0xFFFF_FFFF, 0x00FF_FF00];
        scalar_swap_matches::<2>(words);
        scalar_swap_matches::<4>(words);
        scalar_swap_matches::<8>(words);
        scalar_copy_matches::<2>(words);
        scalar_copy_matches::<4>(words);
        scalar_copy_matches::<8>(words);
    }

    /// The scalar reversal every backend leaves for its tail must reverse
    /// each element width the same way, wherever it runs.
    #[test]
    fn the_scalar_tail_reverses_every_element_width() {
        let words = [0x0000_0000u32, 0x0102_0304, 0xFFFF_FFFF, 0x00FF_FF00];
        tail_swap_matches::<2>(words);
        tail_swap_matches::<4>(words);
        tail_swap_matches::<8>(words);
        tail_copy_matches::<2>(words);
        tail_copy_matches::<4>(words);
        tail_copy_matches::<8>(words);
    }

    /// [`SwapLanes`] over `Scalar` in place, against reversing the bytes of
    /// each `SIZE`-byte element by hand.
    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    #[track_caller]
    fn scalar_swap_matches<const SIZE: usize>(words: [u32; 4]) {
        let mut lanes = words;
        SwapLanes::<SIZE> { lanes: &mut lanes }.with_simd(Scalar);
        assert_eq!(
            lane_bytes(lanes),
            reversed_bytes::<SIZE>(words),
            "size {SIZE}"
        );
    }

    /// [`SwapCopy`] over `Scalar` into another buffer, against the same.
    #[cfg(not(any(
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    #[track_caller]
    fn scalar_copy_matches<const SIZE: usize>(words: [u32; 4]) {
        let mut got = [0u32; 4];
        SwapCopy::<SIZE> {
            src: &words,
            dst: &mut got,
        }
        .with_simd(Scalar);
        assert_eq!(
            lane_bytes(got),
            reversed_bytes::<SIZE>(words),
            "size {SIZE}"
        );
    }

    /// [`swap_lanes`] in place against reversing each element's bytes.
    #[track_caller]
    fn tail_swap_matches<const SIZE: usize>(words: [u32; 4]) {
        let mut lanes = words;
        swap_lanes::<SIZE>(&mut lanes);
        assert_eq!(
            lane_bytes(lanes),
            reversed_bytes::<SIZE>(words),
            "size {SIZE}"
        );
    }

    /// [`swap_lanes_to`] into another buffer against the same.
    #[track_caller]
    fn tail_copy_matches<const SIZE: usize>(words: [u32; 4]) {
        let mut got = [0u32; 4];
        swap_lanes_to::<SIZE>(&words, &mut got);
        assert_eq!(
            lane_bytes(got),
            reversed_bytes::<SIZE>(words),
            "size {SIZE}"
        );
    }

    /// The bytes of four lanes.
    fn lane_bytes(lanes: [u32; 4]) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        for (lane, chunk) in lanes.iter().zip(bytes.as_chunks_mut::<4>().0) {
            chunk.copy_from_slice(&lane.to_ne_bytes());
        }
        bytes
    }

    /// The bytes of `words` with each `SIZE`-byte element reversed.
    fn reversed_bytes<const SIZE: usize>(words: [u32; 4]) -> [u8; 16] {
        let mut reversed = [0u8; 16];
        for (from, to) in lane_bytes(words)
            .as_chunks::<SIZE>()
            .0
            .iter()
            .zip(reversed.as_chunks_mut::<SIZE>().0)
        {
            for (to, from) in to.iter_mut().zip(from.iter().rev()) {
                *to = *from;
            }
        }
        reversed
    }

    /// [`decode_be`] reverses each element the way `from_be_bytes` would,
    /// through the lane copy where the payload is lane aligned and the staged
    /// path where it is not, and leaves a trailing partial element out.
    #[test]
    fn decode_reverses_at_every_alignment() {
        for offset in 0..4usize {
            for len in 0..20usize {
                let mut buffer = vec![0u8; offset];
                // Three bytes past the elements, a partial one to ignore.
                buffer.extend((0..len * 8 + 3).map(|i| u8::try_from(i % 251).expect("below 251")));
                let payload = &buffer[offset..];
                decoded_bytes_match::<u16, 2>(payload);
                decoded_bytes_match::<u32, 4>(payload);
                decoded_bytes_match::<u64, 8>(payload);
            }
        }
    }

    /// The bytes of a decoded payload against the payload's own bytes, each
    /// `SIZE`-byte element reversed.
    #[track_caller]
    fn decoded_bytes_match<T: Copy, const SIZE: usize>(payload: &[u8]) {
        let got = decode_be::<T, SIZE>(payload);
        let got = as_bytes(&got);
        assert_eq!(got.len(), payload.len() / SIZE * SIZE);
        for (from, to) in payload
            .as_chunks::<SIZE>()
            .0
            .iter()
            .zip(got.as_chunks::<SIZE>().0)
        {
            let mut reversed = *from;
            reversed.reverse();
            assert_eq!(*to, reversed);
        }
    }
}
