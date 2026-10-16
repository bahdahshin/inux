// SPDX-License-Identifier: GPL-2.0
//! The kernel log and consoles.
//!
//! A partial port of `kernel/printk/printk.c`, with the log levels of
//! `include/linux/kern_levels.h`, the `pr_*()` macros of
//! `include/linux/printk.h` and the console flags of
//! `include/linux/console.h`.
//!
//! Every message goes into a ring buffer of records and to each enabled
//! console whose level allows it. A console registered with
//! `CON_PRINTBUFFER` first gets everything already in the buffer, which is
//! how messages printed before the early console existed still appear.
//!
//! Timestamps come from `local_clock()`, which reads 0 until the TSC is
//! calibrated, as in Linux.
//!
//! Not yet ported: the lockless `printk_ringbuffer`, `/dev/kmsg` and
//! `dmesg`, deferred and threaded printing, and `pr_cont()`; callers build
//! continued lines in a [`FmtBuf`] instead.

use core::fmt::{self, Write};
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use super::locking::SpinLock;

// include/linux/kern_levels.h
pub const LOGLEVEL_DEFAULT: i32 = -1;
pub const LOGLEVEL_EMERG: i32 = 0;
pub const LOGLEVEL_ALERT: i32 = 1;
pub const LOGLEVEL_CRIT: i32 = 2;
pub const LOGLEVEL_ERR: i32 = 3;
pub const LOGLEVEL_WARNING: i32 = 4;
pub const LOGLEVEL_NOTICE: i32 = 5;
pub const LOGLEVEL_INFO: i32 = 6;
pub const LOGLEVEL_DEBUG: i32 = 7;

// include/linux/printk.h, with the default Kconfig values.
pub const MESSAGE_LOGLEVEL_DEFAULT: i32 = 4;
pub const CONSOLE_LOGLEVEL_SILENT: i32 = 0;
pub const CONSOLE_LOGLEVEL_MIN: i32 = 1;
pub const CONSOLE_LOGLEVEL_DEBUG: i32 = 10;
pub const CONSOLE_LOGLEVEL_MOTORMOUTH: i32 = 15;
pub const CONSOLE_LOGLEVEL_DEFAULT: i32 = 7;
pub const CONSOLE_LOGLEVEL_QUIET: i32 = 4;

/// `console_loglevel`: messages below this level reach the consoles.
pub static CONSOLE_LOGLEVEL: AtomicI32 = AtomicI32::new(CONSOLE_LOGLEVEL_DEFAULT);

// include/linux/console.h
pub const CON_PRINTBUFFER: u32 = 1 << 0;
pub const CON_CONSDEV: u32 = 1 << 1;
pub const CON_ENABLED: u32 = 1 << 2;
pub const CON_BOOT: u32 = 1 << 3;
pub const CON_ANYTIME: u32 = 1 << 4;

/// `struct console`: somewhere to write kernel messages.
pub struct Console {
    pub name: &'static str,
    /// Writes raw bytes; lines end in `\n`.
    pub write: fn(&[u8]),
    pub flags: AtomicU32,
    pub index: AtomicI32,
}

impl Console {
    /// A console with `index` -1, meaning "not registered yet".
    pub const fn new(name: &'static str, write: fn(&[u8]), flags: u32) -> Console {
        Console {
            name,
            write,
            flags: AtomicU32::new(flags),
            index: AtomicI32::new(-1),
        }
    }

    fn flags(&self) -> u32 {
        self.flags.load(Ordering::Relaxed)
    }
}

/// `CONFIG_LOG_BUF_SHIFT` = 17: a 128 KiB log.
const LOG_BUF_LEN: usize = 1 << 17;
/// `PRINTKRB_RECORD_MAX`: longest message text.
const PRINTKRB_RECORD_MAX: usize = 1024;
/// Level byte plus a 16-bit length in front of each record.
const RECORD_HEADER: usize = 3;
const MAX_CONSOLES: usize = 8;

/// The log: a byte ring of `[level][len lo][len hi][text]` records. When
/// it fills up, the oldest records are dropped.
struct LogBuf {
    buf: [u8; LOG_BUF_LEN],
    head: usize,
    used: usize,
}

impl LogBuf {
    const fn new() -> LogBuf {
        LogBuf {
            buf: [0; LOG_BUF_LEN],
            head: 0,
            used: 0,
        }
    }

    fn byte(&self, i: usize) -> u8 {
        self.buf[(self.head + i) % LOG_BUF_LEN]
    }

    fn record_len(&self, at: usize) -> usize {
        RECORD_HEADER + (self.byte(at + 1) as usize | (self.byte(at + 2) as usize) << 8)
    }

    fn store(&mut self, level: i32, text: &[u8]) {
        let need = RECORD_HEADER + text.len();
        while self.used + need > LOG_BUF_LEN {
            let oldest = self.record_len(0);
            self.head = (self.head + oldest) % LOG_BUF_LEN;
            self.used -= oldest;
        }
        let len = text.len() as u16;
        let mut pos = (self.head + self.used) % LOG_BUF_LEN;
        for &b in [level as u8, len as u8, (len >> 8) as u8]
            .iter()
            .chain(text)
        {
            self.buf[pos] = b;
            pos = (pos + 1) % LOG_BUF_LEN;
        }
        self.used += need;
    }

    /// Calls `f(level, text)` for each record, oldest first.
    fn for_each(&self, mut f: impl FnMut(i32, &[u8])) {
        let mut text = [0u8; PRINTKRB_RECORD_MAX];
        let mut at = 0;
        while at < self.used {
            let len = self.record_len(at) - RECORD_HEADER;
            for (i, t) in text[..len].iter_mut().enumerate() {
                *t = self.byte(at + RECORD_HEADER + i);
            }
            f(self.byte(at) as i32, &text[..len]);
            at += RECORD_HEADER + len;
        }
    }
}

struct Printk {
    log: LogBuf,
    consoles: [Option<&'static Console>; MAX_CONSOLES],
}

static PRINTK: SpinLock<Printk> = SpinLock::new(Printk {
    log: LogBuf::new(),
    consoles: [None; MAX_CONSOLES],
});

/// Formats into a fixed buffer, truncating like `vsnprintf()`.
struct LineBuf {
    buf: [u8; PRINTKRB_RECORD_MAX],
    len: usize,
}

impl Write for LineBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let n = s.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// `local_clock()` in nanoseconds since boot.
fn local_clock() -> u64 {
    crate::kernel::sched::clock::local_clock()
}

/// `suppress_message_printing()`.
fn suppress_message_printing(level: i32) -> bool {
    level >= CONSOLE_LOGLEVEL.load(Ordering::Relaxed)
}

/// Writes one record to one console, each line prefixed with the time as
/// `print_time()` formats it.
fn console_emit(con: &Console, text: &[u8]) {
    let ts = local_clock();
    let (secs, usecs) = (ts / 1_000_000_000, ts % 1_000_000_000 / 1000);
    for line in text.split(|&b| b == b'\n') {
        let mut out = LineBuf {
            buf: [0; PRINTKRB_RECORD_MAX],
            len: 0,
        };
        let _ = write!(out, "[{secs:5}.{usecs:06}] ");
        let room = out.buf.len() - out.len - 1;
        let n = line.len().min(room);
        out.buf[out.len..out.len + n].copy_from_slice(&line[..n]);
        out.len += n;
        out.buf[out.len] = b'\n';
        (con.write)(&out.buf[..out.len + 1]);
    }
}

/// `vprintk_emit()`: logs a message and prints it on the consoles.
pub fn vprintk_emit(level: i32, args: fmt::Arguments<'_>) {
    let mut line = LineBuf {
        buf: [0; PRINTKRB_RECORD_MAX],
        len: 0,
    };
    let _ = line.write_fmt(args);
    let mut text = &line.buf[..line.len];
    // A trailing newline ends the record rather than being part of it.
    if let Some(stripped) = text.strip_suffix(b"\n") {
        text = stripped;
    }
    let level = if level == LOGLEVEL_DEFAULT {
        MESSAGE_LOGLEVEL_DEFAULT
    } else {
        level
    };

    let mut p = PRINTK.lock_irqsave();
    p.log.store(level, text);
    if !suppress_message_printing(level) {
        for con in p.consoles.iter().flatten() {
            if con.flags() & CON_ENABLED != 0 {
                console_emit(con, text);
            }
        }
    }
}

/// Prints a C string (a byte slice) the way `%s` does. Bytes that are not
/// printable ASCII are shown as `\xNN`.
pub struct BStr<'a>(pub &'a [u8]);

impl fmt::Display for BStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &b in &self.0[..rinux_lib::string::strlen(self.0)] {
            if b.is_ascii_graphic() || b == b' ' {
                f.write_char(b as char)?;
            } else {
                write!(f, "\\x{b:02x}")?;
            }
        }
        Ok(())
    }
}

/// A fixed-size buffer to build one line in pieces, standing in for
/// `pr_cont()`. Output past `N` bytes is dropped.
pub struct FmtBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> FmtBuf<N> {
    pub const fn new() -> FmtBuf<N> {
        FmtBuf {
            buf: [0; N],
            len: 0,
        }
    }

    /// The text so far.
    pub fn as_str(&self) -> &str {
        // Only whole &strs are appended, but truncation can split a
        // character; keep the valid prefix.
        match core::str::from_utf8(&self.buf[..self.len]) {
            Ok(s) => s,
            Err(e) => core::str::from_utf8(&self.buf[..e.valid_up_to()]).unwrap_or(""),
        }
    }
}

impl<const N: usize> Default for FmtBuf<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Write for FmtBuf<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let n = s.len().min(N - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

impl<const N: usize> fmt::Display for FmtBuf<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `console_verbose()`: on oops, let every message through.
pub fn console_verbose() {
    if CONSOLE_LOGLEVEL.load(Ordering::Relaxed) != 0 {
        CONSOLE_LOGLEVEL.store(CONSOLE_LOGLEVEL_MOTORMOUTH, Ordering::Relaxed);
    }
}

/// `no_printk()`: type-checks the arguments and prints nothing.
#[inline(always)]
pub fn no_printk(_args: fmt::Arguments<'_>) {}

/// Writes raw bytes to every enabled console, without the log or a
/// timestamp. Used by the tty for user-space `write()`.
pub fn console_write_raw(bytes: &[u8]) {
    let p = PRINTK.lock_irqsave();
    for con in p.consoles.iter().flatten() {
        if con.flags() & CON_ENABLED != 0 {
            (con.write)(bytes);
        }
    }
}

/// `register_console()`: adds a console, replaying the log to it if it has
/// `CON_PRINTBUFFER`.
pub fn register_console(con: &'static Console) {
    {
        let mut p = PRINTK.lock_irqsave();
        if p.consoles.iter().flatten().any(|c| core::ptr::eq(*c, con)) {
            return;
        }
        let Some(slot) = p.consoles.iter_mut().find(|c| c.is_none()) else {
            return;
        };
        *slot = Some(con);
        if con.index.load(Ordering::Relaxed) < 0 {
            con.index.store(0, Ordering::Relaxed);
        }
        con.flags.fetch_or(CON_ENABLED, Ordering::Relaxed);
        if con.flags() & CON_PRINTBUFFER != 0 {
            p.log.for_each(|level, text| {
                if !suppress_message_printing(level) {
                    console_emit(con, text);
                }
            });
        }
    }
    // con_printk() from kernel/printk/internal.h.
    crate::pr_info!(
        "printk: legacy {}console [{}{}] enabled",
        if con.flags() & CON_BOOT != 0 {
            "boot"
        } else {
            ""
        },
        con.name,
        con.index.load(Ordering::Relaxed)
    );
}

/// `bust_spinlocks()`: on panic, make sure printing can't deadlock on a
/// lock held by the code that panicked.
pub fn bust_spinlocks() {
    // SAFETY: called only on the panic path with interrupts off; whoever
    // held the lock will never run again.
    unsafe { PRINTK.force_unlock() };
}

/// `printk(level, fmt, ...)`.
#[macro_export]
macro_rules! printk {
    ($level:expr, $($arg:tt)*) => {
        $crate::kernel::printk::vprintk_emit($level, format_args!($($arg)*))
    };
}

/// `pr_emerg()`: the system is unusable.
#[macro_export]
macro_rules! pr_emerg {
    ($($arg:tt)*) => { $crate::printk!($crate::kernel::printk::LOGLEVEL_EMERG, $($arg)*) };
}

/// `pr_alert()`: action must be taken immediately.
#[macro_export]
macro_rules! pr_alert {
    ($($arg:tt)*) => { $crate::printk!($crate::kernel::printk::LOGLEVEL_ALERT, $($arg)*) };
}

/// `pr_crit()`: critical conditions.
#[macro_export]
macro_rules! pr_crit {
    ($($arg:tt)*) => { $crate::printk!($crate::kernel::printk::LOGLEVEL_CRIT, $($arg)*) };
}

/// `pr_err()`: error conditions.
#[macro_export]
macro_rules! pr_err {
    ($($arg:tt)*) => { $crate::printk!($crate::kernel::printk::LOGLEVEL_ERR, $($arg)*) };
}

/// `pr_warn()`: warning conditions.
#[macro_export]
macro_rules! pr_warn {
    ($($arg:tt)*) => { $crate::printk!($crate::kernel::printk::LOGLEVEL_WARNING, $($arg)*) };
}

/// `pr_notice()`: normal but significant conditions.
#[macro_export]
macro_rules! pr_notice {
    ($($arg:tt)*) => { $crate::printk!($crate::kernel::printk::LOGLEVEL_NOTICE, $($arg)*) };
}

/// `pr_info()`: informational.
#[macro_export]
macro_rules! pr_info {
    ($($arg:tt)*) => { $crate::printk!($crate::kernel::printk::LOGLEVEL_INFO, $($arg)*) };
}

/// `pr_debug()`: compiled out, as without `DEBUG` or dynamic debug.
#[macro_export]
macro_rules! pr_debug {
    ($($arg:tt)*) => { $crate::kernel::printk::no_printk(format_args!($($arg)*)) };
}
