// SPDX-License-Identifier: GPL-2.0
//! The PC CMOS real-time clock.
//!
//! A small port of `drivers/rtc/rtc-cmos.c` and the time read of
//! `drivers/rtc/rtc-mc146818-lib.c` (`mc146818_get_time`): read the clock
//! registers through the CMOS index/data ports, waiting out any update cycle
//! and converting from BCD / 12-hour form as register B dictates.
//!
//! Simplified from Linux's: a read-only one-shot time read, no alarms,
//! periodic interrupts, update interrupts or NVRAM, and no century register
//! (the 2-digit year is taken as 20xx).

use rinux_lib::bcd::bcd2bin;

use crate::arch::x86::include::io::{inb, outb};
use crate::include::linux::mc146818rtc::*;

/// A wall-clock reading from the RTC.
pub struct RtcTime {
    pub year: u32,
    /// Month, 1..=12.
    pub month: u8,
    /// Day of month, 1..=31.
    pub mday: u8,
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
}

/// `CMOS_READ()`: read CMOS register `reg`.
fn cmos_read(reg: u8) -> u8 {
    // SAFETY: 0x70/0x71 are the CMOS index/data ports; reading a clock
    // register has no side effect beyond selecting it.
    unsafe {
        outb(reg, RTC_PORT_INDEX);
        inb(RTC_PORT_DATA)
    }
}

/// `mc146818_get_time()`: read the current time from the RTC.
pub fn read_time() -> RtcTime {
    // Wait for any in-progress update to finish so the registers are stable.
    let mut spins = 0u32;
    while cmos_read(RTC_REG_A) & RTC_UIP != 0 && spins < 1_000_000 {
        spins += 1;
        core::hint::spin_loop();
    }

    let mut sec = cmos_read(RTC_SECONDS);
    let mut min = cmos_read(RTC_MINUTES);
    let mut hour = cmos_read(RTC_HOURS);
    let mut mday = cmos_read(RTC_DAY_OF_MONTH);
    let mut month = cmos_read(RTC_MONTH);
    let mut year = cmos_read(RTC_YEAR);
    let regb = cmos_read(RTC_REG_B);

    let is_bcd = regb & RTC_DM_BINARY == 0;
    let is_12h = regb & RTC_24H == 0;
    // In 12-hour mode the top bit of the hours register is the PM flag.
    let pm = is_12h && (hour & 0x80 != 0);
    hour &= 0x7f;

    if is_bcd {
        sec = bcd2bin(sec) as u8;
        min = bcd2bin(min) as u8;
        hour = bcd2bin(hour) as u8;
        mday = bcd2bin(mday) as u8;
        month = bcd2bin(month) as u8;
        year = bcd2bin(year) as u8;
    }

    if is_12h {
        // 12 AM is 0, 12 PM stays 12, other PM hours add 12.
        if pm {
            if hour != 12 {
                hour += 12;
            }
        } else if hour == 12 {
            hour = 0;
        }
    }

    RtcTime {
        year: year as u32 + 2000,
        month,
        mday,
        hour,
        min,
        sec,
    }
}
