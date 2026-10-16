// SPDX-License-Identifier: GPL-2.0
//! A bounded text buffer that remembers when a write did not fit.
//!
//! Ported from `lib/seq_buf.c` and `include/linux/seq_buf.h`. The C struct
//! holds a pointer and a size; here the buffer is a borrowed slice. C's
//! overflow marker `len = size + 1` is kept, so [`SeqBuf::used`] and
//! [`SeqBuf::buffer_left`] behave as in C.
//!
//! Not ported, because they need parts of the kernel that are not in rinux
//! yet: `seq_buf_print_seq()` (seq_file), `seq_buf_do_printk()` (printk),
//! `seq_buf_bprintf()` (binary printf), `seq_buf_path()` (dentries) and
//! `seq_buf_to_user()` (user copies).

use crate::hexdump::hex_dump_to_buffer;
use core::fmt::{self, Write};

/// The write did not fit; the buffer is now marked overflowed. C returns -1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overflow;

/// `enum dump_prefix_type` for [`SeqBuf::hex_dump`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DumpPrefix {
    /// `DUMP_PREFIX_NONE`: no address before the line.
    None,
    /// `DUMP_PREFIX_ADDRESS`: the pointer of the line's first byte.
    Address,
    /// `DUMP_PREFIX_OFFSET`: the offset from the start, as `%.8x`.
    Offset,
}

/// `struct seq_buf`: a text buffer with a write position.
#[derive(Debug)]
pub struct SeqBuf<'a> {
    buffer: &'a mut [u8],
    len: usize,
}

/// Most bytes one `putmem_hex` group prints; `MAX_MEMHEX_BYTES` in C.
const MAX_MEMHEX_BYTES: usize = 8;

impl<'a> SeqBuf<'a> {
    /// `seq_buf_init()`: an empty buffer over `buffer`.
    pub fn new(buffer: &'a mut [u8]) -> Self {
        let mut s = Self { buffer, len: 0 };
        s.clear();
        s
    }

    /// `seq_buf_clear()`: forget the contents and NUL-terminate.
    pub fn clear(&mut self) {
        self.len = 0;
        if let Some(first) = self.buffer.first_mut() {
            *first = 0;
        }
    }

    /// The buffer's size in bytes.
    pub fn size(&self) -> usize {
        self.buffer.len()
    }

    /// `seq_buf_has_overflowed()`.
    pub fn has_overflowed(&self) -> bool {
        self.len > self.size()
    }

    fn set_overflow(&mut self) {
        self.len = self.size() + 1;
    }

    /// `seq_buf_used()`: bytes written, or the whole buffer after overflow.
    pub fn used(&self) -> usize {
        if self.has_overflowed() {
            self.size()
        } else {
            self.len
        }
    }

    /// `seq_buf_str()`: the written text. Lossy on bytes that are not UTF-8.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer[..self.used()]
    }

    /// `seq_buf_buffer_left()`: free bytes, zero after overflow.
    pub fn buffer_left(&self) -> usize {
        if self.has_overflowed() {
            0
        } else {
            self.size() - self.len
        }
    }

    fn can_fit(&self, len: usize) -> bool {
        self.len + len <= self.size()
    }

    /// `seq_buf_puts()`: append a string and a NUL, the NUL not counted.
    pub fn puts(&mut self, s: &str) -> Result<(), Overflow> {
        let need = s.len() + 1;
        if self.can_fit(need) {
            let at = self.len;
            self.buffer[at..at + s.len()].copy_from_slice(s.as_bytes());
            self.buffer[at + s.len()] = 0;
            self.len += s.len();
            return Ok(());
        }
        self.set_overflow();
        Err(Overflow)
    }

    /// `seq_buf_putc()`: append one byte.
    pub fn putc(&mut self, c: u8) -> Result<(), Overflow> {
        self.putmem(&[c])
    }

    /// `seq_buf_putmem()`: append raw bytes.
    pub fn putmem(&mut self, mem: &[u8]) -> Result<(), Overflow> {
        if self.can_fit(mem.len()) {
            let at = self.len;
            self.buffer[at..at + mem.len()].copy_from_slice(mem);
            self.len += mem.len();
            return Ok(());
        }
        self.set_overflow();
        Err(Overflow)
    }

    /// `seq_buf_putmem_hex()`: bytes as hex, eight at a time. Each group is
    /// printed least significant byte first, as on a little-endian machine,
    /// and followed by a space.
    pub fn putmem_hex(&mut self, mem: &[u8]) -> Result<(), Overflow> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for chunk in mem.chunks(MAX_MEMHEX_BYTES) {
            let mut hex = [0u8; MAX_MEMHEX_BYTES * 2 + 1];
            let mut j = 0;
            for &byte in chunk.iter().rev() {
                hex[j] = HEX[usize::from(byte >> 4)];
                hex[j + 1] = HEX[usize::from(byte & 0xf)];
                j += 2;
            }
            hex[j] = b' ';
            self.putmem(&hex[..j + 1])?;
        }
        Ok(())
    }

    /// `seq_buf_printf()`: format into the rest of the buffer. The text must
    /// leave room for its NUL, as in C.
    pub fn printf(&mut self, args: fmt::Arguments<'_>) -> Result<(), Overflow> {
        if self.has_overflowed() {
            return Err(Overflow);
        }
        let start = self.len;
        let room = self.size() - start;
        let mut w = Limited {
            out: &mut self.buffer[start..],
            written: 0,
            total: 0,
        };
        let _ = w.write_fmt(args);
        let total = w.total;
        if room > 0 {
            // `vsnprintf` NUL-terminates what it wrote.
            let end = start + w.written;
            self.buffer[end] = 0;
        }
        if start + total < self.size() {
            self.len = start + total;
            return Ok(());
        }
        self.set_overflow();
        Err(Overflow)
    }

    /// `seq_buf_hex_dump()`: `print_hex_dump()` into the buffer, one line per
    /// `rowsize` bytes (16 or 32; other values use 16).
    pub fn hex_dump(
        &mut self,
        prefix_str: &str,
        prefix_type: DumpPrefix,
        rowsize: i32,
        groupsize: i32,
        buf: &[u8],
        ascii: bool,
    ) -> Result<(), Overflow> {
        let rowsize = if rowsize != 16 && rowsize != 32 {
            16
        } else {
            rowsize as usize
        };
        let mut linebuf = [0u8; 32 * 3 + 2 + 32 + 1];
        let mut remaining = buf.len();
        for i in (0..buf.len()).step_by(rowsize) {
            let linelen = remaining.min(rowsize);
            remaining = remaining.saturating_sub(rowsize);
            hex_dump_to_buffer(
                &buf[i..i + linelen],
                rowsize as i32,
                groupsize,
                &mut linebuf,
                ascii,
            );
            let end = linebuf
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(linebuf.len());
            let line = core::str::from_utf8(&linebuf[..end]).unwrap_or("");
            match prefix_type {
                DumpPrefix::Address => {
                    let ptr = buf.as_ptr().wrapping_add(i);
                    self.printf(format_args!("{prefix_str}{ptr:p}: {line}\n"))?
                }
                DumpPrefix::Offset => self.printf(format_args!("{prefix_str}{i:08x}: {line}\n"))?,
                DumpPrefix::None => self.printf(format_args!("{prefix_str}{line}\n"))?,
            }
        }
        Ok(())
    }
}

/// A `fmt::Write` sink over a slice that keeps one byte for the NUL and
/// counts everything written, as `vsnprintf()`'s return value does.
struct Limited<'a> {
    out: &'a mut [u8],
    written: usize,
    total: usize,
}

impl Write for Limited<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            if self.written + 1 < self.out.len() {
                self.out[self.written] = b;
                self.written += 1;
            }
            self.total += 1;
        }
        Ok(())
    }
}
