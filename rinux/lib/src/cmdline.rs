// SPDX-License-Identifier: GPL-2.0-only
//! Helpers for parsing the kernel command line and module options.
//!
//! Ported from `lib/cmdline.c`. Code and copyrights come from init/main.c
//! and arch/i386/kernel/setup.c.
//!
//! Strings follow the C-string convention of [`crate::string`]. Where C
//! advances a `char **` through the string, these functions advance a
//! `&mut &[u8]` or return the rest of the string.
//!
//! [`next_arg`] returns sub-slices instead of writing NULs into the
//! command line; [`next_arg_in_place`] is the C version that does.

use crate::ctype::isspace;
use crate::string::{skip_spaces, strlen, strncmp};
use crate::vsprintf::{simple_strtol, simple_strtoull};

#[inline]
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

#[inline]
fn sub(s: &[u8], i: usize) -> &[u8] {
    s.get(i..).unwrap_or(&[])
}

/// `get_range()`: after [`get_option`] found a `-`, expands the range
/// `M-N` into `ints[0..]` (which holds M) as M, M+1, ..., N-1, writing at
/// most `n` values. Returns `N - M`. The upper number is left for the next
/// [`get_option`] call.
fn get_range(s: &mut &[u8], ints: &mut [i32], n: usize) -> i32 {
    *s = sub(s, 1);
    let upper_range = simple_strtol(*s, 0).0 as i32;
    let first = ints[0];
    let inc_counter = upper_range.wrapping_sub(first);
    let mut x = first;
    let mut n = n;
    let mut i = 0;
    while n != 0 && x < upper_range {
        ints[i] = x;
        i += 1;
        x += 1;
        n -= 1;
    }
    inc_counter
}

/// `get_option()`: reads an integer at the start of `*s` and a comma
/// after it.
///
/// Stores the integer in `pint` (if given) and advances `*s` past it.
/// Returns 0 if there is no integer, 1 if there is one, 2 if a comma
/// follows (and is skipped), and 3 if a `-` follows, denoting a range. A
/// leading `-` without an integer is consumed but counts as no integer.
pub fn get_option(s: &mut &[u8], pint: Option<&mut i32>) -> i32 {
    let mut cur = *s;
    if at(cur, 0) == 0 {
        return 0;
    }
    let value = if at(cur, 0) == b'-' {
        cur = &cur[1..];
        let (v, end) = simple_strtoull(cur, 0);
        *s = sub(cur, end);
        v.wrapping_neg() as i32
    } else {
        let (v, end) = simple_strtoull(cur, 0);
        *s = sub(cur, end);
        v as i32
    };
    if let Some(pint) = pint {
        *pint = value;
    }
    if cur.len() == s.len() {
        return 0;
    }
    match at(s, 0) {
        b',' => {
            *s = &s[1..];
            2
        }
        b'-' => 3,
        _ => 1,
    }
}

/// `get_options()`: parses a list of integers such as `"1,3-5,8"` into
/// `ints`.
///
/// The list holds comma-separated integers and hyphenated ranges of
/// positive integers. Parsing stops when `nints - 1` values have been
/// stored or no more numbers can be read. `ints[0]` receives the number
/// of values, which follow in `ints[1..]`.
///
/// With `nints == 0` the string is only validated: `ints[0]` receives the
/// number of values and nothing else is written.
///
/// Returns the rest of the string from the byte that ended parsing (empty
/// if it all parsed).
pub fn get_options<'a>(s: &'a [u8], nints: i32, ints: &mut [i32]) -> &'a [u8] {
    let validate = nints == 0;
    let mut s = s;
    let mut i: i32 = 1;

    while i < nints || validate {
        let idx = if validate { 0 } else { i as usize };

        let res = get_option(&mut s, Some(&mut ints[idx]));
        if res == 0 {
            break;
        }
        if res == 3 {
            let n = if validate { 0 } else { (nints - i) as usize };
            let range_nums = get_range(&mut s, &mut ints[idx..], n);
            if range_nums < 0 {
                break;
            }
            // Decrement the result by one to leave out the last number in
            // the range. The next iteration will handle the upper number.
            i = i.wrapping_add(range_nums - 1);
        }
        i = i.wrapping_add(1);
        if res == 1 {
            break;
        }
    }
    ints[0] = i - 1;
    s
}

/// `memparse()`: parses a size such as `"64M"`.
///
/// The number is read by [`simple_strtoull`] and may be followed by one of
/// the suffixes K, M, G, T, P, E (either case), which multiply it by
/// powers of 1024. A value that overflows becomes `u64::MAX`; a valid
/// suffix is consumed even then. Returns the value and the index after
/// what was parsed.
pub fn memparse(ptr: &[u8]) -> (u64, usize) {
    let (ret, mut endptr) = simple_strtoull(ptr, 0);
    let shl = match at(ptr, endptr) {
        b'E' | b'e' => 60,
        b'P' | b'p' => 50,
        b'T' | b't' => 40,
        b'G' | b'g' => 30,
        b'M' | b'm' => 20,
        b'K' | b'k' => 10,
        _ => 0,
    };

    let mut ret = ret;
    if shl != 0 && endptr != 0 {
        // Have valid suffix with preceding number.
        ret = if ret.leading_zeros() < shl {
            u64::MAX
        } else {
            ret << shl
        };
        endptr += 1;
    }
    (ret, endptr)
}

/// `parse_option_str()`: is `option` one of the comma-separated items of
/// `s`? Items may carry values (`a=b,c`); only an exact `option` item
/// matches.
pub fn parse_option_str(s: &[u8], option: &[u8]) -> bool {
    let olen = strlen(option);
    let mut i = 0;
    while at(s, i) != 0 {
        if strncmp(sub(s, i), option, olen) == 0 {
            i += olen;
            if at(s, i) == 0 || at(s, i) == b',' {
                return true;
            }
        }
        while at(s, i) != 0 && at(s, i) != b',' {
            i += 1;
        }
        if at(s, i) == b',' {
            i += 1;
        }
    }
    false
}

/// Where `next_arg()` found the parts of an argument.
struct ArgScan {
    /// The argument started with `"`, which is skipped.
    quoted: bool,
    /// Index of the first `=` after the skipped quote, 0 if none.
    equals: usize,
    /// Length of the argument after the skipped quote.
    len: usize,
}

/// The scanning loop of `next_arg()`: `"` quotes spaces but cannot be
/// escaped.
fn scan_arg(args: &[u8]) -> ArgScan {
    let quoted = at(args, 0) == b'"';
    let args = if quoted { &args[1..] } else { args };
    let mut in_quote = quoted;
    let mut equals = 0;
    let mut i = 0;
    while at(args, i) != 0 {
        let c = args[i];
        if isspace(c) && !in_quote {
            break;
        }
        if equals == 0 && c == b'=' {
            equals = i;
        }
        if c == b'"' {
            in_quote = !in_quote;
        }
        i += 1;
    }
    ArgScan {
        quoted,
        equals,
        len: i,
    }
}

/// `next_arg()`: splits the next `param=value` pair off the start of
/// `args`. Returns the parameter, its value if it has one, and the rest
/// after the white space that follows.
///
/// `"` quotes spaces but cannot be escaped; quotes around the value or
/// around the whole argument are removed. A `=` at the very start does not
/// count. The results are the C strings that [`next_arg_in_place`] leaves
/// in the buffer, and the rest is empty at the end of the string.
pub fn next_arg(args: &[u8]) -> (&[u8], Option<&[u8]>, &[u8]) {
    let args = &args[..strlen(args)];
    let ArgScan {
        quoted,
        equals,
        len: i,
    } = scan_arg(args);
    let args = if quoted { &args[1..] } else { args };

    // C ends the value (or a quoted parameter) at a closing quote...
    let close_quote = i > 0 && args[i - 1] == b'"';
    let mut end = i;
    let val = if equals == 0 {
        None
    } else {
        let mut start = equals + 1;
        // Don't include quotes in value.
        if at(args, start) == b'"' {
            start += 1;
            if close_quote {
                end = i - 1;
            }
        }
        if quoted && close_quote {
            end = i - 1;
        }
        Some(&args[start..end.max(start)])
    };
    // ...and the parameter at the '='.
    let param_end = if equals != 0 {
        equals
    } else if quoted && close_quote {
        i - 1
    } else {
        i
    };

    let rest = if i < args.len() {
        &args[i + 1..]
    } else {
        &args[i..]
    };
    // Chew up trailing spaces.
    (&args[..param_end], val, &rest[skip_spaces(rest)..])
}

/// What [`next_arg_in_place`] found: indices into the buffer of the
/// parameter, its value if there is one, and the rest of the arguments.
/// Each of them is a C string that `next_arg_in_place` terminated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NextArg {
    pub param: usize,
    pub val: Option<usize>,
    pub next: usize,
}

/// `next_arg()` as in C: splits the next `param=value` pair off
/// `buf[args..]`, writing NULs into `buf` to end the parameter and value.
/// See [`next_arg`] for the rules.
pub fn next_arg_in_place(buf: &mut [u8], args: usize) -> NextArg {
    let ArgScan {
        quoted,
        equals,
        len: i,
    } = scan_arg(sub(buf, args));
    let mut args = if quoted { args + 1 } else { args };

    let param = args;
    let val = if equals == 0 {
        None
    } else {
        buf[args + equals] = 0;
        let mut val = args + equals + 1;
        // Don't include quotes in value.
        if at(buf, val) == b'"' {
            val += 1;
            if buf[args + i - 1] == b'"' {
                buf[args + i - 1] = 0;
            }
        }
        Some(val)
    };
    if quoted && i > 0 && buf[args + i - 1] == b'"' {
        buf[args + i - 1] = 0;
    }

    if at(buf, args + i) != 0 {
        buf[args + i] = 0;
        args += i + 1;
    } else {
        args += i;
    }

    // Chew up trailing spaces.
    NextArg {
        param,
        val,
        next: args + skip_spaces(sub(buf, args)),
    }
}
