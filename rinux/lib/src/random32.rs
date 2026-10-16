// SPDX-License-Identifier: GPL-2.0
//! Seeded pseudo-random numbers (`prandom`).
//!
//! Ported from `lib/random32.c` and `include/linux/prandom.h`.
//!
//! This is a maximally equidistributed combined Tausworthe generator
//! (lfsr113) based on code from the GNU Scientific Library 1.5, from
//! P. L'Ecuyer, "Maximally Equidistributed Combined Tausworthe Generators",
//! Mathematics of Computation, 65, 213 (1996), 203--213. Its period is
//! about 2^113.
//!
//! It is not cryptographically secure. Use it for tests and simulations
//! with a known seed. Seeding the per-CPU state from the entropy pool
//! (`prandom_seed_full_state()`) will come with the random driver.

/// `struct rnd_state`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RndState {
    pub s1: u32,
    pub s2: u32,
    pub s3: u32,
    pub s4: u32,
}

/// `__seed()`: enforces the minimum seed values.
///
/// An erratum to the paper requires s1 > 1, s2 > 7, s3 > 15 and s4 > 127.
#[inline]
pub const fn seed_min(x: u32, m: u32) -> u32 {
    if x < m { x.wrapping_add(m) } else { x }
}

#[inline]
const fn tausworthe(s: u32, a: u32, b: u32, c: u32, d: u32) -> u32 {
    ((s & c) << d) ^ (((s << a) ^ s) >> b)
}

impl RndState {
    /// `prandom_seed_state()`: seeds the generator from a 64-bit value.
    pub const fn new(seed: u64) -> RndState {
        let i = ((seed >> 32) ^ (seed << 10) ^ seed) as u32;
        RndState {
            s1: seed_min(i, 2),
            s2: seed_min(i, 8),
            s3: seed_min(i, 16),
            s4: seed_min(i, 128),
        }
    }

    /// `prandom_u32_state()`: the next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        self.s1 = tausworthe(self.s1, 6, 13, 4_294_967_294, 18);
        self.s2 = tausworthe(self.s2, 2, 27, 4_294_967_288, 2);
        self.s3 = tausworthe(self.s3, 13, 21, 4_294_967_280, 7);
        self.s4 = tausworthe(self.s4, 3, 12, 4_294_967_168, 13);
        self.s1 ^ self.s2 ^ self.s3 ^ self.s4
    }

    /// `prandom_bytes_state()`: fills `buf` with random bytes.
    ///
    /// Whole words are stored in native byte order, as the C version does.
    pub fn fill_bytes(&mut self, buf: &mut [u8]) {
        let mut chunks = buf.chunks_exact_mut(4);
        for chunk in &mut chunks {
            chunk.copy_from_slice(&self.next_u32().to_ne_bytes());
        }
        let tail = chunks.into_remainder();
        if !tail.is_empty() {
            let mut rem = self.next_u32();
            for b in tail {
                *b = rem as u8;
                rem >>= 8;
            }
        }
    }

    /// `prandom_warmup()`: runs the generator ten times so the recurrence
    /// condition holds after seeding.
    pub fn warmup(&mut self) {
        for _ in 0..10 {
            self.next_u32();
        }
    }
}

/// `prandom_seed_state()`.
#[inline]
pub fn prandom_seed_state(state: &mut RndState, seed: u64) {
    *state = RndState::new(seed);
}

/// `prandom_u32_state()`.
#[inline]
pub fn prandom_u32_state(state: &mut RndState) -> u32 {
    state.next_u32()
}

/// `prandom_bytes_state()`.
#[inline]
pub fn prandom_bytes_state(state: &mut RndState, buf: &mut [u8]) {
    state.fill_bytes(buf);
}

/// `prandom_warmup()`.
#[inline]
pub fn prandom_warmup(state: &mut RndState) {
    state.warmup();
}
