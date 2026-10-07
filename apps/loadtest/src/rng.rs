// SPDX-License-Identifier: GPL-3.0-or-later
//! A small deterministic pseudo-random generator (SplitMix64), so a run is reproducible from
//! its seed without another dependency. Not for anything secret.

/// SplitMix64 state.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A generator for `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x6A09_E667_F3BC_C909)
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform value in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A uniform value in [`lo`, `hi`) (`lo` when the range is empty or not finite).
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        if !lo.is_finite() || !hi.is_finite() || hi <= lo {
            return lo;
        }
        lo + (hi - lo) * self.unit()
    }

    /// A uniform integer in [0, `n`) (0 when `n` is 0).
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// Fills `buf` with random bytes.
    pub fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let v = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&v[..chunk.len()]);
        }
    }

    /// `n` random bytes.
    pub fn bytes(&mut self, n: usize) -> Vec<u8> {
        let mut v = vec![0u8; n];
        self.fill(&mut v);
        v
    }

    /// An independent generator derived from this one.
    pub fn fork(&mut self) -> Self {
        Self::new(self.next_u64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_in_range() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
            let u = a.unit();
            b.unit();
            assert!((0.0..1.0).contains(&u));
            let r = a.range(-3.0, 5.0);
            b.range(-3.0, 5.0);
            assert!((-3.0..5.0).contains(&r));
            assert!(a.below(10) < 10);
            b.below(10);
        }
        assert_eq!(a.below(0), 0);
        assert_eq!(a.range(2.0, 1.0), 2.0);
        assert!(a.range(f64::NAN, 1.0).is_nan());
        assert_eq!(a.bytes(13).len(), 13);
    }
}
