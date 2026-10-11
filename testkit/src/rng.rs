//! Deterministic pseudo-random generator used by the test suites.
//!
//! Frozen by golden tests: changing its output would silently invalidate every
//! recorded failing seed, so do not "improve" it.
//!
//! Not cryptographically secure. Never use for keys, nonces or tokens.

/// The `SplitMix64` output finaliser, usable as a standalone 64-bit hash mix.
#[inline]
pub(crate) const fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// `SplitMix64` (Steele, Lea, Flood 2014). Expands a single seed into
/// well-mixed state for [`Pcg32`].
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix64(self.state)
    }
}

const PCG_MULT: u64 = 6_364_136_223_846_793_005;

/// PCG-XSH-RR 64/32 (O'Neill 2014), matching the reference `pcg32_random_r`.
#[derive(Clone, Debug)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    /// Equivalent to the reference `pcg32_srandom_r(init_state, init_seq)`.
    pub const fn new(init_state: u64, init_seq: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (init_seq << 1) | 1,
        };
        rng.step();
        rng.state = rng.state.wrapping_add(init_state);
        rng.step();
        rng
    }

    /// Expands a single seed into state and stream via `SplitMix64`.
    pub const fn seed_from_u64(seed: u64) -> Self {
        let mut sm = SplitMix64::new(seed);
        let state = sm.next_u64();
        let seq = sm.next_u64();
        Self::new(state, seq)
    }

    #[inline]
    const fn step(&mut self) {
        self.state = self.state.wrapping_mul(PCG_MULT).wrapping_add(self.inc);
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)] // truncation is the algorithm
    pub const fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.step();
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    #[inline]
    pub const fn next_u64(&mut self) -> u64 {
        let hi = self.next_u32() as u64;
        let lo = self.next_u32() as u64;
        (hi << 32) | lo
    }

    /// Uniform in `0..bound` (Lemire's nearly-divisionless method).
    /// Returns 0 when `bound == 0`.
    #[allow(clippy::cast_possible_truncation)] // taking the high/low halves of a u64 product
    pub const fn below(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        let mut m = (self.next_u32() as u64) * (bound as u64);
        let mut low = m as u32;
        if low < bound {
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                m = (self.next_u32() as u64) * (bound as u64);
                low = m as u32;
            }
        }
        (m >> 32) as u32
    }

    /// Uniform in `0..bound` for 64-bit bounds. Returns 0 when `bound == 0`.
    #[allow(clippy::cast_possible_truncation)] // taking the high/low halves of a u128 product
    pub const fn below_u64(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }
        let mut m = (self.next_u64() as u128) * (bound as u128);
        let mut low = m as u64;
        if low < bound {
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                m = (self.next_u64() as u128) * (bound as u128);
                low = m as u64;
            }
        }
        (m >> 64) as u64
    }

    /// Uniform index into a collection of length `len`. Returns 0 for `len == 0`.
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // result < len, which is a usize
    pub const fn index(&mut self, len: usize) -> usize {
        self.below_u64(len as u64) as usize
    }

    /// `true` with probability `num / den`.
    #[inline]
    pub const fn ratio(&mut self, num: u32, den: u32) -> bool {
        self.below(den) < num
    }

    #[inline]
    pub const fn bool(&mut self) -> bool {
        self.next_u32() & 1 == 1
    }

    pub fn fill_bytes(&mut self, out: &mut [u8]) {
        for chunk in out.chunks_mut(4) {
            let bytes = self.next_u32().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
    }

    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            items.get(self.index(items.len()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_golden() {
        let mut sm = SplitMix64::new(0);
        assert_eq!(sm.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(sm.next_u64(), 0x6e78_9e6a_a1b9_65f4);
        assert_eq!(sm.next_u64(), 0x06c4_5d18_8009_454f);
    }

    #[test]
    fn pcg32_reference_golden() {
        // Output of the reference pcg32-demo with pcg32_srandom_r(42, 54).
        let mut rng = Pcg32::new(42, 54);
        let expected = [
            0xa15c_02b7_u32,
            0x7b47_f409,
            0xba1d_3330,
            0x83d2_f293,
            0xbfa4_784b,
            0xcbed_606e,
        ];
        for e in expected {
            assert_eq!(rng.next_u32(), e);
        }
    }

    #[test]
    fn seed_from_u64_golden() {
        // Frozen: recorded seeds depend on this exact sequence.
        let mut rng = Pcg32::seed_from_u64(0x5eed);
        let got = [rng.next_u32(), rng.next_u32(), rng.next_u32()];
        let mut again = Pcg32::seed_from_u64(0x5eed);
        assert_eq!(got, [again.next_u32(), again.next_u32(), again.next_u32()]);
        assert_eq!(got, [2_851_957_292, 406_565_052, 3_973_736_297]);
    }
}
