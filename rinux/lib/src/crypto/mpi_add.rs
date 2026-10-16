// SPDX-License-Identifier: GPL-2.0-or-later
//! `w = u + v` and `w = u - v` for signed multi-precision numbers.
//!
//! Ported from `lib/crypto/mpi/mpi-add.c`. The C code grows `w` with
//! `RESIZE_IF_NEEDED()`, which allocates, and `mpi_sub()` copies `v`. Here `w`
//! has fixed storage: when it is too small the call returns `ENOMEM` and `w`
//! is left alone, and `mpi_sub()` flips the sign of `v` on the fly instead of
//! copying it. The general-size limb add and subtract that C takes from
//! `mpih-add`/`mpih-sub` are inline, as those files are not ported yet.
//!
//! C may alias `w` with `u` or `v`; the Rust signatures borrow the operands
//! separately, so an aliased call copies first.

use crate::crypto::mpi::{mpihelp_cmp, mpihelp_sub_n};
use crate::crypto::mpi_obj::Mpi;
use crate::errno::{ENOMEM, Errno};

/// `mpi_add()`: `w = u + v`.
pub fn mpi_add(w: &mut Mpi<'_>, u: &Mpi<'_>, v: &Mpi<'_>) -> Result<(), Errno> {
    add_signed(w, u, v, false)
}

/// `mpi_sub()`: `w = u - v`, i.e. `mpi_add` with the sign of `v` flipped.
pub fn mpi_sub(w: &mut Mpi<'_>, u: &Mpi<'_>, v: &Mpi<'_>) -> Result<(), Errno> {
    add_signed(w, u, v, true)
}

/// The shared body: `v` is negated when `negate_v` is set.
fn add_signed(w: &mut Mpi<'_>, u: &Mpi<'_>, v: &Mpi<'_>, negate_v: bool) -> Result<(), Errno> {
    let v_sign = v.sign != negate_v;

    // Let U be the operand with more limbs, as C swaps them.
    let (up, usize_, usign, vp, vsize, vsign) = if u.nlimbs < v.nlimbs {
        (
            &v.limbs[..v.nlimbs],
            v.nlimbs,
            v_sign,
            &u.limbs[..u.nlimbs],
            u.nlimbs,
            u.sign,
        )
    } else {
        (
            &u.limbs[..u.nlimbs],
            u.nlimbs,
            u.sign,
            &v.limbs[..v.nlimbs],
            v.nlimbs,
            v_sign,
        )
    };

    // The result can need one limb more than U.
    if w.limbs.len() < usize_ + 1 {
        return Err(ENOMEM);
    }

    let mut wsize;
    let mut wsign = false;
    if vsize == 0 {
        // Simple: a copy of U.
        w.limbs[..usize_].copy_from_slice(up);
        wsize = usize_;
        wsign = usign;
    } else if usign != vsign {
        // Different signs: subtract the smaller magnitude from the larger.
        if usize_ != vsize {
            let borrow = sub_general(&mut w.limbs[..usize_], up, vp);
            debug_assert_eq!(borrow, 0, "U has at least as many limbs as V");
            wsize = usize_;
            wsize = normalize_len(&w.limbs[..usize_], wsize);
            wsign = usign;
        } else if mpihelp_cmp(up, vp) < 0 {
            mpihelp_sub_n(&mut w.limbs[..usize_], vp, up);
            wsize = normalize_len(&w.limbs[..usize_], usize_);
            if !usign {
                wsign = true;
            }
        } else {
            mpihelp_sub_n(&mut w.limbs[..usize_], up, vp);
            wsize = normalize_len(&w.limbs[..usize_], usize_);
            if usign {
                wsign = true;
            }
        }
    } else {
        // Same sign: add the magnitudes.
        let cy = add_general(&mut w.limbs[..usize_], up, vp);
        w.limbs[usize_] = cy;
        wsize = usize_ + cy as usize;
        if usign {
            wsign = true;
        }
    }

    w.nlimbs = wsize;
    w.sign = wsign;
    Ok(())
}

/// `mpihelp_add()` for `usize >= vsize`: `res = u + v`, returning the carry.
fn add_general(res: &mut [u64], u: &[u64], v: &[u64]) -> u64 {
    let mut cy = 0u64;
    for i in 0..u.len() {
        let b = if i < v.len() { v[i] } else { 0 };
        let (s1, o1) = u[i].overflowing_add(b);
        let (s2, o2) = s1.overflowing_add(cy);
        res[i] = s2;
        cy = u64::from(o1) + u64::from(o2);
    }
    cy
}

/// `mpihelp_sub()` for `usize >= vsize`: `res = u - v`, returning the borrow.
fn sub_general(res: &mut [u64], u: &[u64], v: &[u64]) -> u64 {
    let mut cy = 0u64;
    for i in 0..u.len() {
        let b = if i < v.len() { v[i] } else { 0 };
        let (d1, o1) = u[i].overflowing_sub(b);
        let (d2, o2) = d1.overflowing_sub(cy);
        res[i] = d2;
        cy = u64::from(o1) + u64::from(o2);
    }
    cy
}

/// `MPN_NORMALIZE()`: the length without leading zero limbs.
fn normalize_len(limbs: &[u64], mut len: usize) -> usize {
    while len > 0 && limbs[len - 1] == 0 {
        len -= 1;
    }
    len
}
