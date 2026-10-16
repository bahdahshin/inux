// SPDX-License-Identifier: GPL-2.0-only
//! A one-byte compare-and-swap built from a 32-bit one.
//!
//! Ported from `lib/cmpxchg-emu.c`, `cmpxchg_emu_u8()`. Some architectures have
//! no byte `cmpxchg`, so C finds the 32-bit word that holds the byte, reads it,
//! and swaps the whole word when only that byte changed. The word is retried
//! until the swap succeeds or the byte no longer matches. Here the bytes live in
//! an array of `AtomicU32` words, byte `i` of the array being byte `i % 4` of
//! word `i / 4`, least significant first, as the C union lays out a little-endian
//! word.

use core::sync::atomic::{AtomicU32, Ordering};

/// `cmpxchg_emu_u8()`: if byte `index` equals `old`, replace it with `new`.
/// Returns the byte's value before the call, which is `old` on success.
///
/// Panics if `index` is outside `words`.
pub fn cmpxchg_emu_u8(words: &[AtomicU32], index: usize, old: u8, new: u8) -> u8 {
    let word = &words[index / 4];
    let shift = 8 * (index % 4) as u32;
    let mut cur = word.load(Ordering::Relaxed);
    loop {
        let byte = (cur >> shift) as u8;
        if byte != old {
            return byte;
        }
        let next = (cur & !(0xff << shift)) | (u32::from(new) << shift);
        match word.compare_exchange(cur, next, Ordering::SeqCst, Ordering::Relaxed) {
            Ok(_) => return old,
            Err(actual) => cur = actual,
        }
    }
}
