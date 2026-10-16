// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! Kernel error numbers.
//!
//! Ported from `include/uapi/asm-generic/errno-base.h`,
//! `include/uapi/asm-generic/errno.h`, `include/linux/errno.h` and the
//! errno parts of `include/linux/err.h`.
//!
//! C kernel functions return `-EINVAL` and friends in an `int`. rinux returns
//! `Err(EINVAL)` instead; [`Errno::to_errno`] gives back the negative C value
//! where an ABI (syscalls, the C kernel) needs it.

use core::fmt;
use core::num::NonZeroU16;

/// Largest error number that can be encoded in a pointer or syscall return.
pub const MAX_ERRNO: i32 = 4095;

/// Returns true if `x`, read as a syscall or `ERR_PTR` value, encodes an error.
///
/// This is `IS_ERR_VALUE()`: the top 4095 values of the address space.
#[inline]
pub const fn is_err_value(x: u64) -> bool {
    x >= (-(MAX_ERRNO as i64)) as u64
}

/// A kernel error number, always in `1..=MAX_ERRNO`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Errno(NonZeroU16);

/// `Result` with [`Errno`] as the error type.
pub type Result<T = (), E = Errno> = core::result::Result<T, E>;

impl Errno {
    const fn from_const(errno: u16) -> Errno {
        match NonZeroU16::new(errno) {
            Some(n) => Errno(n),
            None => panic!("errno 0 is not an error"),
        }
    }

    /// Builds an `Errno` from a positive error number such as `22`.
    pub const fn new(errno: i32) -> Option<Errno> {
        if errno < 1 || errno > MAX_ERRNO {
            return None;
        }
        Some(Errno::from_const(errno as u16))
    }

    /// Builds an `Errno` from a negative C return value such as `-22`.
    pub const fn from_errno(ret: i32) -> Option<Errno> {
        if ret >= 0 {
            return None;
        }
        Errno::new(ret.wrapping_neg())
    }

    /// The positive error number, e.g. `22` for `EINVAL`.
    #[inline]
    pub const fn get(self) -> i32 {
        self.0.get() as i32
    }

    /// The negative value C code returns, e.g. `-22` for `EINVAL`.
    #[inline]
    pub const fn to_errno(self) -> i32 {
        -self.get()
    }

    /// The symbolic name, e.g. `"EINVAL"`, if this is a known error number.
    pub fn name(self) -> Option<&'static str> {
        TABLE.iter().find(|e| e.0 == self).map(|e| e.1)
    }

    /// The description from the C header comment, if there is one.
    pub fn description(self) -> Option<&'static str> {
        TABLE
            .iter()
            .find(|e| e.0 == self)
            .map(|e| e.2)
            .filter(|d| !d.is_empty())
    }
}

impl fmt::Debug for Errno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "Errno({})", self.get()),
        }
    }
}

impl fmt::Display for Errno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.description() {
            Some(desc) => f.write_str(desc),
            None => write!(f, "Unknown error {}", self.get()),
        }
    }
}

macro_rules! errno_table {
    ($($name:ident = $value:literal, $desc:literal;)*) => {
        $(
            #[doc = concat!("`", stringify!($name), "` (", $value, ") ", $desc)]
            pub const $name: Errno = Errno::from_const($value);
        )*

        static TABLE: &[(Errno, &str, &str)] = &[$(($name, stringify!($name), $desc)),*];
    };
}

include!("errno_table.rs");
