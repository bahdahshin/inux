// SPDX-License-Identifier: GPL-2.0-only
//! Constant-time inequality test for byte strings.
//!
//! Ported from `lib/crypto/memneq.c`, `__crypto_memneq_generic()`. The C code
//! ORs together the XOR of every byte pair, so the time taken does not depend
//! on where the first difference is. The optimiser is kept from undoing that
//! by `OPTIMIZER_HIDE_VAR()`; `core::hint::black_box` plays that role here.
//! The architecture-specific versions compute the same answer.

/// `crypto_memneq()`: true when `a` and `b` differ in any byte, checking the
/// whole length regardless of where the difference is.
///
/// Panics if the slices differ in length.
pub fn crypto_memneq(a: &[u8], b: &[u8]) -> bool {
    assert_eq!(a.len(), b.len(), "crypto_memneq lengths differ");
    let mut neq: u8 = 0;
    for (x, y) in a.iter().zip(b) {
        neq |= x ^ y;
        neq = core::hint::black_box(neq);
    }
    neq != 0
}
