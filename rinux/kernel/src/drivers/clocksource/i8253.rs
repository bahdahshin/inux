// SPDX-License-Identifier: GPL-2.0
//! The 8253/8254 programmable interval timer.
//!
//! A partial port of `drivers/clocksource/i8253.c` with the ports of
//! `include/linux/i8253.h`: channel 0 as a periodic tick source. The PIT
//! clocksource and one-shot mode come with the clockevents framework.

use crate::arch::x86::include::io::outb_p;
use crate::kernel::locking::SpinLock;
use crate::kernel::time::jiffies::HZ;

/// `PIT_TICK_RATE`: the PIT input clock, from `include/linux/timex.h`.
pub const PIT_TICK_RATE: u64 = 1_193_182;
/// Mode/command register.
pub const PIT_MODE: u16 = 0x43;
/// Channel 0, the system timer.
pub const PIT_CH0: u16 = 0x40;
/// Channel 2, the speaker channel, used for calibration.
pub const PIT_CH2: u16 = 0x42;
/// `PIT_LATCH`: the channel 0 count for one tick.
pub const PIT_LATCH: u64 = (PIT_TICK_RATE + HZ / 2) / HZ;

/// `i8253_lock`: the PIT's registers are shared with calibration code.
pub static I8253_LOCK: SpinLock<()> = SpinLock::new(());

/// `pit_set_periodic()`: channel 0 in rate generator mode at `HZ`.
pub fn pit_set_periodic() {
    let _guard = I8253_LOCK.lock_irqsave();
    // SAFETY: the PIT ports belong to this driver.
    unsafe {
        // binary, mode 2, LSB/MSB, ch 0
        outb_p(0x34, PIT_MODE);
        outb_p((PIT_LATCH & 0xff) as u8, PIT_CH0); // LSB
        outb_p((PIT_LATCH >> 8) as u8, PIT_CH0); // MSB
    }
}
