// SPDX-License-Identifier: GPL-2.0-or-later
//! `w = u - v` for an `MPI` and an unsigned limb.
//!
//! Ported from `lib/crypto/mpi/mpi-sub-ui.c`. The C version may grow `w` with
//! `mpi_resize()`, which allocates, and returns `-ENOMEM` on failure. Here `w`
//! has fixed storage, so when it is too small the call returns `ENOMEM` and
//! leaves `w` as it was. C's `w` may alias `u`; the Rust signature takes two
//! separate values, so the aliased case is the caller's to copy first.
//!
//! The addition and subtraction on limbs are the `mpihelp_add_1` and
//! `mpihelp_sub_1` loops, written inline here because those C files are not
//! ported yet.

use crate::crypto::mpi_obj::Mpi;
use crate::errno::{ENOMEM, Errno};

/// `mpi_sub_ui()`: `w = u - vval`, sign and size as in C. `w` must have room
/// for `u.nlimbs + 1` limbs (or one limb when `u` is zero).
pub fn mpi_sub_ui(w: &mut Mpi<'_>, u: &Mpi<'_>, vval: u64) -> Result<(), Errno> {
    let un = u.nlimbs;
    if un == 0 {
        if w.limbs.is_empty() {
            return Err(ENOMEM);
        }
        w.limbs[0] = vval;
        w.nlimbs = usize::from(vval != 0);
        w.sign = vval != 0;
        return Ok(());
    }

    // Room for W and a possible carry limb.
    if w.limbs.len() < un + 1 {
        return Err(ENOMEM);
    }

    if u.sign {
        // -(|u| + vval): add the magnitudes, keeping the sign.
        let mut cy = vval;
        for i in 0..un {
            let (sum, ov) = u.limbs[i].overflowing_add(cy);
            w.limbs[i] = sum;
            cy = u64::from(ov);
        }
        w.limbs[un] = cy;
        w.nlimbs = un + cy as usize;
        w.sign = true;
    } else if un == 1 && u.limbs[0] < vval {
        // The result is negative and one limb: vval - u.
        w.limbs[0] = vval - u.limbs[0];
        w.nlimbs = 1;
        w.sign = true;
    } else {
        // u - vval, with u at least vval in magnitude, so no borrow leaves.
        let mut borrow = vval;
        for i in 0..un {
            let (diff, ov) = u.limbs[i].overflowing_sub(borrow);
            w.limbs[i] = diff;
            borrow = u64::from(ov);
        }
        // The size can drop by at most one limb.
        w.nlimbs = un - usize::from(w.limbs[un - 1] == 0);
        w.sign = false;
    }
    w.normalize();
    Ok(())
}
