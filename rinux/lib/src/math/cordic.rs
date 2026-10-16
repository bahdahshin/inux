// SPDX-License-Identifier: GPL-2.0
//! The CORDIC algorithm: fixed-point sine and cosine.
//!
//! A port of `lib/math/cordic.c` and `include/linux/cordic.h`. Given an angle
//! in whole degrees, [`cordic_calc_iq`] returns the in-phase (cosine) and
//! quadrature (sine) components, each scaled by 2^16, computed with only
//! shifts and adds. The angle is reduced to [-180, 180] and then to the first
//! quadrant before the iteration.

/// An i/q coordinate: `i` is the real (cosine) part, `q` the imaginary (sine)
/// part, both scaled by 2^16.
pub struct CordicIq {
    pub i: i32,
    pub q: i32,
}

/// The gain-normalized starting value (`1/K` scaled by 2^16).
const CORDIC_ANGLE_GEN: i32 = 39797;
const CORDIC_PRECISION_SHIFT: u32 = 16;
const CORDIC_NUM_ITER: usize = (CORDIC_PRECISION_SHIFT + 2) as usize;

#[inline]
const fn cordic_fixed(x: i32) -> i32 {
    x << CORDIC_PRECISION_SHIFT
}

/// `CORDIC_FLOAT()`: round a 2^16 fixed-point value to the nearest integer.
#[inline]
fn cordic_float(x: i32) -> i32 {
    if x >= 0 {
        ((x >> (CORDIC_PRECISION_SHIFT - 1)) + 1) >> 1
    } else {
        -((((-x) >> (CORDIC_PRECISION_SHIFT - 1)) + 1) >> 1)
    }
}

/// `arctan_table`: `atan(2^-i)` for each iteration, scaled by 2^16 degrees.
#[rustfmt::skip]
const ARCTAN_TABLE: [i32; CORDIC_NUM_ITER] = [
    2949120, 1740967, 919879, 466945, 234379, 117304, 58666, 29335, 14668,
    7334, 3667, 1833, 917, 458, 229, 115, 57, 29,
];

/// `cordic_calc_iq()`: the i/q coordinate (cosine, sine ×2^16) for `theta`
/// degrees.
pub fn cordic_calc_iq(theta: i32) -> CordicIq {
    let mut i = CORDIC_ANGLE_GEN;
    let mut q = 0i32;
    let mut angle = 0i32;
    let mut signx = 1i32;

    let mut theta = cordic_fixed(theta);
    let signtheta = if theta < 0 { -1 } else { 1 };
    theta = ((theta + cordic_fixed(180) * signtheta) % cordic_fixed(360))
        - cordic_fixed(180) * signtheta;

    if cordic_float(theta) > 90 {
        theta -= cordic_fixed(180);
        signx = -1;
    } else if cordic_float(theta) < -90 {
        theta += cordic_fixed(180);
        signx = -1;
    }

    for (iter, &atan) in ARCTAN_TABLE.iter().enumerate() {
        let valtmp;
        if theta > angle {
            valtmp = i - (q >> iter);
            q += i >> iter;
            angle += atan;
        } else {
            valtmp = i + (q >> iter);
            q -= i >> iter;
            angle -= atan;
        }
        i = valtmp;
    }

    CordicIq {
        i: i * signx,
        q: q * signx,
    }
}
