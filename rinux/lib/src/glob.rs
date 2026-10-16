// SPDX-License-Identifier: GPL-2.0 OR MIT
//! Shell-style pattern matching.
//!
//! Ported from `lib/glob.c`.

#[inline]
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// `glob_match()`: shell-style match of `pat` against the whole of `s`,
/// like `!fnmatch(pat, s, 0)`.
///
/// Metacharacters are `?`, `*`, `[` and `\`, and inside classes `!`, `-`
/// and `]`. Classes are complemented by a leading `!` as in glob(7); the
/// regex-style `[^a-z]` is not supported. An opening bracket without a
/// matching close is matched literally. `/` and leading `.` are not
/// special.
///
/// It is meant for device denylists: patterns are not preprocessed, it
/// does not recurse, and run time is at most `len(s) * len(pat)`. The worst
/// case is something like `glob_match("*aaaaa", "aaaaaaaaaa")`, which
/// makes six passes over the pattern.
///
/// Both arguments follow the C-string convention of [`crate::string`].
pub fn glob_match(pat: &[u8], s: &[u8]) -> bool {
    glob_match_str(pat, s, None)
}

/// `glob_match_len()`: like [`glob_match`], reading at most `len` bytes of
/// `s`. A NUL before `len` still ends the string.
pub fn glob_match_len(pat: &[u8], s: &[u8], len: usize) -> bool {
    glob_match_str(pat, s, Some(len))
}

enum Step {
    Next,
    Literal(u8),
    Backtrack,
}

fn glob_match_str(pat: &[u8], s: &[u8], str_end: Option<usize>) -> bool {
    // On mismatch, backtrack to the previous * and retry one character
    // later in the string. Because * matches any character (/ included),
    // there is never a need to backtrack more than one level.
    let mut back_pat: Option<usize> = None;
    let mut back_str = 0;
    let mut p = 0;
    let mut si = 0;

    // Match each token (character or class) of the pattern against the
    // remaining tail of the string. Return false on mismatch, or true
    // after matching the trailing NUL bytes.
    loop {
        let c = if str_end.is_some_and(|end| si >= end) {
            0
        } else {
            at(s, si)
        };
        let d = at(pat, p);
        p += 1;
        si += 1;

        let step = match d {
            // Wildcard: anything but NUL.
            b'?' => {
                if c == 0 {
                    return false;
                }
                Step::Next
            }
            // Any-length wildcard.
            b'*' => {
                if at(pat, p) == 0 {
                    // Trailing * matches the rest.
                    return true;
                }
                back_pat = Some(p);
                // Allow a zero-length match.
                si -= 1;
                back_str = si;
                Step::Next
            }
            // Character class.
            b'[' => {
                if c == 0 {
                    return false;
                }
                let inverted = at(pat, p) == b'!';
                let mut class = if inverted { p + 1 } else { p };
                let mut a = at(pat, class);
                class += 1;
                let mut matched = false;
                let mut malformed = false;

                // Each span is a single character a or a range a-b. The
                // first span may begin with ].
                loop {
                    let mut b = a;
                    if a == 0 {
                        malformed = true;
                        break;
                    }
                    if at(pat, class) == b'-' && at(pat, class + 1) != b']' {
                        b = at(pat, class + 1);
                        if b == 0 {
                            malformed = true;
                            break;
                        }
                        class += 2;
                    }
                    if a <= c && c <= b {
                        matched = true;
                    }
                    a = at(pat, class);
                    class += 1;
                    if a == b']' {
                        break;
                    }
                }

                if malformed {
                    Step::Literal(d)
                } else if matched == inverted {
                    Step::Backtrack
                } else {
                    p = class;
                    Step::Next
                }
            }
            b'\\' => {
                let d = at(pat, p);
                p += 1;
                Step::Literal(d)
            }
            _ => Step::Literal(d),
        };

        match step {
            Step::Next => continue,
            Step::Literal(d) if c == d => {
                if d == 0 {
                    return true;
                }
                continue;
            }
            Step::Literal(_) | Step::Backtrack => {}
        }

        // Backtrack.
        let Some(bp) = back_pat else { return false };
        if c == 0 {
            return false;
        }
        // Try again from the last *, one character later in the string.
        p = bp;
        back_str += 1;
        si = back_str;
    }
}
