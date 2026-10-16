// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `cmpxchg_emu_u8`: byte positions in a word, mismatches, and
//! concurrent updates to neighbouring bytes of one word.

use rinux_lib::cmpxchg_emu::cmpxchg_emu_u8;
use std::sync::atomic::{AtomicU32, Ordering};

#[test]
fn swaps_only_the_named_byte() {
    let words = [AtomicU32::new(0x4433_2211)];
    assert_eq!(cmpxchg_emu_u8(&words, 1, 0x22, 0xaa), 0x22);
    assert_eq!(words[0].load(Ordering::Relaxed), 0x4433_aa11);
}

#[test]
fn a_mismatch_returns_the_current_byte_and_changes_nothing() {
    let words = [AtomicU32::new(0x4433_2211)];
    assert_eq!(cmpxchg_emu_u8(&words, 2, 0x99, 0x00), 0x33);
    assert_eq!(words[0].load(Ordering::Relaxed), 0x4433_2211);
}

#[test]
fn byte_index_crosses_into_the_next_word() {
    let words = [AtomicU32::new(0), AtomicU32::new(0)];
    assert_eq!(cmpxchg_emu_u8(&words, 4, 0, 0x7f), 0);
    assert_eq!(words[1].load(Ordering::Relaxed), 0x7f);
    assert_eq!(words[0].load(Ordering::Relaxed), 0);
}

#[test]
fn concurrent_neighbouring_bytes_all_land() {
    let words = [AtomicU32::new(0)];
    std::thread::scope(|s| {
        for i in 0..4usize {
            let words = &words;
            s.spawn(move || {
                // Each thread sets its own byte once, retrying on the race
                // with the other threads' whole-word swaps.
                loop {
                    if cmpxchg_emu_u8(words, i, 0, (i as u8) + 1) == 0 {
                        break;
                    }
                }
            });
        }
    });
    assert_eq!(words[0].load(Ordering::Relaxed), 0x0403_0201);
}
