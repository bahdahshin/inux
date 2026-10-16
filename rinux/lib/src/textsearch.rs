// SPDX-License-Identifier: GPL-2.0-or-later
//! The text search front end: pick an algorithm by name and search a buffer.
//!
//! Ported from `lib/textsearch.c`. The C version keeps a runtime registry of
//! algorithms, guarded by a spinlock and walked under RCU, and takes a module
//! reference on each one it hands out. Rust has none of those, and the three
//! algorithms are part of this crate, so the registry becomes a static
//! [`Algo`] table: `"kmp"`, `"bm"` and `"fsm"` are the names the C registry
//! would hold. `textsearch_register()` and `textsearch_unregister()` have no
//! counterpart, and the `TS_AUTOLOAD` flag has nothing to load.
//!
//! The C `textsearch_find()` drives a `get_next_block` callback over
//! fragmented buffers. Here the search runs over one slice, and
//! [`TextSearch::find`] returns the start offset of the first match at or
//! after `from`.

use crate::errno::{EINVAL, ENOENT, Result};
use crate::ts_bm::TsBm;
use crate::ts_fsm::{TsFsm, TsFsmToken};
use crate::ts_kmp::TsKmp;

/// `TS_IGNORECASE`: match letters in either case. The FSM algorithm does
/// not support it, as in C.
pub const TS_IGNORECASE: u32 = 2;

/// The algorithms the registry holds, by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algo {
    /// `"kmp"`: Knuth-Morris-Pratt.
    Kmp,
    /// `"bm"`: Boyer-Moore.
    Bm,
    /// `"fsm"`: the token-chain finite-state search.
    Fsm,
}

impl Algo {
    /// `lookup_ts_algo()`: the algorithm registered under `name`, or
    /// `ENOENT`.
    pub fn by_name(name: &[u8]) -> Result<Algo> {
        match name {
            b"kmp" => Ok(Algo::Kmp),
            b"bm" => Ok(Algo::Bm),
            b"fsm" => Ok(Algo::Fsm),
            _ => Err(ENOENT),
        }
    }
}

/// `struct ts_config`: a prepared search. The byte-pattern algorithms borrow
/// their tables from the caller's scratch buffer.
pub enum TextSearch<'a> {
    Kmp(TsKmp<'a>),
    Bm(TsBm<'a>),
    Fsm(TsFsm<'a>),
}

impl<'a> TextSearch<'a> {
    /// `textsearch_prepare()` for a byte pattern. `scratch` holds the tables
    /// the algorithm needs: at least `pattern.len()` words for KMP, and
    /// `256 + pattern.len()` for Boyer-Moore.
    ///
    /// Returns `ENOENT` for an unknown algorithm, `EINVAL` for an empty
    /// pattern, a short scratch buffer, or an FSM algorithm, which takes
    /// tokens rather than bytes (see [`TextSearch::fsm`]).
    pub fn prepare(
        algo: &[u8],
        pattern: &'a [u8],
        scratch: &'a mut [u32],
        flags: u32,
    ) -> Result<TextSearch<'a>> {
        let ignorecase = flags & TS_IGNORECASE != 0;
        match Algo::by_name(algo)? {
            Algo::Kmp => TsKmp::prepare(pattern, scratch, ignorecase)
                .map(TextSearch::Kmp)
                .ok_or(EINVAL),
            Algo::Bm => TsBm::prepare(pattern, scratch, ignorecase)
                .map(TextSearch::Bm)
                .ok_or(EINVAL),
            Algo::Fsm => Err(EINVAL),
        }
    }

    /// `textsearch_prepare("fsm", ...)`: the token-chain search. Rejects
    /// `TS_IGNORECASE`, as `fsm_init()` does.
    pub fn fsm(tokens: &'a [TsFsmToken], flags: u32) -> Result<TextSearch<'a>> {
        if flags & TS_IGNORECASE != 0 {
            return Err(EINVAL);
        }
        TsFsm::new(tokens).map(TextSearch::Fsm)
    }

    /// `textsearch_find_continuous()`: the offset of the first match at or
    /// after `from` in `text`, or `None`. For the FSM the offset is the start
    /// of the matched span.
    pub fn find(&self, text: &[u8], from: usize) -> Option<usize> {
        match self {
            TextSearch::Kmp(s) => s.find(text, from),
            TextSearch::Bm(s) => s.find(text, from),
            TextSearch::Fsm(s) => s.find(text, from).map(|(start, _)| start),
        }
    }
}
