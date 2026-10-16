// SPDX-License-Identifier: GPL-2.0
//! The i8042 PS/2 keyboard/mouse controller.
//!
//! A small port of the low-level command path of `drivers/input/serio/i8042.c`
//! and its port access in `i8042-io.h`: the status-register polling
//! (`i8042_wait_read`/`i8042_wait_write`), the encoded command/response
//! helper (`__i8042_command`), and the controller and keyboard-interface
//! self-tests. The `serio` bus, interrupt handling, AUX/MUX ports and the
//! platform quirks are not ported, so rinux checks the controller but does
//! not yet receive keystrokes.

use crate::arch::x86::include::io::{inb, native_io_delay, outb};

/// Data port (read scancodes / write parameters).
const DATA_REG: u16 = 0x60;
/// Command/status port.
const COMMAND_REG: u16 = 0x64;

/// Status bit: the output buffer holds a byte to read.
const STR_OBF: u8 = 0x01;
/// Status bit: the input buffer still holds a byte the controller hasn't taken.
const STR_IBF: u8 = 0x02;

/// Poll iterations before giving up on the controller.
const CTL_TIMEOUT: u32 = 10_000;

/// Controller self-test command; returns one byte, `0x55` on success. The
/// high nibbles encode "send 0 parameters, receive 1 byte".
pub const CMD_CTL_TEST: u16 = 0x01aa;
/// Keyboard-interface test command; returns `0x00` on success.
pub const CMD_KBD_TEST: u16 = 0x01ab;
/// The self-test success code.
pub const RET_CTL_TEST: u8 = 0x55;

fn read_status() -> u8 {
    // SAFETY: 0x64 is the i8042 status port.
    unsafe { inb(COMMAND_REG) }
}

/// `i8042_read_data()`: read one byte from the data port (0x60).
pub fn read_data() -> u8 {
    // SAFETY: 0x60 is the i8042 data port.
    unsafe { inb(DATA_REG) }
}

/// Writes one byte to the data port (0x60), which the controller forwards to
/// the first PS/2 device (the keyboard). Returns false on a timeout.
pub fn write_data(byte: u8) -> bool {
    if !wait_write() {
        return false;
    }
    // SAFETY: 0x60 is the data port; a byte with no command prefix goes to
    // the keyboard.
    unsafe { outb(byte, DATA_REG) };
    true
}

/// Whether the controller's output buffer holds a byte to read (`STR_OBF`).
pub fn output_full() -> bool {
    read_status() & STR_OBF != 0
}

/// `i8042_wait_read()`: wait until the output buffer is full.
fn wait_read() -> bool {
    for _ in 0..CTL_TIMEOUT {
        if read_status() & STR_OBF != 0 {
            return true;
        }
        native_io_delay();
    }
    false
}

/// `i8042_wait_write()`: wait until the input buffer is empty.
fn wait_write() -> bool {
    for _ in 0..CTL_TIMEOUT {
        if read_status() & STR_IBF == 0 {
            return true;
        }
        native_io_delay();
    }
    false
}

/// `i8042_flush()`: drain any stale bytes from the output buffer.
pub fn flush() {
    let mut i = 0;
    while read_status() & STR_OBF != 0 && i < 16 {
        native_io_delay();
        // SAFETY: reading the data port consumes one buffered byte.
        let _ = unsafe { inb(DATA_REG) };
        i += 1;
    }
}

/// `__i8042_command()`: send the command byte of `command`, then its encoded
/// number of parameter bytes from `param`, then read its encoded number of
/// response bytes back into `param`. Returns false on a timeout.
pub fn command(param: &mut [u8], command: u16) -> bool {
    if !wait_write() {
        return false;
    }
    // SAFETY: 0x64 is the command port.
    unsafe { outb((command & 0xff) as u8, COMMAND_REG) };

    let nsend = ((command >> 12) & 0xf) as usize;
    for p in param.iter().take(nsend) {
        if !wait_write() {
            return false;
        }
        // SAFETY: 0x60 is the data port.
        unsafe { outb(*p, DATA_REG) };
    }

    let nrecv = ((command >> 8) & 0xf) as usize;
    for p in param.iter_mut().take(nrecv) {
        if !wait_read() {
            return false;
        }
        // SAFETY: 0x60 is the data port; a byte is waiting.
        *p = unsafe { inb(DATA_REG) };
    }
    true
}

/// Runs the controller self-test, returning its response byte (`0x55` means
/// the controller passed), or `None` on timeout.
pub fn controller_selftest() -> Option<u8> {
    flush();
    let mut p = [0u8; 1];
    command(&mut p, CMD_CTL_TEST).then_some(p[0])
}

/// Runs the keyboard-interface test (`0x00` means the interface is good).
pub fn keyboard_test() -> Option<u8> {
    let mut p = [0u8; 1];
    command(&mut p, CMD_KBD_TEST).then_some(p[0])
}
