// SPDX-License-Identifier: GPL-2.0
//! The AT/PS2 keyboard driver.
//!
//! A small port of the input path of `drivers/input/keyboard/atkbd.c` reached
//! through `drivers/input/serio/i8042.c`: request IRQ 1, and on each keyboard
//! interrupt read the scancode from the i8042 data port and translate it to
//! ASCII through a US keymap, queueing it for a reader.
//!
//! Simplified heavily from Linux's: scancode set 1 only (the translated set
//! the firmware leaves the controller in), no `serio`/`input` subsystem, no
//! modifier/shift state, no key repeat and no LED or typematic programming --
//! just enough to prove an interrupt-driven keystroke reaches the kernel.

use crate::drivers::input::serio::i8042;
use crate::kernel::irq::{IRQ_HANDLED, IrqReturn, request_irq};
use crate::kernel::locking::SpinLock;

/// Scancode-set-1 make codes to ASCII (0 = no mapping). A break code has bit
/// 0x80 set and is handled before this table is consulted.
const KEYMAP: [u8; 128] = {
    let mut m = [0u8; 128];
    m[0x01] = 0x1b; // Esc
    m[0x02] = b'1';
    m[0x03] = b'2';
    m[0x04] = b'3';
    m[0x05] = b'4';
    m[0x06] = b'5';
    m[0x07] = b'6';
    m[0x08] = b'7';
    m[0x09] = b'8';
    m[0x0a] = b'9';
    m[0x0b] = b'0';
    m[0x0c] = b'-';
    m[0x0d] = b'=';
    m[0x0e] = 0x08; // Backspace
    m[0x0f] = b'\t';
    m[0x10] = b'q';
    m[0x11] = b'w';
    m[0x12] = b'e';
    m[0x13] = b'r';
    m[0x14] = b't';
    m[0x15] = b'y';
    m[0x16] = b'u';
    m[0x17] = b'i';
    m[0x18] = b'o';
    m[0x19] = b'p';
    m[0x1a] = b'[';
    m[0x1b] = b']';
    m[0x1c] = b'\n'; // Enter
    m[0x1e] = b'a';
    m[0x1f] = b's';
    m[0x20] = b'd';
    m[0x21] = b'f';
    m[0x22] = b'g';
    m[0x23] = b'h';
    m[0x24] = b'j';
    m[0x25] = b'k';
    m[0x26] = b'l';
    m[0x27] = b';';
    m[0x28] = b'\'';
    m[0x29] = b'`';
    m[0x2b] = b'\\';
    m[0x2c] = b'z';
    m[0x2d] = b'x';
    m[0x2e] = b'c';
    m[0x2f] = b'v';
    m[0x30] = b'b';
    m[0x31] = b'n';
    m[0x32] = b'm';
    m[0x33] = b',';
    m[0x34] = b'.';
    m[0x35] = b'/';
    m[0x37] = b'*';
    m[0x39] = b' '; // Space
    m
};

/// A tiny queue of raw scancodes filled by the interrupt handler.
struct ScancodeQueue {
    buf: [u8; 32],
    head: usize,
    tail: usize,
}

static QUEUE: SpinLock<ScancodeQueue> = SpinLock::new(ScancodeQueue {
    buf: [0; 32],
    head: 0,
    tail: 0,
});

/// `atkbd_interrupt()`: the IRQ 1 handler. Reads the pending scancode and
/// queues it; the queue drops bytes when full (a reader is not keeping up).
fn keyboard_interrupt(_irq: u32) -> IrqReturn {
    let code = i8042::read_data();
    let mut q = QUEUE.lock();
    let next = (q.head + 1) % q.buf.len();
    if next != q.tail {
        let h = q.head;
        q.buf[h] = code;
        q.head = next;
    }
    IRQ_HANDLED
}

/// Pops one raw scancode from the queue, or `None` if it is empty.
pub fn read_scancode() -> Option<u8> {
    let mut q = QUEUE.lock();
    if q.head == q.tail {
        return None;
    }
    let t = q.tail;
    let code = q.buf[t];
    q.tail = (t + 1) % q.buf.len();
    Some(code)
}

/// Translates a scancode-set-1 make code to ASCII, or `None` for a break
/// code (bit 0x80) or an unmapped key.
pub fn scancode_to_ascii(code: u8) -> Option<u8> {
    if code & 0x80 != 0 {
        return None; // key release
    }
    let c = KEYMAP[(code & 0x7f) as usize];
    if c == 0 { None } else { Some(c) }
}

/// i8042 config-byte bits (`drivers/input/serio/i8042.h`): first-port
/// interrupt, first-port clock disable, and first-port translation.
const CFG_INT1: u8 = 0x01;
const CFG_CLK1_DISABLE: u8 = 0x10;
const CFG_TRANSLATE: u8 = 0x40;

/// `atkbd_connect()`: reprogram the i8042 for keyboard input and hook up
/// IRQ 1 so keystrokes start arriving.
pub fn kbd_init() {
    // The controller self-test run earlier reset the config byte, disabling
    // the keyboard interrupt and port. Re-enable the first PS/2 port, then set
    // the config byte so it raises IRQ 1, keeps the port clock running and
    // translates scancodes to set 1.
    i8042::command(&mut [], 0x00ae); // enable first PS/2 port
    let mut cfg = [0u8; 1];
    if i8042::command(&mut cfg, 0x0120) {
        // cmd 0x20: read config byte
        cfg[0] = (cfg[0] | CFG_INT1 | CFG_TRANSLATE) & !CFG_CLK1_DISABLE;
        i8042::command(&mut cfg, 0x1060); // cmd 0x60: write config byte
    }
    // Tell the keyboard to start scanning (0xf4 -> ACK 0xfa).
    i8042::write_data(0xf4);

    // Drop any bytes left in the controller (the ACK, stale keys).
    while i8042::output_full() {
        let _ = i8042::read_data();
    }
    if request_irq(1, keyboard_interrupt, "keyboard").is_err() {
        pr_warn!("rinux: keyboard: could not request IRQ 1");
    }
}
