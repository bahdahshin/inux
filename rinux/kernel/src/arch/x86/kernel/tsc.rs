// SPDX-License-Identifier: GPL-2.0-only
//! The time-stamp counter.
//!
//! A partial port of `arch/x86/kernel/tsc.c`: frequency detection (CPUID
//! leaves 0x15/0x16, then the fast PIT calibration, then the slow one),
//! the cycles-to-nanoseconds scale, and `native_sched_clock()`.
//!
//! Not yet ported: the model-specific crystal tables and MSR methods, HPET
//! and ACPI PM timer references, TSC_ADJUST and synchronization checks
//! between CPUs, the TSC clocksource and its watchdog, and refinement.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use rinux_lib::math::mul_u64_u32_shr;

use crate::arch::x86::include::io::{inb, outb};
use crate::arch::x86::include::irqflags::{local_irq_restore, local_irq_save};
use crate::arch::x86::include::processor::{cpuid, get_cycles, rdtsc};
use crate::arch::x86::kernel::cpu::common::{X86_FEATURE_TSC, X86_VENDOR_INTEL, boot_cpu_data};
use crate::arch::x86::lib::delay::use_tsc_delay;
use crate::drivers::clocksource::i8253::PIT_TICK_RATE;
use crate::init::calibrate::{set_loops_per_jiffy, set_lpj_fine};
use crate::kernel::time::clocksource::clocks_calc_mult_shift;
use crate::kernel::time::jiffies::{HZ, INITIAL_JIFFIES, jiffies};
use crate::kernel::time::{KHZ, NSEC_PER_MSEC, NSEC_PER_SEC};

/// `cpu_khz`.
pub static CPU_KHZ: AtomicU64 = AtomicU64::new(0);
/// `tsc_khz`.
pub static TSC_KHZ: AtomicU64 = AtomicU64::new(0);

/// `__use_tsc`: `sched_clock()` reads the TSC.
static USE_TSC: AtomicBool = AtomicBool::new(false);

/// `struct cyc2ns_data`, one copy: there is one CPU and it is written
/// before `USE_TSC` is set.
static CYC2NS_MUL: AtomicU32 = AtomicU32::new(0);
static CYC2NS_SHIFT: AtomicU32 = AtomicU32::new(0);
static CYC2NS_OFFSET: AtomicU64 = AtomicU64::new(0);

const CPUID_LEAF_TSC: i32 = 0x15;
const CPUID_LEAF_FREQ: i32 = 0x16;

/// `__cycles_2_ns()`.
fn cycles_2_ns(cyc: u64) -> u64 {
    let mul = CYC2NS_MUL.load(Ordering::Relaxed);
    let shift = CYC2NS_SHIFT.load(Ordering::Relaxed);
    CYC2NS_OFFSET
        .load(Ordering::Relaxed)
        .wrapping_add(mul_u64_u32_shr(cyc, mul, shift))
}

/// `__set_cyc2ns_scale()`: a new scale, keeping the clock continuous.
fn set_cyc2ns_scale(khz: u64, tsc_now: u64) {
    let ns_now = cycles_2_ns(tsc_now);
    let (mut mul, mut shift) = clocks_calc_mult_shift(khz as u32, NSEC_PER_MSEC as u32, 0);
    // The shift is exported to perf user pages, which expect at most 31.
    if shift == 32 {
        shift = 31;
        mul >>= 1;
    }
    let offset = ns_now.wrapping_sub(mul_u64_u32_shr(tsc_now, mul, shift));
    CYC2NS_MUL.store(mul, Ordering::Relaxed);
    CYC2NS_SHIFT.store(shift, Ordering::Relaxed);
    CYC2NS_OFFSET.store(offset, Ordering::Relaxed);
}

/// `native_sched_clock()`: nanoseconds since boot.
pub fn native_sched_clock() -> u64 {
    if USE_TSC.load(Ordering::Acquire) {
        return cycles_2_ns(rdtsc());
    }
    // Fall back to jiffies if there's no TSC available.
    jiffies().wrapping_sub(INITIAL_JIFFIES) * (NSEC_PER_SEC / HZ)
}

/// `cpu_khz_from_cpuid()`: CPUID leaf 0x16, on Intel.
fn cpu_khz_from_cpuid() -> u64 {
    let c = boot_cpu_data();
    if c.x86_vendor != X86_VENDOR_INTEL || c.cpuid_level < CPUID_LEAF_FREQ {
        return 0;
    }
    let (eax_base_mhz, _, _, _) = cpuid(CPUID_LEAF_FREQ as u32);
    eax_base_mhz as u64 * 1000
}

/// `native_calibrate_tsc()`: the TSC/crystal ratio of CPUID leaf 0x15.
fn native_calibrate_tsc() -> u64 {
    let c = boot_cpu_data();
    if c.x86_vendor != X86_VENDOR_INTEL || c.cpuid_level < CPUID_LEAF_TSC {
        return 0;
    }
    let (eax_denominator, ebx_numerator, ecx_hz, _) = cpuid(CPUID_LEAF_TSC as u32);
    if ebx_numerator == 0 || eax_denominator == 0 {
        return 0;
    }
    let mut crystal_khz = ecx_hz as u64 / 1000;
    // Some CPUs don't report the crystal; derive it from leaf 0x16.
    if crystal_khz == 0 && c.cpuid_level >= CPUID_LEAF_FREQ {
        let (eax_base_mhz, _, _, _) = cpuid(CPUID_LEAF_FREQ as u32);
        crystal_khz = eax_base_mhz as u64 * 1000 * eax_denominator as u64 / ebx_numerator as u64;
    }
    if crystal_khz == 0 {
        return 0;
    }
    crystal_khz * ebx_numerator as u64 / eax_denominator as u64
}

/// PIT channel 2 and the speaker gate, which only calibration uses.
fn port_in(port: u16) -> u8 {
    // SAFETY: calibration owns PIT channel 2 and port 0x61's gate bit.
    unsafe { inb(port) }
}

fn port_out(value: u8, port: u16) {
    // SAFETY: as for port_in.
    unsafe { outb(value, port) }
}

/// `pit_verify_msb()`.
fn pit_verify_msb(val: u8) -> bool {
    // Ignore the LSB.
    port_in(0x42);
    port_in(0x42) == val
}

/// `pit_expect_msb()`: wait while the PIT MSB reads `val`; returns the TSC
/// at the last read that still saw it and the uncertainty of that read.
fn pit_expect_msb(val: u8) -> Option<(u64, u64)> {
    let mut tsc = 0u64;
    let mut prev_tsc = 0u64;
    let mut count = 0;
    while count < 50000 {
        if !pit_verify_msb(val) {
            break;
        }
        prev_tsc = tsc;
        tsc = get_cycles();
        count += 1;
    }
    let delta = get_cycles().wrapping_sub(prev_tsc);
    // We require some success; the quality control is based on the error
    // terms of the TSC values.
    (count > 5).then_some((tsc, delta))
}

const MAX_QUICK_PIT_MS: u64 = 50;
const MAX_QUICK_PIT_ITERATIONS: u64 = MAX_QUICK_PIT_MS * PIT_TICK_RATE / 1000 / 256;

/// `quick_pit_calibrate()`: count TSC cycles while the PIT's MSB counts
/// down, until the error is below 500 ppm.
fn quick_pit_calibrate() -> u64 {
    // Set the gate high, disable the speaker.
    port_out((port_in(0x61) & !0x02) | 0x01, 0x61);

    // Counter 2, mode 0 (one-shot), binary count. Mode 0 decrements by one,
    // which is better than mode 2 for looking at individual counts.
    port_out(0xb0, 0x43);
    // Start at 0xffff.
    port_out(0xff, 0x42);
    port_out(0xff, 0x42);

    // The PIT starts counting at the next edge; reading the counter back
    // once takes about a microsecond, which is enough delay.
    pit_verify_msb(0);

    let mut success = None;
    if let Some((tsc, d1)) = pit_expect_msb(0xff) {
        for i in 1..=MAX_QUICK_PIT_ITERATIONS {
            let Some((t2, d2)) = pit_expect_msb((0xff - i) as u8) else {
                break;
            };
            let delta = t2.wrapping_sub(tsc);
            // Extrapolate the error and fail fast if it will never be
            // below 500 ppm.
            if i == 1 && d1 + d2 >= (delta * MAX_QUICK_PIT_ITERATIONS) >> 11 {
                return 0;
            }
            // Iterate until the error is less than 500 ppm.
            if d1 + d2 >= delta >> 11 {
                continue;
            }
            // Check the PIT one more time to verify that all TSC reads were
            // stable with respect to it.
            if !pit_verify_msb((0xfe - i) as u8) {
                break;
            }
            success = Some((delta, i));
            break;
        }
    }

    let Some((delta, i)) = success else {
        pr_info!("tsc: Fast TSC calibration failed");
        return 0;
    };
    // kHz = ticks / time-in-seconds / 1000
    //     = ((t2 - t1) * PIT_TICK_RATE) / (I * 256 * 1000)
    let khz = delta * PIT_TICK_RATE / (i * 256 * 1000);
    pr_info!("tsc: Fast TSC calibration using PIT");
    khz
}

const CAL_MS: u64 = 10;
const CAL_LATCH: u64 = PIT_TICK_RATE / (1000 / CAL_MS);
const CAL_PIT_LOOPS: u32 = 1000;
const CAL2_MS: u64 = 50;
const CAL2_LATCH: u64 = PIT_TICK_RATE / (1000 / CAL2_MS);
const CAL2_PIT_LOOPS: u32 = 5000;

/// `pit_calibrate_tsc()`: TSC cycles per millisecond over a PIT one-shot
/// of `ms` milliseconds, or `u64::MAX` if the measurement was disturbed.
fn pit_calibrate_tsc(latch: u64, ms: u64, loopmin: u32) -> u64 {
    // Set the gate high, disable the speaker.
    port_out((port_in(0x61) & !0x02) | 0x01, 0x61);

    // Channel 2, mode 0 (interrupt on terminal count), binary count, with
    // the latch loaded to start the countdown.
    port_out(0xb0, 0x43);
    port_out((latch & 0xff) as u8, 0x42);
    port_out((latch >> 8) as u8, 0x42);

    let t1 = get_cycles();
    let mut tsc = t1;
    let mut t2 = t1;
    let mut pitcnt = 0u32;
    let mut tscmax = 0u64;
    let mut tscmin = u64::MAX;
    while port_in(0x61) & 0x20 == 0 {
        t2 = get_cycles();
        let delta = t2.wrapping_sub(tsc);
        tsc = t2;
        tscmin = tscmin.min(delta as u32 as u64);
        tscmax = tscmax.max(delta as u32 as u64);
        pitcnt += 1;
    }

    // Too few reads, or one read ten times slower than the fastest, means
    // an SMI (or a busy hypervisor) got in the way.
    if pitcnt < loopmin || tscmax > 10 * tscmin {
        return u64::MAX;
    }
    t2.wrapping_sub(t1) / ms
}

/// `pit_hpet_ptimer_calibrate_cpu()` without HPET or PM timer: the lowest
/// of three PIT calibrations.
fn pit_hpet_ptimer_calibrate_cpu() -> u64 {
    let (mut latch, mut ms, mut loopmin) = (CAL_LATCH, CAL_MS, CAL_PIT_LOOPS);
    let mut tsc_pit_min = u64::MAX;
    for i in 0..3 {
        let flags = local_irq_save();
        let tsc_pit_khz = pit_calibrate_tsc(latch, ms, loopmin);
        local_irq_restore(flags);
        tsc_pit_min = tsc_pit_min.min(tsc_pit_khz);
        // If the PIT failed twice, give a virtual machine a longer window.
        if i == 1 && tsc_pit_min == u64::MAX {
            latch = CAL2_LATCH;
            ms = CAL2_MS;
            loopmin = CAL2_PIT_LOOPS;
        }
    }
    if tsc_pit_min == u64::MAX {
        pr_warn!("tsc: Unable to calibrate against PIT");
        pr_notice!("tsc: No reference (HPET/PMTIMER) available");
        return 0;
    }
    pr_info!("tsc: Using PIT calibration value");
    tsc_pit_min
}

/// `native_calibrate_cpu_early()`.
fn native_calibrate_cpu_early() -> u64 {
    let mut fast_calibrate = cpu_khz_from_cpuid();
    if fast_calibrate == 0 {
        let flags = local_irq_save();
        fast_calibrate = quick_pit_calibrate();
        local_irq_restore(flags);
    }
    fast_calibrate
}

/// `native_calibrate_cpu()`.
fn native_calibrate_cpu() -> u64 {
    match native_calibrate_cpu_early() {
        0 => pit_hpet_ptimer_calibrate_cpu(),
        khz => khz,
    }
}

/// `determine_cpu_tsc_frequencies()`.
fn determine_cpu_tsc_frequencies(early: bool) -> bool {
    let mut cpu_khz;
    let mut tsc_khz;
    if early {
        cpu_khz = native_calibrate_cpu_early();
        tsc_khz = native_calibrate_tsc();
    } else {
        cpu_khz = pit_hpet_ptimer_calibrate_cpu();
        tsc_khz = 0;
    }

    // Trust a non-zero tsc_khz as authoritative and use it to sanity check
    // cpu_khz, which will be off if the system timer is.
    if tsc_khz == 0 {
        tsc_khz = cpu_khz;
    } else if cpu_khz.abs_diff(tsc_khz) * 10 > tsc_khz {
        cpu_khz = tsc_khz;
    }
    CPU_KHZ.store(cpu_khz, Ordering::Relaxed);
    TSC_KHZ.store(tsc_khz, Ordering::Relaxed);

    if tsc_khz == 0 {
        return false;
    }
    pr_info!(
        "tsc: Detected {}.{:03} MHz processor",
        cpu_khz / KHZ,
        cpu_khz % KHZ
    );
    if cpu_khz != tsc_khz {
        pr_info!(
            "tsc: Detected {}.{:03} MHz TSC",
            tsc_khz / KHZ,
            tsc_khz % KHZ
        );
    }
    true
}

/// `get_loops_per_jiffy()`: with TSC delays, a "loop" is a TSC cycle.
fn get_loops_per_jiffy() -> u64 {
    TSC_KHZ.load(Ordering::Relaxed) * KHZ / HZ
}

/// `tsc_enable_sched_clock()`.
fn tsc_enable_sched_clock() {
    set_loops_per_jiffy(get_loops_per_jiffy());
    use_tsc_delay();
    // cyc2ns_init_boot_cpu()
    set_cyc2ns_scale(TSC_KHZ.load(Ordering::Relaxed), rdtsc());
    USE_TSC.store(true, Ordering::Release);
}

fn has_tsc() -> bool {
    boot_cpu_data().has(X86_FEATURE_TSC)
}

/// `tsc_early_init()`: calibrate in `setup_arch()` if a fast method works.
pub fn tsc_early_init() {
    if !has_tsc() {
        return;
    }
    if !determine_cpu_tsc_frequencies(true) {
        return;
    }
    tsc_enable_sched_clock();
}

/// `tsc_init()`: calibrate the slow way if the early attempt failed.
pub fn tsc_init() {
    if !has_tsc() {
        return;
    }
    if TSC_KHZ.load(Ordering::Relaxed) == 0 {
        // We failed to determine frequencies earlier; try again.
        if !determine_cpu_tsc_frequencies(false) {
            pr_info!("tsc: Marking TSC unstable due to could not calculate TSC khz");
            return;
        }
        tsc_enable_sched_clock();
    }
    set_lpj_fine(get_loops_per_jiffy());
}
