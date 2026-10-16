// SPDX-License-Identifier: GPL-2.0
//! Split a string at whitespace into argument words.
//!
//! Ported from `lib/argv_split.c`. The C function copies the string, NUL-
//! terminates each word and returns a `kmalloc`ed `argv` array. Here the
//! words are index ranges into the caller's bytes, so nothing is allocated
//! and the caller decides where the storage comes from. The string ends at
//! its first NUL, as in C.

use crate::ctype::isspace;
use core::ops::Range;

/// The C string in `s`: the bytes before the first NUL, or all of `s`.
fn c_str(s: &[u8]) -> &[u8] {
    match s.iter().position(|&c| c == 0) {
        Some(end) => &s[..end],
        None => s,
    }
}

/// `count_argc()`: how many whitespace-separated words `s` holds.
pub fn count_argc(s: &[u8]) -> usize {
    argv_split(s).count()
}

/// `argv_split()` without the allocation: an iterator over the index range
/// of each word in `s`. Runs of whitespace separate words, and leading or
/// trailing whitespace yields nothing. No quote processing is done.
pub fn argv_split(s: &[u8]) -> ArgvSplit<'_> {
    ArgvSplit {
        s: c_str(s),
        pos: 0,
    }
}

/// The words of a string, as produced by [`argv_split`].
#[derive(Clone, Debug)]
pub struct ArgvSplit<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Iterator for ArgvSplit<'_> {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Range<usize>> {
        // Skip the separator run.
        while self.pos < self.s.len() && isspace(self.s[self.pos]) {
            self.pos += 1;
        }
        if self.pos == self.s.len() {
            return None;
        }
        let start = self.pos;
        while self.pos < self.s.len() && !isspace(self.s[self.pos]) {
            self.pos += 1;
        }
        Some(start..self.pos)
    }
}
