// SPDX-License-Identifier: GPL-2.0
//! The user-memory copy primitives that the `*_from_user()` helpers need.
//!
//! Ported from the copy interface of `include/linux/uaccess.h` and the
//! callers in `lib/strncpy_from_user.c`, `lib/strnlen_user.c`,
//! `lib/usercopy.c` and `lib/kstrtox.c`.
//!
//! The C helpers call `copy_from_user()` and `copy_to_user()`, which return
//! the number of bytes they did not copy. Here that is the
//! [`UserMemory`] trait. The kernel implements it over the current task's
//! address space with the page-fault fixup; host tests use [`FlatUser`].

use crate::errno::{EFAULT, Result};
use crate::strncpy_from_user::strncpy_from_user;
use crate::strnlen_user::strnlen_user;
use crate::usercopy::check_zeroed_user;

/// The copy primitives of user memory. Addresses are offsets in the user
/// address space, as `usize`.
pub trait UserMemory {
    /// `copy_from_user()`: copies `dst.len()` bytes from user address `src`.
    /// Returns the number of bytes that were not copied, so 0 is success.
    fn copy_from_user(&self, dst: &mut [u8], src: usize) -> usize;

    /// `copy_to_user()`: copies `src` to user address `dst`. Returns the
    /// number of bytes that were not copied.
    fn copy_to_user(&mut self, dst: usize, src: &[u8]) -> usize;
}

/// Reads one user byte, or `None` if it faults. Used by the byte-wise
/// `*_from_user()` helpers.
pub fn get_user_byte(user: &impl UserMemory, addr: usize) -> Option<u8> {
    let mut b = [0u8];
    if user.copy_from_user(&mut b, addr) == 0 {
        Some(b[0])
    } else {
        None
    }
}

/// `copy_from_user()` as the C callers see it: `EFAULT` for any byte not
/// copied.
pub(crate) fn copy_in(user: &impl UserMemory, dst: &mut [u8], src: usize) -> Result<()> {
    if user.copy_from_user(dst, src) == 0 {
        Ok(())
    } else {
        Err(EFAULT)
    }
}

/// A flat user address space backed by a byte slice: user address `base + i`
/// is `mem[i]`. Anything outside the slice faults. A copy that starts inside
/// the slice and runs past its end copies the bytes that fit, as a fault part
/// way through a copy does in C.
#[derive(Debug)]
pub struct FlatUser<'a> {
    mem: &'a mut [u8],
    base: usize,
}

impl<'a> FlatUser<'a> {
    /// Maps `mem` at user address `base`.
    pub fn new(mem: &'a mut [u8], base: usize) -> Self {
        Self { mem, base }
    }

    /// The `(offset, len)` range of the mapping that a copy of `len` bytes at
    /// `addr` covers. Empty if `addr` is outside the mapping.
    fn span(&self, addr: usize, len: usize) -> (usize, usize) {
        let Some(off) = addr.checked_sub(self.base) else {
            return (0, 0);
        };
        if off >= self.mem.len() {
            return (0, 0);
        }
        (off, len.min(self.mem.len() - off))
    }
}

impl UserMemory for FlatUser<'_> {
    fn copy_from_user(&self, dst: &mut [u8], src: usize) -> usize {
        let (off, n) = self.span(src, dst.len());
        dst[..n].copy_from_slice(&self.mem[off..off + n]);
        dst.len() - n
    }

    fn copy_to_user(&mut self, dst: usize, src: &[u8]) -> usize {
        let (off, n) = self.span(dst, src.len());
        self.mem[off..off + n].copy_from_slice(&src[..n]);
        src.len() - n
    }
}

/// `strncpy_from_user()` over user memory. See [`strncpy_from_user`] for the
/// rules; this only supplies the bytes.
pub fn strncpy_from_user_mem(
    user: &impl UserMemory,
    dst: &mut [u8],
    count: usize,
    src: usize,
) -> Result<usize> {
    strncpy_from_user(dst, count, |i| get_user_byte(user, src + i))
}

/// `strnlen_user()` over user memory. See [`strnlen_user`] for the rules.
pub fn strnlen_user_mem(user: &impl UserMemory, count: i64, src: usize) -> i64 {
    strnlen_user(count, |i| get_user_byte(user, src + i))
}

/// `check_zeroed_user()` over user memory. See [`check_zeroed_user`].
pub fn check_zeroed_user_mem(user: &impl UserMemory, size: usize, src: usize) -> i32 {
    check_zeroed_user(size, |i| get_user_byte(user, src + i))
}

/// The copy `kstrto*_from_user()` makes: at most `N - 1` bytes, then a NUL.
/// Returns the buffer and the length of the string, NUL included, as C passes
/// it to the parser.
pub(crate) fn kstrto_copy<const N: usize>(
    user: &impl UserMemory,
    src: usize,
    count: usize,
) -> Result<([u8; N], usize)> {
    let mut buf = [0u8; N];
    let n = count.min(N - 1);
    copy_in(user, &mut buf[..n], src)?;
    buf[n] = 0;
    Ok((buf, n + 1))
}

/// The buffer sizes `kstrto*_from_user()` uses: `1 + bits + 1 + 1` for the
/// integer types, which covers a sign, the base-2 digits, a newline and a
/// NUL.
pub(crate) const KSTRTO64_BUF: usize = 1 + 64 + 1 + 1;
pub(crate) const KSTRTO32_BUF: usize = 1 + 32 + 1 + 1;
pub(crate) const KSTRTO16_BUF: usize = 1 + 16 + 1 + 1;
pub(crate) const KSTRTO8_BUF: usize = 1 + 8 + 1 + 1;
/// `kstrtobool_from_user()`: longest word needed to tell booleans apart.
pub(crate) const KSTRTOBOOL_BUF: usize = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_user_copies_inside_and_faults_outside() {
        let mut mem = *b"hello";
        let user = FlatUser::new(&mut mem, 0x100);
        let mut dst = [0u8; 3];
        assert_eq!(user.copy_from_user(&mut dst, 0x101), 0);
        assert_eq!(&dst, b"ell");
        let mut dst = [0u8; 2];
        assert_eq!(user.copy_from_user(&mut dst, 0x200), 2);
    }

    #[test]
    fn a_copy_running_off_the_end_reports_the_rest() {
        let mut mem = *b"abc";
        let user = FlatUser::new(&mut mem, 0);
        let mut dst = [0u8; 5];
        assert_eq!(user.copy_from_user(&mut dst, 1), 3);
        assert_eq!(&dst[..2], b"bc");
    }

    #[test]
    fn copy_to_user_writes_and_reports_the_rest() {
        let mut mem = [0u8; 4];
        let mut user = FlatUser::new(&mut mem, 0x10);
        assert_eq!(user.copy_to_user(0x12, b"xy"), 0);
        assert_eq!(user.copy_to_user(0x13, b"zz"), 1);
        assert_eq!(mem, [0, 0, b'x', b'z']);
    }

    #[test]
    fn get_user_byte_faults_past_the_mapping() {
        let mut mem = *b"a";
        let user = FlatUser::new(&mut mem, 0);
        assert_eq!(get_user_byte(&user, 0), Some(b'a'));
        assert_eq!(get_user_byte(&user, 1), None);
    }
}
