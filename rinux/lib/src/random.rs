// SPDX-License-Identifier: GPL-2.0
//! The random source that the lib helpers draw from.
//!
//! Ported from the draw interface of `include/linux/random.h` and
//! `drivers/char/random.c` (`get_random_u32_below()`).
//!
//! The kernel's `get_random_u32()` reads per-CPU state and the entropy pool.
//! Lib code cannot reach those, so it takes a [`RandomSource`]. The kernel
//! implements the trait over its generator, and tests use the seeded
//! [`RndState`] from [`crate::random32`].

use crate::random32::RndState;

/// A source of random 32-bit words.
pub trait RandomSource {
    /// `get_random_u32()`: the next random 32 bits.
    fn get_random_u32(&mut self) -> u32;

    /// `get_random_bytes()`: fills `buf` from successive 32-bit draws, least
    /// significant byte first, as the C helpers take them.
    fn get_random_bytes(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(4) {
            let word = self.get_random_u32().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
    }

    /// `get_random_u32_below()`: a value in `[0, ceil)`, with no modulo bias.
    ///
    /// The draw is multiplied by `ceil` and the high word kept. Draws whose
    /// low word falls under the rejection threshold are discarded, as in C.
    /// `ceil` of 0 or 1 gives 0.
    fn get_random_u32_below(&mut self, ceil: u32) -> u32 {
        let mut ret = self.get_random_u32();
        let mut mult = ceil as u64 * ret as u64;
        let mut leftover = mult as u32;
        if leftover < ceil {
            // (2^32 - ceil) % ceil, computed in 32 bits as C does.
            let threshold = ceil.wrapping_neg() % ceil;
            while leftover < threshold {
                ret = self.get_random_u32();
                mult = ceil as u64 * ret as u64;
                leftover = mult as u32;
            }
        }
        (mult >> 32) as u32
    }
}

impl RandomSource for RndState {
    fn get_random_u32(&mut self) -> u32 {
        self.next_u32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source that returns a fixed sequence, then panics.
    struct Seq<'a>(&'a [u32]);

    impl RandomSource for Seq<'_> {
        fn get_random_u32(&mut self) -> u32 {
            let (&first, rest) = self.0.split_first().expect("ran out of draws");
            self.0 = rest;
            first
        }
    }

    #[test]
    fn below_takes_the_high_word_of_the_product() {
        // ceil 3, draw u32::MAX: the low word 2^32 - 3 is above the threshold.
        assert_eq!(Seq(&[u32::MAX]).get_random_u32_below(3), 2);
    }

    #[test]
    fn below_rejects_draws_under_the_threshold() {
        // ceil 3 has threshold (2^32 - 3) % 3 = 1, so the draw of 0 is
        // discarded and the next one is used.
        assert_eq!(Seq(&[0, 0x8000_0000]).get_random_u32_below(3), 1);
    }

    #[test]
    fn below_one_and_zero_give_zero() {
        assert_eq!(Seq(&[12345]).get_random_u32_below(1), 0);
        assert_eq!(Seq(&[12345]).get_random_u32_below(0), 0);
    }

    #[test]
    fn get_random_bytes_takes_little_endian_words() {
        let mut buf = [0u8; 6];
        Seq(&[0x0403_0201, 0x0000_0605]).get_random_bytes(&mut buf);
        assert_eq!(buf, [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn rnd_state_is_a_random_source() {
        let mut a = RndState::new(7);
        let mut b = RndState::new(7);
        assert_eq!(RandomSource::get_random_u32(&mut a), b.next_u32());
    }
}
