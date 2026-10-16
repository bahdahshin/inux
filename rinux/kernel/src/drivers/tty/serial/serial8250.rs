// SPDX-License-Identifier: GPL-2.0
//! The 8250/16550 UART, with a hardware loopback self-test.
//!
//! A small port of the probe-time checks of `drivers/tty/serial/8250/`
//! (`8250_port.c`): the scratch-register test used by `autoconfig()` to
//! detect a 16450-or-later UART, and the internal loopback used by
//! `autoconfig_irq()` -- with the MCR loopback bit set, a byte written to the
//! transmit register is received back on the same chip, which proves the UART
//! works without touching the line. Register numbers and bits come from
//! `include/uapi/linux/serial_reg.h` ([`crate::include::uapi::serial_reg`]).
//!
//! Not ported: the full tty/uart layer, interrupt-driven I/O, the FIFO sizing
//! and the many UART variants. rinux keeps using the early console for output.

use crate::arch::x86::include::io::{inb, outb};
use crate::include::uapi::serial_reg::*;

/// The COM1 I/O base (ttyS0).
pub const COM1: u16 = 0x3f8;

#[inline]
fn sin(base: u16, reg: u32) -> u8 {
    // SAFETY: base+reg is a UART register port we own.
    unsafe { inb(base + reg as u16) }
}

#[inline]
fn sout(base: u16, reg: u32, val: u8) {
    // SAFETY: as sin; a UART register write only configures the port.
    unsafe { outb(val, base + reg as u16) }
}

/// Scratch-register test: a 16450-or-later UART holds a byte written to its
/// otherwise-unused scratch register. Restores the register afterwards.
pub fn scratch_test(base: u16) -> bool {
    let save = sin(base, UART_SCR);
    sout(base, UART_SCR, 0xa5);
    let a = sin(base, UART_SCR);
    sout(base, UART_SCR, 0x5a);
    let b = sin(base, UART_SCR);
    sout(base, UART_SCR, save);
    a == 0xa5 && b == 0x5a
}

/// Loopback test: put the UART in internal loopback, transmit `byte`, and
/// read it back. Returns the received byte, or `None` on timeout. The IER,
/// LCR and MCR registers are saved and restored, so the console keeps working.
pub fn loopback_test(base: u16, byte: u8) -> Option<u8> {
    let save_ier = sin(base, UART_IER);
    let save_lcr = sin(base, UART_LCR);
    let save_mcr = sin(base, UART_MCR);

    sout(base, UART_IER, 0); // mask interrupts
    sout(base, UART_LCR, UART_LCR_WLEN8 as u8); // 8N1, DLAB clear
    sout(
        base,
        UART_MCR,
        (UART_MCR_LOOP | UART_MCR_RTS | UART_MCR_DTR) as u8,
    );

    // Wait for the transmit holding register to drain, then send.
    for _ in 0..100_000 {
        if sin(base, UART_LSR) & UART_LSR_THRE as u8 != 0 {
            break;
        }
    }
    sout(base, UART_TX, byte);

    // Wait for the byte to loop back into the receive register.
    let mut got = None;
    for _ in 0..100_000 {
        if sin(base, UART_LSR) & UART_LSR_DR as u8 != 0 {
            got = Some(sin(base, UART_RX));
            break;
        }
    }

    sout(base, UART_MCR, save_mcr);
    sout(base, UART_LCR, save_lcr);
    sout(base, UART_IER, save_ier);
    got
}
