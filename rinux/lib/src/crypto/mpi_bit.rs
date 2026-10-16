// SPDX-License-Identifier: GPL-2.0-only
//! Bit-level operations on the `Mpi` object: bit count, bit test, bit set and
//! right shift.
//!
//! Ported from `lib/crypto/mpi/mpi-bit.c`. `mpi_set_bit()` may grow the number
//! with `mpi_resize()`, which allocates; here the storage is fixed, so setting
//! a bit past it returns `ENOMEM` and leaves the number alone. `mpi_rshift()`
//! has an in-place mode in C (`x == a`); the Rust signature takes two separate
//! values, so the caller copies first when it wants that.

use crate::crypto::mpi_obj::Mpi;
use crate::errno::{ENOMEM, Errno};

/// Bits per limb (`BITS_PER_MPI_LIMB`).
const BITS_PER_MPI_LIMB: usize = 64;

impl Mpi<'_> {
    /// `mpi_get_nbits()`: the number of significant bits, after normalising.
    pub fn get_nbits(&mut self) -> usize {
        self.normalize();
        if self.nlimbs == 0 {
            return 0;
        }
        // The top limb is never zero after normalising, so its leading zeros
        // are well defined.
        let top = self.limbs[self.nlimbs - 1];
        BITS_PER_MPI_LIMB - top.leading_zeros() as usize + (self.nlimbs - 1) * BITS_PER_MPI_LIMB
    }

    /// `mpi_test_bit()`: bit `n`, zero beyond the valid limbs.
    pub fn test_bit(&self, n: usize) -> bool {
        let limbno = n / BITS_PER_MPI_LIMB;
        let bitno = n % BITS_PER_MPI_LIMB;
        if limbno >= self.nlimbs {
            return false;
        }
        self.limbs[limbno] & (1 << bitno) != 0
    }

    /// `mpi_set_bit()`: set bit `n`, growing the number if the storage allows.
    /// Returns `ENOMEM` when the bit lies beyond the storage.
    pub fn set_bit(&mut self, n: usize) -> Result<(), Errno> {
        let limbno = n / BITS_PER_MPI_LIMB;
        let bitno = n % BITS_PER_MPI_LIMB;
        if limbno >= self.limbs.len() {
            return Err(ENOMEM);
        }
        if limbno >= self.nlimbs {
            // Zero the limbs being brought into use, as C does.
            for limb in &mut self.limbs[self.nlimbs..=limbno] {
                *limb = 0;
            }
            self.nlimbs = limbno + 1;
        }
        self.limbs[limbno] |= 1 << bitno;
        Ok(())
    }
}

/// `mpi_rshift()`: `x = a >> n`, with the sign copied from `a`. `x` must have
/// room for `a.nlimbs` limbs; otherwise `ENOMEM` and `x` is left alone.
pub fn mpi_rshift(x: &mut Mpi<'_>, a: &Mpi<'_>, n: usize) -> Result<(), Errno> {
    let nlimbs = n / BITS_PER_MPI_LIMB;
    let nbits = (n % BITS_PER_MPI_LIMB) as u32;
    let xsize = a.nlimbs;
    if x.limbs.len() < xsize {
        return Err(ENOMEM);
    }
    x.sign = a.sign;

    if nlimbs >= xsize {
        x.nlimbs = 0;
        return Ok(());
    }

    // Drop the low `nlimbs` limbs, then shift the rest by the remaining bits.
    let kept = xsize - nlimbs;
    x.limbs[..kept].copy_from_slice(&a.limbs[nlimbs..xsize]);
    x.nlimbs = kept;
    if nbits != 0 {
        shift_right_in_place(&mut x.limbs[..kept], nbits);
    }
    x.normalize();
    Ok(())
}

/// Shift a limb slice right by `nbits` (1 to 63) in place: each limb takes
/// its own bits and the low bits of the limb above. Walking upward means the
/// limb above is still unshifted when it is read, so no copy is needed.
fn shift_right_in_place(limbs: &mut [u64], nbits: u32) {
    let sh_2 = 64 - nbits;
    let len = limbs.len();
    for i in 0..len - 1 {
        limbs[i] = (limbs[i] >> nbits) | (limbs[i + 1] << sh_2);
    }
    limbs[len - 1] >>= nbits;
}
