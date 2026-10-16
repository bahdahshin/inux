// SPDX-License-Identifier: GPL-2.0
//! A lock-free single-producer single-consumer byte FIFO.
//!
//! A port of the byte-oriented core of `lib/kfifo.c` (the `__kfifo_in` /
//! `__kfifo_out` path and `kfifo_copy_in`/`kfifo_copy_out`). As in Linux the
//! `in` and `out` indices are free-running counters and the capacity is a
//! power of two, so the used count is `in - out` in modular arithmetic and
//! an index into the buffer is the counter masked with `capacity - 1`; a
//! write or read that wraps the end of the buffer is done as two copies.
//!
//! Simplified from Linux's: a fixed inline buffer (no `kfifo_alloc`), bytes
//! only (`esize == 1`, no record variants), and the single-producer/
//! single-consumer memory barriers are left implicit (rinux is one CPU).

/// A byte FIFO with a fixed capacity `N`, which must be a power of two.
pub struct Kfifo<const N: usize> {
    /// Producer counter (free-running).
    r#in: u32,
    /// Consumer counter (free-running).
    out: u32,
    data: [u8; N],
}

impl<const N: usize> Default for Kfifo<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Kfifo<N> {
    /// Creates an empty FIFO. Panics if `N` is not a power of two.
    pub const fn new() -> Kfifo<N> {
        assert!(N.is_power_of_two(), "kfifo capacity must be a power of two");
        Kfifo {
            r#in: 0,
            out: 0,
            data: [0; N],
        }
    }

    #[inline]
    const fn mask(&self) -> usize {
        N - 1
    }

    /// Number of bytes currently stored (`kfifo_len`).
    pub fn len(&self) -> usize {
        self.r#in.wrapping_sub(self.out) as usize
    }

    /// True if the FIFO holds no bytes.
    pub fn is_empty(&self) -> bool {
        self.r#in == self.out
    }

    /// True if the FIFO is full.
    pub fn is_full(&self) -> bool {
        self.len() == N
    }

    /// Free space in bytes (`kfifo_unused`).
    pub fn unused(&self) -> usize {
        N - self.len()
    }

    /// Empties the FIFO (`kfifo_reset`).
    pub fn reset(&mut self) {
        self.r#in = 0;
        self.out = 0;
    }

    /// `kfifo_copy_in`: write `len` bytes of `src` at counter position `off`,
    /// splitting the copy at the end of the buffer.
    fn copy_in(&mut self, src: &[u8], len: usize, off: u32) {
        let off = (off as usize) & self.mask();
        let l = len.min(N - off);
        self.data[off..off + l].copy_from_slice(&src[..l]);
        self.data[..len - l].copy_from_slice(&src[l..len]);
    }

    /// `kfifo_copy_out`: read `len` bytes into `dst` from counter position
    /// `off`, splitting the copy at the end of the buffer.
    fn copy_out(&self, dst: &mut [u8], len: usize, off: u32) {
        let off = (off as usize) & self.mask();
        let l = len.min(N - off);
        dst[..l].copy_from_slice(&self.data[off..off + l]);
        dst[l..len].copy_from_slice(&self.data[..len - l]);
    }

    /// `__kfifo_in`: enqueue up to `src.len()` bytes, returning how many were
    /// accepted (fewer than requested if the FIFO filled up).
    pub fn put(&mut self, src: &[u8]) -> usize {
        let len = src.len().min(self.unused());
        self.copy_in(src, len, self.r#in);
        self.r#in = self.r#in.wrapping_add(len as u32);
        len
    }

    /// `__kfifo_out_peek`: copy up to `dst.len()` bytes to `dst` without
    /// consuming them. Returns the number copied.
    pub fn peek(&self, dst: &mut [u8]) -> usize {
        let len = dst.len().min(self.len());
        self.copy_out(dst, len, self.out);
        len
    }

    /// `__kfifo_out`: dequeue up to `dst.len()` bytes, returning how many were
    /// removed.
    pub fn get(&mut self, dst: &mut [u8]) -> usize {
        let len = self.peek(dst);
        self.out = self.out.wrapping_add(len as u32);
        len
    }

    /// Enqueues one byte, returning false if the FIFO was full.
    pub fn put_byte(&mut self, b: u8) -> bool {
        self.put(&[b]) == 1
    }

    /// Dequeues one byte, or `None` if the FIFO was empty.
    pub fn get_byte(&mut self) -> Option<u8> {
        let mut b = [0u8; 1];
        if self.get(&mut b) == 1 {
            Some(b[0])
        } else {
            None
        }
    }
}
