// SPDX-License-Identifier: GPL-2.0
//! The 8253/8254 programmable interval timer.
//!
//! A partial port of `drivers/clocksource/i8253.c` with the ports of
//! `include/linux/i8253.h`: channel 0 as a periodic tick source. The PIT
//! clocksource and one-shot mode come with the clockevents framework.

use crate::arch::x86::include::io::outb_p;
use crate::kernel::locking::SpinLock;

/// `PIT_TICK_RATE`: the PIT input clock, from `include/linux/timex.h`.
pub const PIT_TICK_RATE: u64 = 1_193_182;
/// Mode/command register.
pub const PIT_MODE: u16 = 0x43;
/// Channel 0, the system timer.
pub const PIT_CH0: u16 = 0x40;
/// Channel 2, the speaker channel, used for calibration.
pub const PIT_CH2: u16 = 0x42;

/// `i8253_lock`: the PIT's registers are shared with calibration code.
pub static I8253_LOCK: SpinLock<()> = SpinLock::new(());

/// Channel 0 in one-shot mode (mode 0, interrupt on terminal count),
/// LSB/MSB access, binary.
const PIT_ONESHOT_CMD: u8 = 0x30;

/// The longest count a 16-bit channel can hold.
pub const PIT_MAX_COUNT: u64 = 0xffff;

/// `pit_set_oneshot()`: arm channel 0 to count `count` ticks once, then stop.
/// Callers are the clockevents core, through [`PitOneshot`].
pub fn pit_set_oneshot(count: u16) {
    let _guard = I8253_LOCK.lock_irqsave();
    // SAFETY: the PIT ports belong to this driver.
    unsafe {
        outb_p(PIT_ONESHOT_CMD, PIT_MODE);
        outb_p((count & 0xff) as u8, PIT_CH0);
        outb_p((count >> 8) as u8, PIT_CH0);
    }
}

/// `pit_read_count()`: the current count of channel 0, latched and read back.
pub fn pit_read_count() -> u16 {
    let _guard = I8253_LOCK.lock_irqsave();
    use crate::arch::x86::include::io::{inb, outb};
    // SAFETY: the PIT ports belong to this driver. Command 0x00 latches
    // channel 0; the two data reads are LSB then MSB.
    unsafe {
        outb(0x00, PIT_MODE);
        let lo = inb(PIT_CH0) as u16;
        let hi = inb(PIT_CH0) as u16;
        (hi << 8) | lo
    }
}

/// The PIT as a clock event device: [`SetNextEvent`] arms a one-shot count.
pub struct PitOneshot;

impl rinux_lib::clockevents::SetNextEvent for PitOneshot {
    fn set_next_event(&mut self, cycles: u64) -> i32 {
        // A count of 0 or one past the 16-bit range cannot be armed; refusing
        // makes the clockevents core retry with its minimum delta.
        if cycles == 0 || cycles > PIT_MAX_COUNT {
            return -1;
        }
        pit_set_oneshot(cycles as u16);
        0
    }
}
