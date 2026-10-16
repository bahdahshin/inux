// SPDX-License-Identifier: GPL-2.0
//! Kernel command line parameters.
//!
//! A partial port of `kernel/params.c`: `parameq()`, `parameqn()` and the
//! `parse_args()` loop, for the early parameters that `parse_early_param()`
//! handles. Module parameters come with module support. The tokenizer,
//! `next_arg()`, comes from rinux-lib's port of `lib/cmdline.c`.

use rinux_lib::cmdline::next_arg;
use rinux_lib::string::{skip_spaces, strlen};

/// Byte `i` of a C string, or NUL past the end.
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

fn dash2underscore(c: u8) -> u8 {
    if c == b'-' { b'_' } else { c }
}

/// `parameqn()`: compares `n` bytes, treating `-` and `_` as equal.
pub fn parameqn(a: &[u8], b: &[u8], n: usize) -> bool {
    (0..n).all(|i| dash2underscore(at(a, i)) == dash2underscore(at(b, i)))
}

/// `parameq()`: `a` and `b` are the same name, with `-` equal to `_`.
pub fn parameq(a: &[u8], b: &[u8]) -> bool {
    parameqn(a, b, strlen(a) + 1)
}

/// `parse_args()` without module parameters: calls `unknown(param, val)`
/// for every parameter, stopping at `--`. Returns what follows `--`.
pub fn parse_args(
    args: &[u8],
    mut unknown: impl FnMut(&[u8], Option<&[u8]>) -> i32,
) -> Option<&[u8]> {
    let args = &args[..strlen(args)];
    // Chew leading spaces.
    let mut args = &args[skip_spaces(args)..];
    while !args.is_empty() {
        let (param, val, rest) = next_arg(args);
        args = rest;
        // Stop at --
        if val.is_none() && param == b"--" {
            return Some(args);
        }
        unknown(param, val);
    }
    None
}
