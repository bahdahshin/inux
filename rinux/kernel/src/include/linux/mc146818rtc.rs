// SPDX-License-Identifier: GPL-2.0
//! Motorola MC146818 real-time-clock register definitions.
//!
//! A subset of `include/linux/mc146818rtc.h` and the x86 port access of
//! `arch/x86/include/asm/mc146818rtc.h`: the index/data ports and the
//! register numbers and control bits the time read needs.

/// The RTC index/data ports: write a register number to `0x70`, then read or
/// write its value at `0x71`.
pub const RTC_PORT_INDEX: u16 = 0x70;
pub const RTC_PORT_DATA: u16 = 0x71;

// Clock register numbers.
pub const RTC_SECONDS: u8 = 0;
pub const RTC_MINUTES: u8 = 2;
pub const RTC_HOURS: u8 = 4;
pub const RTC_DAY_OF_MONTH: u8 = 7;
pub const RTC_MONTH: u8 = 8;
pub const RTC_YEAR: u8 = 9;

/// Register A: status, including the update-in-progress flag.
pub const RTC_REG_A: u8 = 10;
/// Register B: control, including the data-mode and 24-hour bits.
pub const RTC_REG_B: u8 = 11;

/// Register A: an update cycle is in progress; the time is unreliable.
pub const RTC_UIP: u8 = 0x80;

/// Register B: time/date values are binary; if clear they are BCD.
pub const RTC_DM_BINARY: u8 = 0x04;
/// Register B: 24-hour mode; if clear, hours bit 7 is the PM flag.
pub const RTC_24H: u8 = 0x02;
