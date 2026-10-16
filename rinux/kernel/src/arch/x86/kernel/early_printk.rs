// SPDX-License-Identifier: GPL-2.0
//! Early consoles: `earlyprintk=serial` and `earlyprintk=vga`.
//!
//! A partial port of `arch/x86/kernel/early_printk.c`. Not yet ported:
//! the `mmio32`, `pciserial`, `dbgp`, `xen` and `xdbc` consoles.

use core::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use rinux_lib::string::{strcspn, strncmp, strstr};
use rinux_lib::vsprintf::simple_strtoull;

use super::setup::boot_info;
use crate::arch::x86::boot::multiboot2::FRAMEBUFFER_TYPE_EGA_TEXT;
use crate::arch::x86::include::io::{inb, outb};
use crate::kernel::locking::SpinLock;
use crate::kernel::printk::{CON_BOOT, CON_PRINTBUFFER, Console, register_console};

fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

fn tail(s: &[u8], i: usize) -> &[u8] {
    s.get(i..).unwrap_or(&[])
}

// Simple VGA output.

struct Vga {
    base: usize,
    max_ypos: usize,
    max_xpos: usize,
    current_ypos: usize,
    current_xpos: usize,
}

/// `VGABASE`: the text buffer, identity-mapped.
const VGABASE: usize = 0xb8000;

static VGA: SpinLock<Vga> = SpinLock::new(Vga {
    base: VGABASE,
    max_ypos: 25,
    max_xpos: 80,
    current_ypos: 25,
    current_xpos: 0,
});

fn writew(val: u16, addr: usize) {
    // SAFETY: addr is inside the VGA text buffer, which is identity-mapped
    // MMIO that nothing else uses.
    unsafe { (addr as *mut u16).write_volatile(val) }
}

fn readw(addr: usize) -> u16 {
    // SAFETY: as for writew.
    unsafe { (addr as *const u16).read_volatile() }
}

fn early_vga_write(s: &[u8]) {
    let mut v = VGA.lock();
    let (base, max_x, max_y) = (v.base, v.max_xpos, v.max_ypos);
    for &c in s.iter().take_while(|&&c| c != 0) {
        if v.current_ypos >= max_y {
            // Scroll one line up.
            for k in 1..max_y {
                for i in 0..max_x {
                    writew(
                        readw(base + 2 * (max_x * k + i)),
                        base + 2 * (max_x * (k - 1) + i),
                    );
                }
            }
            for i in 0..max_x {
                writew(0x720, base + 2 * (max_x * (max_y - 1) + i));
            }
            v.current_ypos = max_y - 1;
        }
        if c == b'\n' {
            v.current_xpos = 0;
            v.current_ypos += 1;
        } else if c != b'\r' {
            writew(
                (0x7 << 8) | c as u16,
                base + 2 * (max_x * v.current_ypos + v.current_xpos),
            );
            v.current_xpos += 1;
            if v.current_xpos >= max_x {
                v.current_xpos = 0;
                v.current_ypos += 1;
            }
        }
    }
}

static EARLY_VGA_CONSOLE: Console = Console::new("earlyvga", early_vga_write, CON_PRINTBUFFER);

// Serial functions loosely based on a similar package from Klaus P. Gerlicher.

/// ttyS0.
static EARLY_SERIAL_BASE: AtomicU16 = AtomicU16::new(0x3f8);

const XMTRDY: u8 = 0x20;
const DLAB: u8 = 0x80;

/// Transmit register (write).
const TXR: u16 = 0;
/// Interrupt enable.
const IER: u16 = 1;
/// FIFO control.
const FCR: u16 = 2;
/// Line control.
const LCR: u16 = 3;
/// Modem control.
const MCR: u16 = 4;
/// Line status.
const LSR: u16 = 5;
/// Divisor latch low.
const DLL: u16 = 0;
/// Divisor latch high.
const DLH: u16 = 1;

fn serial_in(offset: u16) -> u8 {
    // SAFETY: the early serial port belongs to this console.
    unsafe { inb(EARLY_SERIAL_BASE.load(Ordering::Relaxed) + offset) }
}

fn serial_out(offset: u16, value: u8) {
    // SAFETY: the early serial port belongs to this console.
    unsafe { outb(value, EARLY_SERIAL_BASE.load(Ordering::Relaxed) + offset) }
}

fn early_serial_putc(ch: u8) -> i32 {
    let mut timeout: u32 = 0xffff;
    while serial_in(LSR) & XMTRDY == 0 && {
        timeout -= 1;
        timeout != 0
    } {
        core::hint::spin_loop();
    }
    serial_out(TXR, ch);
    if timeout != 0 { 0 } else { -1 }
}

fn early_serial_write(s: &[u8]) {
    for &c in s.iter().take_while(|&&c| c != 0) {
        if c == b'\n' {
            early_serial_putc(b'\r');
        }
        early_serial_putc(c);
    }
}

fn early_serial_hw_init(divisor: u32) {
    serial_out(LCR, 0x3); // 8n1
    serial_out(IER, 0); // no interrupt
    serial_out(FCR, 0); // no fifo
    serial_out(MCR, 0x3); // DTR + RTS

    let c = serial_in(LCR);
    serial_out(LCR, c | DLAB);
    serial_out(DLL, (divisor & 0xff) as u8);
    serial_out(DLH, ((divisor >> 8) & 0xff) as u8);
    serial_out(LCR, c & !DLAB);
}

const DEFAULT_BAUD: u64 = 9600;

/// `early_serial_init()`: parses `[,][ttyS<n>|0x<port>][,<baud>]`.
fn early_serial_init(mut s: &[u8]) {
    let mut baud = DEFAULT_BAUD;

    if at(s, 0) == b',' {
        s = &s[1..];
    }

    if at(s, 0) != 0 {
        if strncmp(s, b"0x", 2) == 0 {
            let (port, _) = simple_strtoull(s, 16);
            EARLY_SERIAL_BASE.store(port as u16, Ordering::Relaxed);
        } else {
            const BASES: [u16; 2] = [0x3f8, 0x2f8];
            if strncmp(s, b"ttyS", 4) == 0 {
                s = &s[4..];
            }
            let (mut port, consumed) = simple_strtoull(s, 10);
            if port > 1 || consumed == 0 {
                port = 0;
            }
            EARLY_SERIAL_BASE.store(BASES[port as usize], Ordering::Relaxed);
        }
        s = tail(s, strcspn(s, b","));
        if at(s, 0) == b',' {
            s = &s[1..];
        }
    }

    if at(s, 0) != 0 {
        let (b, consumed) = simple_strtoull(s, 0);
        baud = if b == 0 || consumed == 0 {
            DEFAULT_BAUD
        } else {
            b
        };
    }

    // Convert from baud to divisor value.
    let divisor = (115_200 / baud) as u32;

    // Set up the hardware.
    early_serial_hw_init(divisor);
}

static EARLY_SERIAL_CONSOLE: Console =
    Console::new("earlyser", early_serial_write, CON_PRINTBUFFER);

/// `early_console`: set once an early console is registered.
static EARLY_CONSOLE: AtomicBool = AtomicBool::new(false);

fn early_console_register(con: &'static Console, keep_early: bool) {
    if con.index.load(Ordering::Relaxed) != -1 {
        pr_crit!("ERROR: earlyprintk= {} already used", con.name);
        return;
    }
    EARLY_CONSOLE.store(true, Ordering::Relaxed);
    if keep_early {
        con.flags.fetch_and(!CON_BOOT, Ordering::Relaxed);
    } else {
        con.flags.fetch_or(CON_BOOT, Ordering::Relaxed);
    }
    register_console(con);
}

/// `setup_early_printk()`: the `earlyprintk=` handler.
///
/// Like the C version, it scans the whole value and registers a console
/// for each keyword it finds, so `serial,ttyS0,115200,vga` gives both.
fn setup_early_printk(buf: Option<&[u8]>) -> i32 {
    let Some(buf) = buf else { return 0 };
    if EARLY_CONSOLE.load(Ordering::Relaxed) {
        return 0;
    }

    let keep = strstr(buf, b"keep").is_some();
    let mut i = 0;
    while at(buf, i) != 0 {
        if strncmp(tail(buf, i), b"serial", 6) == 0 {
            i += 6;
            early_serial_init(tail(buf, i));
            early_console_register(&EARLY_SERIAL_CONSOLE, keep);
            if strncmp(tail(buf, i), b",ttyS", 5) == 0 {
                i += 5;
            }
        }
        if strncmp(tail(buf, i), b"ttyS", 4) == 0 {
            early_serial_init(tail(buf, i + 4));
            early_console_register(&EARLY_SERIAL_CONSOLE, keep);
        }
        if strncmp(tail(buf, i), b"vga", 3) == 0 {
            // Linux checks boot_params.screen_info.orig_video_isVGA; the
            // multiboot2 equivalent is an EGA text framebuffer.
            if let Some(fb) = boot_info().and_then(|bi| bi.framebuffer())
                && fb.typ == FRAMEBUFFER_TYPE_EGA_TEXT
            {
                let mut v = VGA.lock();
                v.base = fb.addr as usize;
                v.max_xpos = fb.width as usize;
                v.max_ypos = fb.height as usize;
                // Linux continues below the BIOS cursor row (orig_y);
                // multiboot2 doesn't report it, so start at the bottom and
                // scroll, as the C default of 25 does.
                v.current_ypos = v.max_ypos;
                drop(v);
                early_console_register(&EARLY_VGA_CONSOLE, keep);
            }
        }
        i += 1;
    }
    0
}

crate::early_param!("earlyprintk", setup_early_printk);
