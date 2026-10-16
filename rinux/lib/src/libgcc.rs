// SPDX-License-Identifier: GPL-2.0-or-later
//! 64-bit shift and compare helpers that GCC calls on 32-bit targets.
//!
//! Ported from `lib/ashldi3.c`, `lib/ashrdi3.c`, `lib/lshrdi3.c`,
//! `lib/cmpdi2.c`, `lib/ucmpdi2.c` and `include/linux/libgcc.h`. Each C
//! function splits a 64-bit value into two 32-bit halves and shifts them
//! with carries. They are kept here as the same algorithm on halves, so the
//! results match the C on every input. rinux targets x86-64, where GCC does
//! not call them, and no kernel code uses them directly.
//!
//! The shift counts follow the C domain `0..64`; outside it the C code is
//! undefined, and these functions mask the count to 6 bits, as x86 does.
//! Comparisons return `0` for less, `1` for equal and `2` for greater, as in
//! libgcc.

/// Split a 64-bit value into `(high, low)` 32-bit halves.
fn split(v: u64) -> (u32, u32) {
    ((v >> 32) as u32, v as u32)
}

/// `__ashldi3()`: `u << b`, with the carry from the low half.
pub fn ashldi3(u: i64, b: i32) -> i64 {
    if b == 0 {
        return u;
    }
    let (high, low) = split(u as u64);
    let b = (b & 63) as u32;
    let bm = 32i32 - b as i32;
    let (w_high, w_low) = if bm <= 0 {
        ((low << (-bm) as u32), 0)
    } else {
        let carries = low >> bm;
        ((high << b) | carries, low << b)
    };
    (((w_high as u64) << 32) | w_low as u64) as i64
}

/// `__ashrdi3()`: arithmetic `u >> b`, sign bits shifted in from the top.
pub fn ashrdi3(u: i64, b: i32) -> i64 {
    if b == 0 {
        return u;
    }
    let (high, low) = split(u as u64);
    let high = high as i32;
    let b = (b & 63) as u32;
    let bm = 32i32 - b as i32;
    let (w_high, w_low) = if bm <= 0 {
        // The high half becomes all sign bits.
        (high >> 31, high >> (-bm) as u32)
    } else {
        let carries = (high as u32) << bm;
        (high >> b, ((low >> b) | carries) as i32)
    };
    ((((w_high as u32) as u64) << 32) | ((w_low as u32) as u64)) as i64
}

/// `__lshrdi3()`: logical `u >> b`, zeros shifted in from the top.
pub fn lshrdi3(u: i64, b: i32) -> i64 {
    if b == 0 {
        return u;
    }
    let (high, low) = split(u as u64);
    let b = (b & 63) as u32;
    let bm = 32i32 - b as i32;
    let (w_high, w_low) = if bm <= 0 {
        (0, high >> (-bm) as u32)
    } else {
        let carries = high << bm;
        (high >> b, (low >> b) | carries)
    };
    (((w_high as u64) << 32) | w_low as u64) as i64
}

/// `__cmpdi2()`: signed comparison of two 64-bit values, `0`, `1` or `2`.
pub fn cmpdi2(a: i64, b: i64) -> i32 {
    let (ah, al) = split(a as u64);
    let (bh, bl) = split(b as u64);
    if (ah as i32) < (bh as i32) {
        return 0;
    }
    if (ah as i32) > (bh as i32) {
        return 2;
    }
    cmp_low(al, bl)
}

/// `__ucmpdi2()`: unsigned comparison of two 64-bit values, `0`, `1` or `2`.
pub fn ucmpdi2(a: u64, b: u64) -> i32 {
    let (ah, al) = split(a);
    let (bh, bl) = split(b);
    if ah < bh {
        return 0;
    }
    if ah > bh {
        return 2;
    }
    cmp_low(al, bl)
}

fn cmp_low(al: u32, bl: u32) -> i32 {
    if al < bl {
        0
    } else if al > bl {
        2
    } else {
        1
    }
}
