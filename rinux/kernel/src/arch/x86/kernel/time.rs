// SPDX-License-Identifier: GPL-2.0
//! The system timer.
//!
//! A partial port of `arch/x86/kernel/time.c` and `setup_pit_timer()` from
//! `arch/x86/kernel/i8253.c`: the PIT drives a periodic tick on IRQ0. The
//! HPET and the local APIC timer come later.
//!
//! Copyright (c) 1991,1992,1995  Linus Torvalds

use crate::arch::x86::kernel::tsc::tsc_init;
use crate::drivers::clocksource::i8253::pit_set_periodic;
use crate::init::main::set_late_time_init;
use crate::kernel::irq::{IRQ_HANDLED, IrqReturn, request_irq};
use crate::kernel::time::tick_common::tick_handle_periodic;

/// `timer_interrupt()`: the legacy timer IRQ0.
fn timer_interrupt(_irq: u32) -> IrqReturn {
    // global_clock_event->event_handler()
    tick_handle_periodic();
    IRQ_HANDLED
}

/// `setup_default_timer_irq()`.
fn setup_default_timer_irq() {
    if request_irq(0, timer_interrupt, "timer").is_err() {
        pr_info!("Failed to register legacy timer interrupt");
    }
}

/// `setup_pit_timer()`.
fn setup_pit_timer() {
    pit_set_periodic();
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
