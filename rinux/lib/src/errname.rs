// SPDX-License-Identifier: GPL-2.0
//! Map an errno value to its symbolic name, such as `EIO`.
//!
//! Ported from `lib/errname.c` and `include/linux/errname.h`. The C file
//! keeps two `static const char *` tables indexed by errno and guards them
//! with `BUILD_BUG_ON_ZERO`. Here the names come from the crate's errno
//! table, which already carries every name as a `stringify!` of its
//! identifier, so no second table is kept. The MIPS-only `EDQUOT` entry is
//! not ported because the crate targets x86-64.
//!
//! The C `errname(EIO)` returns `"EIO"` and `errname(-EIO)` returns
//! `"-EIO"`. Returning a `&str` would need a concatenation, so [`ErrName`]
//! carries the sign and writes it through [`fmt::Display`].

use crate::errno::Errno;
use core::fmt;

/// The result of [`errname`]: an errno name, optionally with a leading `-`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrName {
    errno: Errno,
    negative: bool,
}

impl ErrName {
    /// The bare name, as in `errname(EIO)` -> `EIO`.
    pub fn bare(self) -> &'static str {
        // Every errno in the table has a name, see `errno_table!`.
        self.errno.name().unwrap_or("")
    }
}

impl fmt::Display for ErrName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            f.write_str("-")?;
        }
        f.write_str(self.bare())
    }
}

/// `errname()`: the name of `err`, or of `-err`, with the sign kept.
///
/// Returns `None` for zero, for values outside `1..=MAX_ERRNO` and for
/// numbers the table does not know.
pub fn errname(err: i32) -> Option<ErrName> {
    // C takes `abs(err)`; `unsigned_abs` makes `i32::MIN` a clean miss
    // instead of undefined behaviour.
    let magnitude = i32::try_from(err.unsigned_abs()).ok()?;
    let errno = Errno::new(magnitude)?;
    errno.name()?;
    Some(ErrName {
        errno,
        negative: err < 0,
    })
}
