// SPDX-License-Identifier: GPL-2.0
//! Delay loop calibration ("BogoMIPS").
//!
//! A partial port of `init/calibrate.c`, with `loops_per_jiffy` from
//! `init/main.c`. When the TSC is calibrated the loop count comes from its
//! frequency; otherwise the delay loop is timed against jiffies.
//!
//! Copyright (C) 1991, 1992  Linus Torvalds

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::arch::x86::lib::delay::__delay;
use crate::kernel::time::jiffies::{HZ, jiffies};

/// `loops_per_jiffy`: a guess until calibrated.
static LOOPS_PER_JIFFY: AtomicU64 = AtomicU64::new(1 << 12);

/// `lpj_fine`: loops per jiffy computed from a known timer frequency.
static LPJ_FINE: AtomicU64 = AtomicU64::new(0);

/// `loops_per_jiffy`.
pub fn loops_per_jiffy() -> u64 {
    LOOPS_PER_JIFFY.load(Ordering::Relaxed)
}

pub fn set_loops_per_jiffy(lpj: u64) {
    LOOPS_PER_JIFFY.store(lpj, Ordering::Relaxed);
}

pub fn set_lpj_fine(lpj: u64) {
    LPJ_FINE.store(lpj, Ordering::Relaxed);
}

/// `LPS_PREC`: bits of precision in the binary search.
const LPS_PREC: u32 = 8;

/// `calibrate_delay_converge()`: time the delay loop against jiffies, which
/// must be ticking.
fn calibrate_delay_converge() -> u64 {
    // First stage: slowly accelerate to find initial bounds.
    let lpj: u64 = 1 << 12;
    let mut trials: u64 = 0;
    let mut band: u64 = 0;
    let mut trial_in_band: u64 = 0;

    // Wait for the start of a clock tick.
    let mut ticks = jiffies();
    while ticks == jiffies() {
        core::hint::spin_loop();
    }
    // Go...
    ticks = jiffies();
    loop {
        trial_in_band += 1;
        if trial_in_band == 1 << band {
            band += 1;
            trial_in_band = 0;
        }
        __delay(lpj * band);
        trials += band;
        if ticks != jiffies() {
            break;
        }
    }
    // We overshot, so retreat to a clear underestimate. Then estimate the
    // largest likely undershoot. This defines our chop bounds.
    trials -= band;
    let mut loopadd_base = lpj * band;
    let mut lpj_base = lpj * trials;

    loop {
        let mut lpj = lpj_base;
        let mut loopadd = loopadd_base;

        // Binary approximation to get lpj equal to one tick (up to
        // LPS_PREC bits).
        let chop_limit = lpj >> LPS_PREC;
        while loopadd > chop_limit {
            lpj += loopadd;
            ticks = jiffies();
            while ticks == jiffies() {
                core::hint::spin_loop();
            }
            ticks = jiffies();
            __delay(lpj);
            if jiffies() != ticks {
                // Longer than one tick.
                lpj -= loopadd;
            }
            loopadd >>= 1;
        }

        // If we incremented every single time, presume we massively
        // underestimated and retry with a higher start and larger range.
        if lpj + loopadd * 2 == lpj_base + loopadd_base * 2 {
            lpj_base = lpj;
            loopadd_base <<= 2;
            continue;
        }
        return lpj;
    }
}

/// `calibrate_delay()`: sets `loops_per_jiffy` and prints BogoMIPS.
pub fn calibrate_delay() {
    static PRINTED: AtomicBool = AtomicBool::new(false);
    let printed = PRINTED.load(Ordering::Relaxed);
    let fine = LPJ_FINE.load(Ordering::Relaxed);

    let (lpj, how) = if !printed && fine != 0 {
        (
            fine,
            "Calibrating delay loop (skipped), value calculated using timer frequency.. ",
        )
    } else {
        (calibrate_delay_converge(), "Calibrating delay loop... ")
    };

    if !printed {
        pr_info!(
            "{}{}.{:02} BogoMIPS (lpj={})",
            how,
            lpj / (500_000 / HZ),
            (lpj / (5000 / HZ)) % 100,
            lpj
        );
    }
    set_loops_per_jiffy(lpj);
    PRINTED.store(true, Ordering::Relaxed);
}
