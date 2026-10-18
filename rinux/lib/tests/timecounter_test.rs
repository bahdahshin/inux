// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the timecounter: the scale, the fractional carry, and the
//! wrap of a narrow counter.

use rinux_lib::timecounter::{Cyclecounter, Timecounter};
use std::cell::{Cell, RefCell};

#[test]
fn counts_nanoseconds_from_the_start_value() {
    let c = Cell::new(0u64);
    let read = || {
        c.set(c.get() + 2);
        c.get()
    };
    // 10 ns per cycle: mult/2^shift = 160/16.
    let cc = Cyclecounter {
        read: &read,
        mask: u64::MAX,
        mult: 160,
        shift: 4,
    };
    let mut tc = Timecounter::new(&cc, 1000);
    assert_eq!(tc.read(), 1020);
    assert_eq!(tc.read(), 1040);
}

#[test]
fn fractions_of_a_nanosecond_carry_over() {
    let c = Cell::new(0u64);
    let read = || {
        c.set(c.get() + 1);
        c.get()
    };
    // One cycle is 1/16 ns, so 16 cycles are exactly 1 ns.
    let cc = Cyclecounter {
        read: &read,
        mask: u64::MAX,
        mult: 1,
        shift: 4,
    };
    let mut tc = Timecounter::new(&cc, 0);
    for _ in 0..15 {
        assert_eq!(tc.read(), 0);
    }
    assert_eq!(tc.read(), 1);
}

#[test]
fn a_narrow_counter_wrap_is_one_small_step() {
    // An 8-bit counter going 250 -> 6 is 12 cycles, not a huge jump.
    let seq = RefCell::new(vec![250u64, 6].into_iter());
    let read = || seq.borrow_mut().next().unwrap();
    let cc = Cyclecounter {
        read: &read,
        mask: 0xff,
        mult: 16,
        shift: 4,
    };
    let mut tc = Timecounter::new(&cc, 0);
    assert_eq!(tc.read(), 12);
}
