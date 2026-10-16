// SPDX-License-Identifier: GPL-2.0-only
//! Reset of an i.MX/STMP block through its SET/CLR/value registers.
//!
//! Ported from `lib/stmp_device.c`, `stmp_reset_block()`, and the register
//! layout of `include/linux/stmp_device.h`. The sequence is the one C runs:
//! clear the soft-reset bit, clear clock gating, set soft reset, wait for the
//! clock gate to show, then clear soft reset and clock gating, polling each
//! bit clear with a bounded wait. Register access and the microsecond delay are
//! supplied by the caller, since the MMIO mapping is the kernel's.

/// `STMP_OFFSET_REG_SET`: the write-one-to-set alias of a register.
pub const STMP_OFFSET_REG_SET: usize = 0x4;
/// `STMP_OFFSET_REG_CLR`: the write-one-to-clear alias of a register.
pub const STMP_OFFSET_REG_CLR: usize = 0x8;
/// `STMP_MODULE_CLKGATE`: the clock gate bit.
pub const STMP_MODULE_CLKGATE: u32 = 1 << 30;
/// `STMP_MODULE_SFTRST`: the soft reset bit.
pub const STMP_MODULE_SFTRST: u32 = 1 << 31;

const TIMEOUT: u32 = 0x400;

use crate::errno::{ETIMEDOUT, Errno};

/// The register access that the reset needs.
pub trait StmpRegs {
    /// Read the 32-bit register at `offset` from the block base.
    fn read(&mut self, offset: usize) -> u32;
    /// Write the 32-bit register at `offset` from the block base.
    fn write(&mut self, offset: usize, value: u32);
    /// Wait about a microsecond (`udelay(1)` in C).
    fn delay_us_1(&mut self);
}

/// `stmp_clear_poll_bit()`: clear `mask` through the CLR alias and poll until
/// it reads clear. Returns true on timeout, as C's `!timeout` return.
fn clear_poll_bit<R: StmpRegs>(regs: &mut R, mask: u32) -> bool {
    let mut timeout = TIMEOUT;
    regs.write(STMP_OFFSET_REG_CLR, mask);
    regs.delay_us_1();
    while regs.read(0) & mask != 0 && timeout > 0 {
        timeout -= 1;
    }
    timeout == 0
}

/// `stmp_reset_block()`: reset the block. Returns `Ok(())`, or `Err(ETIMEDOUT)`
/// when a poll times out.
pub fn stmp_reset_block<R: StmpRegs>(regs: &mut R) -> Result<(), Errno> {
    if clear_poll_bit(regs, STMP_MODULE_SFTRST) {
        return Err(ETIMEDOUT);
    }
    regs.write(STMP_OFFSET_REG_CLR, STMP_MODULE_CLKGATE);
    regs.write(STMP_OFFSET_REG_SET, STMP_MODULE_SFTRST);
    regs.delay_us_1();

    let mut timeout = TIMEOUT;
    while regs.read(0) & STMP_MODULE_CLKGATE == 0 && timeout > 0 {
        timeout -= 1;
    }
    if timeout == 0 {
        return Err(ETIMEDOUT);
    }

    if clear_poll_bit(regs, STMP_MODULE_SFTRST) {
        return Err(ETIMEDOUT);
    }
    if clear_poll_bit(regs, STMP_MODULE_CLKGATE) {
        return Err(ETIMEDOUT);
    }
    Ok(())
}
