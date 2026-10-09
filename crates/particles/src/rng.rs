// SPDX-License-Identifier: GPL-3.0-or-later
//! The random source of the particle runtime (`particle-runtime.md` §3.3, §3.4).
//!
//! [`Ran2`] is L'Ecuyer's combined multiplicative congruential generator with a 32-entry
//! Bays–Durham shuffle table (the published "ran2" algorithm), with the constants of §3.3.
//! The original client shares one such stream between all of its code, so its sequences
//! cannot be reproduced; Tessera keeps the algorithm and lets tests and tools seed a stream
//! of their own (§12 rule 1).

use std::time::{SystemTime, UNIX_EPOCH};

/// Modulus 1 (§3.3).
const M1: i64 = 2_147_483_563;
/// Multiplier 1.
const A1: i64 = 40_014;
/// Schrage quotient of modulus 1.
const Q1: i64 = 53_668;
/// Schrage remainder of modulus 1.
const R1: i64 = 12_211;
/// Modulus 2.
const M2: i64 = 2_147_483_399;
/// Multiplier 2.
const A2: i64 = 40_692;
/// Schrage quotient of modulus 2.
const Q2: i64 = 52_774;
/// Schrage remainder of modulus 2.
const R2: i64 = 3_791;
/// Entries of the shuffle table.
const TABLE: usize = 32;
/// Divisor that turns the previous output into a table index.
const DIV: i64 = 67_108_862;
/// Largest value a draw returns: 1 − 1.2 × 10⁻⁷.
const MAX_DRAW: f64 = 1.0 - 1.2e-7;

/// One step `s ← a · s mod m` with Schrage's decomposition; a negative intermediate result
/// is made positive by adding the modulus (§3.3).
fn schrage(s: i64, a: i64, q: i64, r: i64, m: i64) -> i64 {
    let k = s / q;
    let v = a * (s - k * q) - k * r;
    if v < 0 { v + m } else { v }
}

/// The particle random generator (`particle-runtime.md` §3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran2 {
    s1: i64,
    s2: i64,
    table: [i64; TABLE],
    y: i64,
}

impl Ran2 {
    /// A generator seeded with `seed` (§3.3 "Seeding with *n*"; 0 counts as 1).
    #[must_use]
    pub fn new(seed: u32) -> Self {
        let mut g = Self {
            s1: 1,
            s2: 1,
            table: [0; TABLE],
            y: 0,
        };
        g.reseed(seed);
        g
    }

    /// A generator seeded as the client seeds its own at start-up: with the wall-clock
    /// time in whole seconds (§3.3 "When"). The value is truncated to 32 bits.
    #[must_use]
    pub fn from_wall_clock() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_secs());
        Self::new(secs as u32)
    }

    /// Seeds the generator with `n` (§3.3): both states become `n` (1 when `n` is 0), the
    /// first state is advanced forty times filling the table from entry 39 down to entry
    /// 0 (entries below 32 only), and the previous output becomes table entry 0.
    pub fn reseed(&mut self, n: u32) {
        let n = if n == 0 { 1 } else { i64::from(n) };
        self.s1 = n;
        self.s2 = n;
        for k in (0..TABLE + 8).rev() {
            self.s1 = schrage(self.s1, A1, Q1, R1, M1);
            if let Some(slot) = self.table.get_mut(k) {
                *slot = self.s1;
            }
        }
        self.y = self.table[0];
    }

    /// One draw in (0, 1) (§3.3 "One draw").
    pub fn next_f64(&mut self) -> f64 {
        self.s1 = schrage(self.s1, A1, Q1, R1, M1);
        self.s2 = schrage(self.s2, A2, Q2, R2, M2);
        // The previous output lies in [1, M1 − 1], so the index is below 32; `rem_euclid`
        // keeps it in range for any state a caller could construct.
        let j = (self.y / DIV).rem_euclid(TABLE as i64) as usize;
        let entry = self.table[j];
        self.y = entry - self.s2;
        self.table[j] = self.s1;
        if self.y < 1 {
            self.y += M1 - 1;
        }
        (self.y as f64 / M1 as f64).min(MAX_DRAW)
    }

    /// A real-range draw (§3.4): `a` without drawing when `a` = `b`; otherwise
    /// lo + *u* × (hi − lo) for one draw *u*, in double precision rounded to single.
    /// Non-finite bounds give a non-finite result (callers sanitise).
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        if a == b {
            return a;
        }
        let (lo, hi) = (f64::from(a.min(b)), f64::from(a.max(b)));
        let u = self.next_f64();
        (lo + u * (hi - lo)) as f32
    }

    /// An integer-range draw (§3.4): `a` without drawing when `a` = `b`; otherwise lo + the
    /// integer part of *u* × (hi − lo + 1).
    pub fn range_int(&mut self, a: i64, b: i64) -> i64 {
        if a == b {
            return a;
        }
        let (lo, hi) = (a.min(b), a.max(b));
        let span = (hi as f64) - (lo as f64) + 1.0;
        let u = self.next_f64();
        let k = (u * span).trunc();
        // u < 1, so k ≤ hi − lo; the clamp only guards rounding of huge spans.
        lo.saturating_add(k as i64).min(hi)
    }

    /// An index drawn uniformly from 0 to `count` − 1 (one integer-range draw); 0 without a
    /// draw when `count` ≤ 1.
    pub fn index(&mut self, count: usize) -> usize {
        if count <= 1 {
            return 0;
        }
        let hi = i64::try_from(count - 1).unwrap_or(i64::MAX);
        usize::try_from(self.range_int(0, hi)).unwrap_or(0)
    }
}

impl Default for Ran2 {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An independent rendering of the published algorithm ("ran2", initialised with a
    /// negative seed), written as the book states it, for invariant RT-I3.
    struct Published {
        idum: i64,
        idum2: i64,
        iy: i64,
        iv: [i64; 32],
    }

    impl Published {
        fn new(seed: i64) -> Self {
            let mut p = Self {
                idum: -seed,
                idum2: 123_456_789,
                iy: 0,
                iv: [0; 32],
            };
            p.init();
            p
        }

        fn init(&mut self) {
            if -self.idum < 1 {
                self.idum = 1;
            } else {
                self.idum = -self.idum;
            }
            self.idum2 = self.idum;
            for j in (0..=39i64).rev() {
                let k = self.idum / 53_668;
                self.idum = 40_014 * (self.idum - k * 53_668) - k * 12_211;
                if self.idum < 0 {
                    self.idum += 2_147_483_563;
                }
                if j < 32 {
                    self.iv[j as usize] = self.idum;
                }
            }
            self.iy = self.iv[0];
        }

        fn next(&mut self) -> f64 {
            let k = self.idum / 53_668;
            self.idum = 40_014 * (self.idum - k * 53_668) - k * 12_211;
            if self.idum < 0 {
                self.idum += 2_147_483_563;
            }
            let k = self.idum2 / 52_774;
            self.idum2 = 40_692 * (self.idum2 - k * 52_774) - k * 3_791;
            if self.idum2 < 0 {
                self.idum2 += 2_147_483_399;
            }
            let j = (self.iy / (1 + 2_147_483_562 / 32)) as usize;
            self.iy = self.iv[j] - self.idum2;
            self.iv[j] = self.idum;
            if self.iy < 1 {
                self.iy += 2_147_483_562;
            }
            let temp = self.iy as f64 * (1.0 / 2_147_483_563.0);
            temp.min(1.0 - 1.2e-7)
        }
    }

    /// RT-I3: the generator seeded with *n* > 0 matches the published algorithm
    /// initialised with −*n* over the first 10,000 draws.
    #[test]
    fn matches_the_published_algorithm() {
        for seed in [1u32, 7, 12_345, 1_700_000_000, 2_000_000_000] {
            let mut ours = Ran2::new(seed);
            let mut theirs = Published::new(i64::from(seed));
            for i in 0..10_000 {
                let (a, b) = (ours.next_f64(), theirs.next());
                assert!((a - b).abs() <= 1e-15, "seed {seed} draw {i}: {a} vs {b}");
            }
        }
    }

    #[test]
    fn draws_lie_in_the_open_unit_interval() {
        let mut g = Ran2::new(99);
        let mut sum = 0.0;
        for _ in 0..100_000 {
            let u = g.next_f64();
            assert!(u > 0.0 && u < 1.0);
            sum += u;
        }
        let mean = sum / 100_000.0;
        assert!((mean - 0.5).abs() < 0.01, "{mean}");
        // Seed 0 counts as 1.
        assert_eq!(Ran2::new(0), Ran2::new(1));
    }

    /// RT-I4: a real-range draw with a = b consumes no number of the stream.
    #[test]
    fn equal_bounds_draw_nothing() {
        let mut a = Ran2::new(5);
        let mut b = Ran2::new(5);
        assert_eq!(a.range(2.5, 2.5), 2.5);
        assert_eq!(a.range_int(3, 3), 3);
        assert_eq!(a.index(1), 0);
        assert_eq!(a.next_f64(), b.next_f64());
        // Bounds in either order give the same draw.
        let mut c = Ran2::new(5);
        let mut d = Ran2::new(5);
        assert_eq!(c.range(1.0, 3.0), d.range(3.0, 1.0));
    }

    #[test]
    fn integer_ranges_cover_their_bounds() {
        let mut g = Ran2::new(11);
        let mut seen = [0u32; 4];
        for _ in 0..4_000 {
            let k = g.range_int(0, 3);
            assert!((0..=3).contains(&k));
            seen[k as usize] += 1;
        }
        assert!(seen.iter().all(|&n| n > 800), "{seen:?}");
        // The widest range neither overflows nor panics.
        for _ in 0..100 {
            let _ = g.range_int(i64::MIN, i64::MAX);
        }
        assert!((0..100).all(|_| g.range_int(-5, 5).abs() <= 5));
        // A NaN bound is ignored by min/max: no panic, a finite draw.
        assert!(g.range(f32::NAN, 1.0).is_finite());
    }
}
