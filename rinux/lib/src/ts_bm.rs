// SPDX-License-Identifier: GPL-2.0-or-later
//! Boyer-Moore text search.
//!
//! A port of `lib/ts_bm.c`: precompute a bad-character table (indexed by byte)
//! and a good-suffix table from the pattern, then scan the text comparing the
//! pattern right-to-left, shifting by the larger of the two heuristics on a
//! mismatch.
//!
//! Simplified from Linux's, like [`ts_kmp`](crate::ts_kmp): the `textsearch`
//! framework's fragmented-buffer `get_next_block` abstraction is dropped for a
//! single linear `&[u8]` text, and the caller supplies the table scratch
//! buffer. The `TS_IGNORECASE` flag is applied with `toupper` as characters
//! are compared, and the bad-character table carries both cases.

use crate::ctype::{tolower, toupper};

/// Alphabet size (`ASIZE` in the C): every possible byte.
const ASIZE: usize = 256;

/// A prepared Boyer-Moore search: a pattern and its shift tables.
pub struct TsBm<'a> {
    pattern: &'a [u8],
    /// `bad_shift[0..256]` then `good_shift[0..patlen]`, in one slice.
    tables: &'a [u32],
    ignorecase: bool,
}

#[inline]
fn cf(c: u8, ignorecase: bool) -> u8 {
    if ignorecase { toupper(c) } else { c }
}

/// `subpattern()`: whether a reoccurrence of the suffix exists, used to build
/// the good-suffix table. Indices may go negative, as in the C, which the
/// loop's `y < 0` guard reaches before any out-of-bounds access.
fn subpattern(pattern: &[u8], i: i32, j: i32, g_in: i32, icase: bool) -> bool {
    let mut x = i + g_in - 1;
    let mut y = j + g_in - 1;
    let mut g = g_in;
    loop {
        let px = cf(pattern[x as usize], icase);
        let py = cf(pattern[y as usize], icase);
        x -= 1;
        y -= 1;
        if px != py {
            return false;
        }
        if y < 0 {
            return true;
        }
        g -= 1;
        if g == 0 {
            return cf(pattern[(i - 1) as usize], icase) != cf(pattern[(j - 1) as usize], icase);
        }
    }
}

impl<'a> TsBm<'a> {
    /// `bm_init()` / `compute_prefix_tbl()`: prepare a search for `pattern`,
    /// filling `scratch` (at least `256 + pattern.len()` long) with the
    /// bad-character and good-suffix tables.
    ///
    /// Returns `None` for an empty pattern (which `bm_init` rejects with
    /// `-EINVAL`) or a scratch buffer that is too small.
    pub fn prepare(
        pattern: &'a [u8],
        scratch: &'a mut [u32],
        ignorecase: bool,
    ) -> Option<TsBm<'a>> {
        let patlen = pattern.len();
        if patlen == 0 || scratch.len() < ASIZE + patlen {
            return None;
        }
        {
            let t = &mut scratch[..ASIZE + patlen];
            // Bad-character table: default to patlen, then the distance of
            // each pattern byte (but the last) from the end.
            for b in t.iter_mut().take(ASIZE) {
                *b = patlen as u32;
            }
            for i in 0..patlen - 1 {
                let shift = (patlen - 1 - i) as u32;
                if ignorecase {
                    t[toupper(pattern[i]) as usize] = shift;
                    t[tolower(pattern[i]) as usize] = shift;
                } else {
                    t[pattern[i] as usize] = shift;
                }
            }
            // Good-suffix table (stored after the bad-character table).
            t[ASIZE] = 1;
            for k in 1..patlen {
                t[ASIZE + k] = patlen as u32;
            }
            let mut g: i32 = 1;
            let mut i: i32 = patlen as i32 - 1;
            while i > 0 {
                let mut j = i - 1;
                while j >= 1 - g {
                    if subpattern(pattern, i, j, g, ignorecase) {
                        t[ASIZE + g as usize] = (patlen as i32 - j - g) as u32;
                        break;
                    }
                    j -= 1;
                }
                g += 1;
                i -= 1;
            }
        }
        Some(TsBm {
            pattern,
            tables: &scratch[..ASIZE + patlen],
            ignorecase,
        })
    }

    #[inline]
    fn bad_shift(&self, c: u8) -> u32 {
        self.tables[c as usize]
    }
    #[inline]
    fn good_shift(&self, i: usize) -> u32 {
        self.tables[ASIZE + i]
    }

    /// The pattern length.
    pub fn pattern_len(&self) -> usize {
        self.pattern.len()
    }

    /// `matchpat()`: how many trailing bytes of the pattern match the text
    /// ending at `shift` (compared right-to-left).
    fn matchpat(&self, text: &[u8], shift: usize) -> usize {
        let patlen = self.pattern.len();
        let mut i = 0;
        while i < patlen {
            let t = cf(text[shift - i], self.ignorecase);
            if t != cf(self.pattern[patlen - 1 - i], self.ignorecase) {
                break;
            }
            i += 1;
        }
        i
    }

    /// `bm_find()`: the start index of the first match in `text` at or after
    /// `from`, or `None`.
    pub fn find(&self, text: &[u8], from: usize) -> Option<usize> {
        let patlen = self.pattern.len();
        let text_len = text.len();
        // `shift` is the index in the text of the pattern's last byte.
        let mut shift = from + patlen - 1;
        while shift < text_len {
            let i = self.matchpat(text, shift);
            if i == patlen {
                return Some(shift - (patlen - 1));
            }
            let bs = self.bad_shift(text[shift - i]) as i64;
            let by_bad = shift as i64 - i as i64 + bs;
            let by_good = shift as i64 + self.good_shift(i) as i64;
            shift = by_bad.max(by_good) as usize;
        }
        None
    }
}
