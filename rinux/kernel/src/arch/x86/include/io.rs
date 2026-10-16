// SPDX-License-Identifier: GPL-2.0
//! Port I/O.
//!
//! Ported from `arch/x86/include/asm/shared/io.h`.
//!
//! These are `unsafe` because a port write can do anything the device on
//! the other end does, including DMA over arbitrary memory. Callers must
//! own the port they touch.

use core::arch::asm;

/// `outb()`: writes a byte to an I/O port.
///
/// # Safety
///
/// The caller must own `port` and know the write has no unsafe effect.
#[inline]
pub unsafe fn outb(value: u8, port: u16) {
    // SAFETY: the caller owns the port.
    unsafe {
        asm!("outb %al, %dx", in("al") value, in("dx") port, options(att_syntax, nomem, nostack, preserves_flags))
    }
}

/// `outw()`: writes a 16-bit value to an I/O port.
///
/// # Safety
///
/// As for [`outb`].
#[inline]
pub unsafe fn outw(value: u16, port: u16) {
    // SAFETY: the caller owns the port.
    unsafe {
        asm!("outw %ax, %dx", in("ax") value, in("dx") port, options(att_syntax, nomem, nostack, preserves_flags))
    }
}

/// `outl()`: writes a 32-bit value to an I/O port.
///
/// # Safety
///
/// As for [`outb`].
#[inline]
pub unsafe fn outl(value: u32, port: u16) {
    // SAFETY: the caller owns the port.
    unsafe {
        asm!("outl %eax, %dx", in("eax") value, in("dx") port, options(att_syntax, nomem, nostack, preserves_flags))
    }
}

/// `inb()`: reads a byte from an I/O port.
///
/// # Safety
///
/// The caller must own `port`; reads can have side effects on devices.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: the caller owns the port.
    unsafe {
        asm!("inb %dx, %al", out("al") value, in("dx") port, options(att_syntax, nomem, nostack, preserves_flags))
    }
    value
}

/// `inw()`: reads a 16-bit value from an I/O port.
///
/// # Safety
///
/// As for [`inb`].
#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    // SAFETY: the caller owns the port.
    unsafe {
        asm!("inw %dx, %ax", out("ax") value, in("dx") port, options(att_syntax, nomem, nostack, preserves_flags))
    }
    value
}

/// `inl()`: reads a 32-bit value from an I/O port.
///
/// # Safety
///
/// As for [`inb`].
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    // SAFETY: the caller owns the port.
    unsafe {
        asm!("inl %dx, %eax", out("eax") value, in("dx") port, options(att_syntax, nomem, nostack, preserves_flags))
    }
    value
}

/// `native_io_delay()`: a write to port 0x80, the traditional way to wait
/// about a microsecond for slow ISA devices.
#[inline]
pub fn native_io_delay() {
    // SAFETY: port 0x80 is the POST diagnostic port, unused otherwise.
    unsafe { outb(0, 0x80) }
}

/// `outb_p()`: [`outb`] followed by an I/O delay.
///
/// # Safety
///
/// As for [`outb`].
#[inline]
pub unsafe fn outb_p(value: u8, port: u16) {
    // SAFETY: the caller owns the port.
    unsafe { outb(value, port) };
    native_io_delay();
}
