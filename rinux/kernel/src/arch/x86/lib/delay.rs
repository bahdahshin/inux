// SPDX-License-Identifier: GPL-2.0
//! Precise delays.
//!
//! Ported from `arch/x86/lib/delay.c`: a calibrated busy loop until the TSC
//! is known, then TSC-based waiting. The TPAUSE/MWAITX variants are not
//! ported.
//!
//! Copyright (C) 1993 Linus Torvalds
//! Copyright (C) 1997 Martin Mares <mj@atrey.karlin.mff.cuni.cz>
//! Copyright (C) 2008 Jiri Hladky <hladky _dot_ jiri _at_ gmail _dot_ com>

use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::arch::x86::include::processor::{cpu_relax, rdtsc_ordered};
use crate::init::calibrate::loops_per_jiffy;
use crate::kernel::time::jiffies::HZ;

static USE_TSC_DELAY: AtomicBool = AtomicBool::new(false);

/// `delay_loop()`: a simple loop based delay.
fn delay_loop(loops: u64) {
    // SAFETY: a register-only countdown loop.
    unsafe {
        asm!(
            "test {0}, {0}",
            "jz 3f",
            "jmp 4f",
            ".align 16",
            "4: jmp 2f",
            ".align 16",
            "2: dec {0}",
            "jnz 2b",
            "3: dec {0}",
            inout(reg) loops => _,
            options(nomem, nostack),
        );
    }
}

/// `delay_tsc()`: waits `cycles` TSC cycles.
fn delay_tsc(cycles: u64) {
    let bclock = rdtsc_ordered();
    loop {
        let now = rdtsc_ordered();
        if now.wrapping_sub(bclock) >= cycles {
            break;
        }
        cpu_relax();
    }
}

/// `use_tsc_delay()`: switch to the TSC once it is calibrated.
pub fn use_tsc_delay() {
    USE_TSC_DELAY.store(true, Ordering::Relaxed);
}

/// `__delay()`: busy-waits `loops` loops (or TSC cycles).
pub fn __delay(loops: u64) {
    if USE_TSC_DELAY.load(Ordering::Relaxed) {
        delay_tsc(loops);
    } else {
        delay_loop(loops);
    }
}

/// `__const_udelay()`: waits `xloops` units of 2^-32 seconds.
pub fn __const_udelay(xloops: u64) {
    let lpj = loops_per_jiffy();
    // mull: the high half of the 32x32 product of xloops * 4 and lpj * HZ/4.
    let xloops = ((xloops.wrapping_mul(4) as u32 as u64) * ((lpj * (HZ / 4)) as u32 as u64)) >> 32;
    __delay(xloops + 1);
}

/// `udelay()`: busy-waits `usecs` microseconds.
pub fn udelay(usecs: u64) {
    __const_udelay(usecs * 0x0000_10c7); // 2**32 / 1000000 (rounded up)
}

/// `mdelay()`.
pub fn mdelay(msecs: u64) {
    for _ in 0..msecs {
        udelay(1000);
    }
}
