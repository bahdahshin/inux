// SPDX-License-Identifier: GPL-2.0-or-later
//! A naive finite-state-machine text search.
//!
//! Ported from `lib/ts_fsm.c`. A pattern is a chain of tokens. Each token
//! accepts one character from a set (a literal, or a ctype class such as
//! digit or alpha) and says how many times it may repeat: exactly once,
//! zero or one time, one or more times, or zero or more times. A
//! `HeadIgnore` token at the front lets the match start anywhere, skipping
//! characters it accepts.
//!
//! Simplified from Linux's: the `textsearch` framework and its
//! `get_next_block` callback (which walks fragmented buffers) are dropped.
//! The search is over one linear `&[u8]`, and the caller gets back the
//! match span rather than a `ts_state`. `TS_IGNORECASE` is rejected, as in C.

use crate::ctype::{_C, _D, _L, _P, _S, _SP, _U, _X};
use crate::errno::{EINVAL, Result};

// Bits in the class table: the `ctype.h` flags, plus two magic bits for the
// ASCII and wildcard classes (`_A` and `_W` in C).
const A: u16 = 0x100;
const W: u16 = 0x200;
const C: u16 = _C as u16;
const SP: u16 = _SP as u16;
const P: u16 = _P as u16;
const D: u16 = _D as u16;
const U: u16 = _U as u16;
const L: u16 = _L as u16;
const X: u16 = _X as u16;
const S: u16 = _S as u16;

/// `token_lookup_tbl`: the class bits of each byte value.
#[rustfmt::skip]
static TOKEN_LOOKUP: [u16; 256] = [
    (W | A | C), (W | A | C), (W | A | C), (W | A | C),
    (W | A | C), (W | A | C), (W | A | C), (W | A | C),
    (W | A | C), (W | A | C | S), (W | A | C | S), (W | A | C | S),
    (W | A | C | S), (W | A | C | S), (W | A | C), (W | A | C),
    (W | A | C), (W | A | C), (W | A | C), (W | A | C),
    (W | A | C), (W | A | C), (W | A | C), (W | A | C),
    (W | A | C), (W | A | C), (W | A | C), (W | A | C),
    (W | A | C), (W | A | C), (W | A | C), (W | A | C),
    (W | A | S | SP), (W | A | P), (W | A | P), (W | A | P),
    (W | A | P), (W | A | P), (W | A | P), (W | A | P),
    (W | A | P), (W | A | P), (W | A | P), (W | A | P),
    (W | A | P), (W | A | P), (W | A | P), (W | A | P),
    (W | A | D), (W | A | D), (W | A | D), (W | A | D),
    (W | A | D), (W | A | D), (W | A | D), (W | A | D),
    (W | A | D), (W | A | D), (W | A | P), (W | A | P),
    (W | A | P), (W | A | P), (W | A | P), (W | A | P),
    (W | A | P), (W | A | U | X), (W | A | U | X), (W | A | U | X),
    (W | A | U | X), (W | A | U | X), (W | A | U | X), (W | A | U),
    (W | A | U), (W | A | U), (W | A | U), (W | A | U),
    (W | A | U), (W | A | U), (W | A | U), (W | A | U),
    (W | A | U), (W | A | U), (W | A | U), (W | A | U),
    (W | A | U), (W | A | U), (W | A | U), (W | A | U),
    (W | A | U), (W | A | U), (W | A | U), (W | A | P),
    (W | A | P), (W | A | P), (W | A | P), (W | A | P),
    (W | A | P), (W | A | L | X), (W | A | L | X), (W | A | L | X),
    (W | A | L | X), (W | A | L | X), (W | A | L | X), (W | A | L),
    (W | A | L), (W | A | L), (W | A | L), (W | A | L),
    (W | A | L), (W | A | L), (W | A | L), (W | A | L),
    (W | A | L), (W | A | L), (W | A | L), (W | A | L),
    (W | A | L), (W | A | L), (W | A | L), (W | A | L),
    (W | A | L), (W | A | L), (W | A | L), (W | A | P),
    (W | A | P), (W | A | P), (W | A | P), (W | A | C),
    W, W, W, W,
    W, W, W, W,
    W, W, W, W,
    W, W, W, W,
    W, W, W, W,
    W, W, W, W,
    W, W, W, W,
    W, W, W, W,
    (W | S | SP), (W | P), (W | P), (W | P),
    (W | P), (W | P), (W | P), (W | P),
    (W | P), (W | P), (W | P), (W | P),
    (W | P), (W | P), (W | P), (W | P),
    (W | P), (W | P), (W | P), (W | P),
    (W | P), (W | P), (W | P), (W | P),
    (W | P), (W | P), (W | P), (W | P),
    (W | P), (W | P), (W | P), (W | P),
    (W | U), (W | U), (W | U), (W | U),
    (W | U), (W | U), (W | U), (W | U),
    (W | U), (W | U), (W | U), (W | U),
    (W | U), (W | U), (W | U), (W | U),
    (W | U), (W | U), (W | U), (W | U),
    (W | U), (W | U), (W | U), (W | P),
    (W | U), (W | U), (W | U), (W | U),
    (W | U), (W | U), (W | U), (W | L),
    (W | L), (W | L), (W | L), (W | L),
    (W | L), (W | L), (W | L), (W | L),
    (W | L), (W | L), (W | L), (W | L),
    (W | L), (W | L), (W | L), (W | L),
    (W | L), (W | L), (W | L), (W | L),
    (W | L), (W | L), (W | L), (W | P),
    (W | L), (W | L), (W | L), (W | L),
    (W | L), (W | L), (W | L), (W | L),
];

/// `enum ts_fsm_type`: the set of characters a token accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TsFsmType {
    /// The single byte `value` of the token.
    Specific,
    /// Any byte.
    Wildcard,
    Digit,
    Xdigit,
    Print,
    Alpha,
    Alnum,
    Ascii,
    Cntrl,
    Graph,
    Lower,
    Upper,
    Punct,
    Space,
}

/// `enum ts_fsm_recur`: how many times a token may match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TsFsmRecur {
    /// Exactly once.
    Single,
    /// Zero or one time.
    Perhaps,
    /// Zero or more times.
    Any,
    /// One or more times.
    Multi,
    /// Zero or more ignored characters at the head; the match then starts
    /// anywhere. Only valid as the first token of a pattern with more tokens
    /// after it.
    HeadIgnore,
}

/// `struct ts_fsm_token`: one step of the pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TsFsmToken {
    pub ttype: TsFsmType,
    pub recur: TsFsmRecur,
    /// The byte for [`TsFsmType::Specific`]; ignored otherwise.
    pub value: u8,
}

/// `token_map[]`: the class bits a token type tests, or 0 for a literal.
const fn token_mask(ttype: TsFsmType) -> u16 {
    match ttype {
        TsFsmType::Specific => 0,
        TsFsmType::Wildcard => W,
        TsFsmType::Cntrl => C,
        TsFsmType::Lower => L,
        TsFsmType::Upper => U,
        TsFsmType::Punct => P,
        TsFsmType::Space => S,
        TsFsmType::Digit => D,
        TsFsmType::Xdigit => D | X,
        TsFsmType::Alpha => U | L,
        TsFsmType::Alnum => U | L | D,
        TsFsmType::Print => P | U | L | D | SP,
        TsFsmType::Graph => P | U | L | D,
        TsFsmType::Ascii => A,
    }
}

/// `match_token()`: does `t` accept byte `d`?
fn match_token(t: &TsFsmToken, d: u8) -> bool {
    let mask = token_mask(t.ttype);
    if mask != 0 {
        TOKEN_LOOKUP[d as usize] & mask != 0
    } else {
        t.value == d
    }
}

/// A checked pattern, `fsm_init()`.
#[derive(Clone, Debug)]
pub struct TsFsm<'a> {
    tokens: &'a [TsFsmToken],
}

/// Result of one pass over the tokens from a fixed start.
enum Pass {
    Found { start: usize, end: usize },
    Mismatch,
    NoMatch,
}

impl<'a> TsFsm<'a> {
    /// `fsm_init()`: check the pattern. Returns `EINVAL` for an empty
    /// pattern, a `HeadIgnore` anywhere but the front of a longer pattern.
    pub fn new(tokens: &'a [TsFsmToken]) -> Result<TsFsm<'a>> {
        if tokens.is_empty() {
            return Err(EINVAL);
        }
        for (i, t) in tokens.iter().enumerate() {
            if t.recur == TsFsmRecur::HeadIgnore && (i != 0 || i == tokens.len() - 1) {
                return Err(EINVAL);
            }
        }
        Ok(TsFsm { tokens })
    }

    /// `fsm_get_pattern()`: the tokens this search was built from.
    pub fn pattern(&self) -> &'a [TsFsmToken] {
        self.tokens
    }

    /// `fsm_find()`: the first match at or after `from`, as `(start, end)`
    /// byte offsets with `end` exclusive, or `None`.
    ///
    /// A pattern that does not start with `HeadIgnore` is strict: it must
    /// match at `from` itself, and a mismatch ends the search. Otherwise a
    /// mismatch moves the start forward one byte and tries again.
    pub fn find(&self, text: &[u8], from: usize) -> Option<(usize, usize)> {
        if from >= text.len() {
            return None;
        }
        let strict = self.tokens[0].recur != TsFsmRecur::HeadIgnore;
        let mut start = from;
        loop {
            match self.pass(text, start) {
                Pass::Found { start, end } => return Some((start, end)),
                Pass::NoMatch => return None,
                Pass::Mismatch if strict => return None,
                Pass::Mismatch => start += 1,
            }
        }
    }

    /// One attempt from `start`: the `startover:` body of `fsm_find()`.
    /// The match start is `start` unless a `HeadIgnore` head moved it past
    /// the ignored bytes.
    fn pass(&self, text: &[u8], start: usize) -> Pass {
        let len = text.len();
        let mut pos = start;
        let mut match_start = start;
        let toks = self.tokens;
        let at = |p: usize| text[p];

        for ti in 0..toks.len() {
            let cur = &toks[ti];
            let next = toks.get(ti + 1);

            match cur.recur {
                TsFsmRecur::Single => {
                    if pos >= len {
                        return Pass::NoMatch;
                    }
                    if !match_token(cur, at(pos)) {
                        return Pass::Mismatch;
                    }
                }
                TsFsmRecur::Perhaps => {
                    if pos >= len || !match_token(cur, at(pos)) {
                        continue;
                    }
                }
                TsFsmRecur::Multi => {
                    if pos >= len {
                        return Pass::NoMatch;
                    }
                    if !match_token(cur, at(pos)) {
                        return Pass::Mismatch;
                    }
                    pos += 1;
                    // Falls through to the `Any` handling in C.
                    match next {
                        None => {
                            return Pass::Found {
                                start: match_start,
                                end: pos,
                            };
                        }
                        Some(nx) => {
                            if pos >= len {
                                continue;
                            }
                            while !match_token(nx, at(pos)) {
                                if !match_token(cur, at(pos)) {
                                    return Pass::Mismatch;
                                }
                                pos += 1;
                                if pos >= len {
                                    return Pass::NoMatch;
                                }
                            }
                            continue;
                        }
                    }
                }
                TsFsmRecur::Any => {
                    let Some(nx) = next else {
                        return Pass::Found {
                            start: match_start,
                            end: pos,
                        };
                    };
                    if pos >= len {
                        continue;
                    }
                    while !match_token(nx, at(pos)) {
                        if !match_token(cur, at(pos)) {
                            return Pass::Mismatch;
                        }
                        pos += 1;
                        if pos >= len {
                            return Pass::NoMatch;
                        }
                    }
                    continue;
                }
                TsFsmRecur::HeadIgnore => {
                    if pos >= len {
                        continue;
                    }
                    let Some(nx) = next else { unreachable!() };
                    while !match_token(nx, at(pos)) {
                        // The head ignores bytes it accepts; anything else
                        // ends the search, with no restart.
                        if !match_token(cur, at(pos)) {
                            return Pass::NoMatch;
                        }
                        pos += 1;
                        if pos >= len {
                            return Pass::NoMatch;
                        }
                    }
                    // The match really starts here, after the ignored run.
                    match_start = pos;
                    continue;
                }
            }
            pos += 1;
        }

        if pos >= len {
            Pass::Found {
                start: match_start,
                end: pos,
            }
        } else {
            Pass::NoMatch
        }
    }
}
