//! Value generators biased towards edge cases.

use crate::rng::Pcg32;

/// A `u64` that is an "interesting" boundary value about a third of the time.
pub const fn u64_edgy(rng: &mut Pcg32) -> u64 {
    match rng.below(9) {
        0 => [0, 1, 127, 128, 16_383, 16_384, u64::MAX, u64::MAX - 1][rng.index(8)],
        1 => {
            let shift = rng.below(64);
            let v = 1u64 << shift;
            match rng.below(3) {
                0 => v,
                1 => v.wrapping_sub(1),
                _ => v.wrapping_add(1),
            }
        }
        2 => rng.below_u64(256),
        _ => rng.next_u64(),
    }
}

pub const fn i64_edgy(rng: &mut Pcg32) -> i64 {
    match rng.below(6) {
        0 => [0, 1, -1, i64::MAX, i64::MIN, -64, 63][rng.index(7)],
        _ => u64_edgy(rng).cast_signed(),
    }
}

/// Length in `0..=max`, biased towards short.
pub fn len(rng: &mut Pcg32, max: usize) -> usize {
    if rng.ratio(1, 4) {
        rng.index(max + 1)
    } else {
        rng.index(max.min(8) + 1)
    }
}

pub fn bytes(rng: &mut Pcg32, max_len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len(rng, max_len)];
    rng.fill_bytes(&mut out);
    out
}

/// A string mixing ASCII and multi-byte scalar values.
pub fn string(rng: &mut Pcg32, max_chars: usize) -> String {
    let n = len(rng, max_chars);
    (0..n)
        .map(|_| match rng.below(4) {
            0 => char::from_u32(rng.below(0xd7ff) + 1).unwrap_or('?'),
            1 => ['é', 'ß', '日', '🦀', '\0'][rng.index(5)],
            _ => char::from(b' ' + u8::try_from(rng.below(95)).unwrap_or(0)),
        })
        .collect()
}

/// A buffer that is either random bytes or a mutation of `valid`.
pub fn mutate(rng: &mut Pcg32, valid: &[u8]) -> Vec<u8> {
    let mut out = valid.to_vec();
    match rng.below(5) {
        0 if !out.is_empty() => {
            let i = rng.index(out.len());
            out[i] ^= 1 << rng.below(8);
        }
        1 if !out.is_empty() => {
            out.truncate(rng.index(out.len()));
        }
        2 => {
            let at = rng.index(out.len() + 1);
            out.insert(at, u8::try_from(rng.below(256)).unwrap_or(0));
        }
        3 if !out.is_empty() => {
            let i = rng.index(out.len());
            out[i] = [0x00, 0x7f, 0x80, 0xff][rng.index(4)];
        }
        _ => out = bytes(rng, 64),
    }
    out
}
