// SPDX-License-Identifier: GPL-2.0
//! `panic()`.
//!
//! A partial port of `kernel/panic.c`. Rust panics, which mean a bug in
//! kernel code, are reported like `BUG()` (`lib/bug.c`) and then panic.
//!
//! Not yet ported: `panic=` reboot timeouts, panic notifiers, stopping
//! other CPUs, kdump, and backtraces.

use core::fmt;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::arch::x86::include::irqflags::{halt_forever, local_irq_disable};
use crate::kernel::printk::bust_spinlocks;

static PANICKING: AtomicBool = AtomicBool::new(false);

/// `panic()`: halts the system after printing a message.
pub fn panic(args: fmt::Arguments<'_>) -> ! {
    local_irq_disable();
    // A panic while panicking: don't try to print again.
    if PANICKING.swap(true, Ordering::Relaxed) {
        halt_forever();
    }
    bust_spinlocks();

    pr_emerg!("Kernel panic - not syncing: {}", args);
    pr_emerg!("---[ end Kernel panic - not syncing: {} ]---", args);
    halt_forever()
}

#[panic_handler]
fn rust_panic(info: &PanicInfo<'_>) -> ! {
    local_irq_disable();
    if !PANICKING.load(Ordering::Relaxed) {
        bust_spinlocks();
        match info.location() {
            Some(loc) => pr_crit!("kernel BUG at {}:{}!", loc.file(), loc.line()),
            None => pr_crit!("kernel BUG!"),
        }
    }
    panic(format_args!("{}", info.message()))
}
