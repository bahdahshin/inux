// SPDX-License-Identifier: GPL-2.0-or-later
//! Knuth-Morris-Pratt text search.
//!
//! A port of `lib/ts_kmp.c`: precompute the KMP prefix table (failure
//! function) from the pattern in O(m), then scan the text in O(n), using the
//! table to avoid re-examining characters on a mismatch.
//!
//! Simplified from Linux's: the `textsearch` framework's `get_next_block`
//! abstraction (which walks fragmented, non-linear buffers such as `skb`
//! data) is dropped in favour of a single linear `&[u8]` text, and the caller
//! supplies the prefix-table scratch buffer rather than a `ts_config` priv
//! allocation. The `TS_IGNORECASE` flag is supported via `toupper` applied as
//! characters are compared.

use crate::ctype::toupper;

/// A prepared Knuth-Morris-Pratt search: a pattern and its prefix table.
pub struct TsKmp<'a> {
    pattern: &'a [u8],
    prefix_tbl: &'a [u32],
    ignorecase: bool,
}

/// Folds case for the comparison when `ignorecase` is set.
#[inline]
fn cf(c: u8, ignorecase: bool) -> u8 {
    if ignorecase { toupper(c) } else { c }
}

impl<'a> TsKmp<'a> {
    /// `kmp_init()`: prepare a search for `pattern`, computing the prefix
    /// table into `scratch` (which must be at least `pattern.len()` long).
    ///
    /// Returns `None` for an empty pattern (which `kmp_init` rejects with
    /// `-EINVAL`, since the search would read past the pattern) or a scratch
    /// buffer that is too small.
    pub fn prepare(
        pattern: &'a [u8],
        scratch: &'a mut [u32],
        ignorecase: bool,
    ) -> Option<TsKmp<'a>> {
        let len = pattern.len();
        if len == 0 || scratch.len() < len {
            return None;
        }
        // `compute_prefix_tbl()` over the pattern. The C code leaves
        // `prefix_tbl[0]` to the zeroed allocation, so set it here.
        {
            let tbl = &mut scratch[..len];
            tbl[0] = 0;
            let mut k: u32 = 0;
            for q in 1..len {
                while k > 0 && cf(pattern[k as usize], ignorecase) != cf(pattern[q], ignorecase) {
                    k = tbl[k as usize - 1];
                }
                if cf(pattern[k as usize], ignorecase) == cf(pattern[q], ignorecase) {
                    k += 1;
                }
                tbl[q] = k;
            }
        }
        Some(TsKmp {
            pattern,
            prefix_tbl: &scratch[..len],
            ignorecase,
        })
    }

    /// The pattern length.
    pub fn pattern_len(&self) -> usize {
        self.pattern.len()
    }

    /// `kmp_find()`: the start index of the first match in `text` at or after
    /// `from`, or `None` if there is none.
    ///
    /// To iterate over all matches, call again with `from` set past the
    /// previous match (its start + 1 for overlapping matches, or + the
    /// pattern length for non-overlapping ones, as the kernel's state does).
    pub fn find(&self, text: &[u8], from: usize) -> Option<usize> {
        if from >= text.len() {
            return None;
        }
        let plen = self.pattern.len() as u32;
        let mut q: u32 = 0;
        for (i, &byte) in text.iter().enumerate().skip(from) {
            let t = cf(byte, self.ignorecase);
            while q > 0 && cf(self.pattern[q as usize], self.ignorecase) != t {
                q = self.prefix_tbl[q as usize - 1];
            }
            if cf(self.pattern[q as usize], self.ignorecase) == t {
                q += 1;
            }
            if q == plen {
                return Some(i + 1 - plen as usize);
            }
        }
        None
    }
}
