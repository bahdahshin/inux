// SPDX-License-Identifier: GPL-2.0-or-later
//! The `MPI` object: a signed multi-precision number whose limbs are a
//! caller-owned slice, with the comparison functions of `lib/crypto/mpi/`.
//!
//! Ported from `struct gcry_mpi` in `include/linux/mpi.h`, `mpi_normalize()`
//! from `mpi-bit.c`, and `mpi_cmp()` and `mpi_cmp_ui()` from `mpi-cmp.c`. The
//! C `alloced` count and flags are not needed: the slice gives the capacity,
//! and secure memory does not exist here. Limbs are least significant first.

use crate::crypto::mpi::mpihelp_cmp;

/// `struct gcry_mpi`, with the limbs borrowed from the caller.
#[derive(Debug)]
pub struct Mpi<'a> {
    /// `d`: the limbs, least significant first. Only `nlimbs` are valid.
    pub limbs: &'a mut [u64],
    /// `nlimbs`: number of valid limbs.
    pub nlimbs: usize,
    /// `sign`: true for a negative number.
    pub sign: bool,
}

impl<'a> Mpi<'a> {
    /// A number of `nlimbs` valid limbs in `limbs`, with the given sign.
    /// Panics if `nlimbs` exceeds the slice length.
    pub fn new(limbs: &'a mut [u64], nlimbs: usize, sign: bool) -> Self {
        assert!(nlimbs <= limbs.len(), "nlimbs exceeds the limb storage");
        Self {
            limbs,
            nlimbs,
            sign,
        }
    }

    /// `mpi_normalize()`: drop the most significant limbs that are zero.
    pub fn normalize(&mut self) {
        while self.nlimbs > 0 && self.limbs[self.nlimbs - 1] == 0 {
            self.nlimbs -= 1;
        }
    }

    /// `mpi_cmp_ui()`: compare with an unsigned value. `0`, `1` or `-1`.
    pub fn cmp_ui(&mut self, v: u64) -> i32 {
        self.normalize();
        if self.nlimbs == 0 {
            return if v == 0 { 0 } else { -1 };
        }
        if self.sign {
            return -1;
        }
        if self.nlimbs > 1 {
            return 1;
        }
        match self.limbs[0].cmp(&v) {
            core::cmp::Ordering::Equal => 0,
            core::cmp::Ordering::Greater => 1,
            core::cmp::Ordering::Less => -1,
        }
    }
}

/// `mpi_cmp()`: compare two numbers, signs first, then sizes for same-sign
/// values, then limbs. Like C it returns the size difference, not just a
/// sign, when the sizes differ.
pub fn mpi_cmp(u: &mut Mpi<'_>, v: &mut Mpi<'_>) -> i32 {
    u.normalize();
    v.normalize();
    let usize_ = u.nlimbs;
    let vsize = v.nlimbs;

    if !u.sign && v.sign {
        return 1;
    }
    if u.sign && !v.sign {
        return -1;
    }
    if usize_ != vsize && !u.sign && !v.sign {
        return (usize_ as i64 - vsize as i64) as i32;
    }
    if usize_ != vsize && u.sign && v.sign {
        return (vsize as i64 - usize_ as i64) as i32;
    }
    if usize_ == 0 {
        return 0;
    }
    let cmp = mpihelp_cmp(&u.limbs[..usize_], &v.limbs[..usize_]);
    if u.sign { -cmp } else { cmp }
}
