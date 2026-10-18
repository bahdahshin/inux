// SPDX-License-Identifier: GPL-2.0
//! The system timer.
//!
//! A partial port of `arch/x86/kernel/time.c` and `setup_pit_timer()` from
//! `arch/x86/kernel/i8253.c`. The PIT drives the tick in one-shot mode, as a
//! clock event device: each IRQ0 re-arms the next tick, so the tick period is
//! set by the clockevents core rather than by the PIT's periodic mode. The
//! HPET and the local APIC timer come later.
//!
//! Copyright (c) 1991,1992,1995  Linus Torvalds

use rinux_lib::clockevents::{ClockEventDevice, NSEC_PER_SEC};

use crate::arch::x86::kernel::tsc::tsc_init;
use crate::drivers::clocksource::i8253::{PIT_TICK_RATE, PitOneshot, pit_read_count};
use crate::init::main::set_late_time_init;
use crate::kernel::irq::{IRQ_HANDLED, IrqReturn, request_irq};
use crate::kernel::locking::SpinLock;
use crate::kernel::time::jiffies::HZ;
use crate::kernel::time::tick_common::tick_handle_periodic;

/// One tick, in nanoseconds.
const TICK_PERIOD_NS: i64 = (NSEC_PER_SEC / HZ) as i64;

/// The PIT's ns-to-count scale, 32-bit shift: mult = rate * 2^32 / 1e9.
const PIT_SHIFT: u32 = 32;
const PIT_MULT: u64 = (PIT_TICK_RATE << PIT_SHIFT) / NSEC_PER_SEC;

/// The tick's clock event device: the PIT in one-shot mode.
static TICK_DEV: SpinLock<ClockEventDevice> = SpinLock::new(ClockEventDevice {
    mult: PIT_MULT,
    shift: PIT_SHIFT,
    min_delta_ns: 10_000,
    max_delta_ns: 60_000_000,
    min_delta_ticks: 0,
    next_event: 0,
    next_event_forced: false,
    retries: 0,
    shutdown: false,
});

/// Re-arm the tick for one period after the last one.
///
/// In mode 0 the PIT's count keeps running after it reaches zero: it wraps
/// to 0xffff and counts down again. The count read now therefore tells how
/// long ago the tick fired, so the next one-shot is shortened by that much
/// and the ticks stay on their schedule, with no drift from IRQ latency.
///
/// The fired event is acknowledged first. A late tick can leave the device
/// on its forced minimum-delta event; once that has fired it must not still
/// count as pending, or the next program arms nothing and the tick stops.
fn rearm_tick() {
    let mut dev = TICK_DEV.lock_irqsave();
    dev.event_fired();
    let since_fire = (0xffffu64 - pit_read_count() as u64) & 0xffff;
    let since_fire_ns = (since_fire * NSEC_PER_SEC / PIT_TICK_RATE) as i64;
    let expires = TICK_PERIOD_NS - since_fire_ns;
    if dev.program_event(&mut PitOneshot, expires, 0, true).is_err() {
        pr_err!("tick: could not re-arm the PIT tick");
    }
}

/// `timer_interrupt()`: the legacy timer IRQ0.
fn timer_interrupt(_irq: u32) -> IrqReturn {
    // global_clock_event->event_handler()
    tick_handle_periodic();
    rearm_tick();
    IRQ_HANDLED
}

/// `setup_default_timer_irq()`.
fn setup_default_timer_irq() {
    if request_irq(0, timer_interrupt, "timer").is_err() {
        pr_info!("Failed to register legacy timer interrupt");
    }
}

/// `setup_pit_timer()`: the first tick, one period from now.
fn setup_pit_timer() {
    if TICK_DEV
        .lock_irqsave()
        .program_event(&mut PitOneshot, TICK_PERIOD_NS, 0, true)
        .is_err()
    {
        pr_err!("tick: could not arm the first PIT tick");
    }
}

/// `hpet_time_init()`: with no HPET, the PIT.
fn hpet_time_init() {
    setup_pit_timer();
    setup_default_timer_irq();
}

/// `x86_late_time_init()`.
fn x86_late_time_init() {
    // x86_init.timers.timer_init()
    hpet_time_init();
    tsc_init();
}

/// `time_init()`: timers are set up late, once interrupts are on.
pub fn time_init() {
    set_late_time_init(x86_late_time_init);
}
