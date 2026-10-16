// SPDX-License-Identifier: GPL-2.0-only
//! Port of `lib/tests/scanf_kunit.c`.
//!
//! The C macros are generic functions over the integer types here. C
//! `long` and `long long` are both `i64`, `char` is `i8` as on x86. Each
//! test function seeds its own random state with the suite's seed.

use std::fmt::Debug;

use rinux_lib::random32::{RndState, prandom_seed_state, prandom_u32_state};
use rinux_lib::snprintf;
use rinux_lib::string::strlen;
use rinux_lib::vsprintf::{
    Arg, ScanArg, simple_strtol, simple_strtoll, simple_strtoul, simple_strtoull, vsnprintf,
    vsscanf,
};

const BUF_SIZE: usize = 1024;

trait TestInt: Copy + PartialEq + Debug + 'static {
    const SIGNED: bool;
    const BITS: u32;
    const MIN: i128;
    const MAX: i128;
    /// `(T)val`.
    fn cast(val: u64) -> Self;
    fn not(self) -> Self;
    fn neg(self) -> Self;
    fn arg(self) -> Arg<'static>;
    fn scan_arg(r: &mut Self) -> ScanArg<'_>;
}

macro_rules! test_int {
    ($($t:ty: $signed:expr),*) => {
        $(
            impl TestInt for $t {
                const SIGNED: bool = $signed;
                const BITS: u32 = <$t>::BITS;
                const MIN: i128 = <$t>::MIN as i128;
                const MAX: i128 = <$t>::MAX as i128;
                fn cast(val: u64) -> Self {
                    val as $t
                }
                fn not(self) -> Self {
                    !self
                }
                fn neg(self) -> Self {
                    self.wrapping_neg()
                }
                fn arg(self) -> Arg<'static> {
                    Arg::from(self)
                }
                fn scan_arg(r: &mut Self) -> ScanArg<'_> {
                    ScanArg::from(r)
                }
            }
        )*
    };
}

test_int!(u8: false, i8: true, u16: false, i16: true, u32: false, i32: true, u64: false, i64: true);

/// `_test()`: scans `string` with `fmt` into `results` and checks that
/// `n_args` values were converted and equal `expect`.
fn _test<T: TestInt>(
    line: u32,
    expect: &[T],
    string: &[u8],
    fmt: &[u8],
    n_args: usize,
    results: &mut [T],
) {
    let mut args: Vec<ScanArg> = results.iter_mut().map(T::scan_arg).collect();
    let ret = vsscanf(string, fmt, &mut args);
    let (s, f) = (
        String::from_utf8_lossy(string),
        String::from_utf8_lossy(fmt),
    );
    assert_eq!(
        ret, n_args,
        "line {line}: vsscanf(\"{s}\", \"{f}\", ...) returned {ret} expected {n_args}"
    );
    drop(args);
    for i in 0..n_args {
        assert_eq!(
            results[i], expect[i],
            "line {line}: vsscanf(\"{s}\", \"{f}\", ...) value {i}"
        );
    }
}

/// Selection of interesting numbers to test, copied from test-kstrtox.c.
const NUMBERS: &[u64] = &[
    0x0,
    0x1,
    0x7f,
    0x80,
    0x81,
    0xff,
    0x100,
    0x101,
    0x7fff,
    0x8000,
    0x8001,
    0xffff,
    0x10000,
    0x10001,
    0x7fffffff,
    0x80000000,
    0x80000001,
    0xffffffff,
    0x100000000,
    0x100000001,
    0x7fffffffffffffff,
    0x8000000000000000,
    0x8000000000000001,
    0xfffffffffffffffe,
    0xffffffffffffffff,
];

fn value_representable_in_type<T: TestInt>(val: u64) -> bool {
    if T::SIGNED {
        (val as i64 as i128) >= T::MIN && (val as i64 as i128) <= T::MAX
    } else {
        (val as i128) <= T::MAX
    }
}

fn test_one_number<T: TestInt>(line: u32, gen_fmt: &str, scan_fmt: &str, val: u64) {
    let expect_val = T::cast(val);
    let mut result = [expect_val.not()]; // should be overwritten
    let mut test_buffer = [0u8; BUF_SIZE];

    snprintf!(&mut test_buffer, gen_fmt, expect_val.arg());
    let fmt = format!("%{scan_fmt}");
    _test(
        line,
        &[expect_val],
        &test_buffer,
        fmt.as_bytes(),
        1,
        &mut result,
    );
}

fn simple_numbers_loop<T: TestInt>(line: u32, gen_fmt: &str, scan_fmt: &str) {
    for &n in NUMBERS {
        if value_representable_in_type::<T>(n) {
            test_one_number::<T>(line, gen_fmt, scan_fmt, n);
        }
        if value_representable_in_type::<T>(n.wrapping_neg()) {
            test_one_number::<T>(line, gen_fmt, scan_fmt, n.wrapping_neg());
        }
    }
}

#[test]
fn numbers_simple() {
    macro_rules! l {
        ($t:ty, $gen:expr, $scan:expr) => {
            simple_numbers_loop::<$t>(line!(), $gen, $scan)
        };
    }
    l!(u64, "%llu", "llu");
    l!(i64, "%lld", "lld");
    l!(i64, "%lld", "lli");
    l!(u64, "%llx", "llx");
    l!(i64, "%llx", "llx");
    l!(i64, "0x%llx", "lli");
    l!(u64, "0x%llx", "llx");
    l!(i64, "0x%llx", "llx");

    l!(u64, "%lu", "lu");
    l!(i64, "%ld", "ld");
    l!(i64, "%ld", "li");
    l!(u64, "%lx", "lx");
    l!(i64, "%lx", "lx");
    l!(i64, "0x%lx", "li");
    l!(u64, "0x%lx", "lx");
    l!(i64, "0x%lx", "lx");

    l!(u32, "%u", "u");
    l!(i32, "%d", "d");
    l!(i32, "%d", "i");
    l!(u32, "%x", "x");
    l!(i32, "%x", "x");
    l!(i32, "0x%x", "i");
    l!(u32, "0x%x", "x");
    l!(i32, "0x%x", "x");

    l!(u16, "%hu", "hu");
    l!(i16, "%hd", "hd");
    l!(i16, "%hd", "hi");
    l!(u16, "%hx", "hx");
    l!(i16, "%hx", "hx");
    l!(i16, "0x%hx", "hi");
    l!(u16, "0x%hx", "hx");
    l!(i16, "0x%hx", "hx");

    l!(u8, "%hhu", "hhu");
    l!(i8, "%hhd", "hhd");
    l!(i8, "%hhd", "hhi");
    l!(u8, "%hhx", "hhx");
    l!(i8, "%hhx", "hhx");
    l!(i8, "0x%hhx", "hhi");
    l!(u8, "0x%hhx", "hhx");
    l!(i8, "0x%hhx", "hhx");
}

fn new_rnd_state() -> RndState {
    let mut rnd_state = RndState::new(0);
    prandom_seed_state(&mut rnd_state, 3141592653589793238);
    rnd_state
}

/// This gives a better variety of number "lengths" in a small sample than
/// the raw prandom*() functions (Not mathematically rigorous!!). Variability
/// of length and value is more important than perfect randomness.
fn next_test_random(rnd: &mut RndState, max_bits: u32) -> u32 {
    let n_bits = prandom_u32_state(rnd).count_ones() % (max_bits + 1);
    (prandom_u32_state(rnd) as u64 & rinux_lib::bitops::genmask(n_bits, 0)) as u32
}

fn next_test_random_ull(rnd: &mut RndState) -> u64 {
    let rand1 = prandom_u32_state(rnd);
    let n_bits = (rand1.count_ones() * 3) % 64;
    let val = prandom_u32_state(rnd) as u64 * rand1 as u64;
    val & rinux_lib::bitops::genmask_ull(n_bits, 0)
}

fn random_for_type<T: TestInt>(rnd: &mut RndState) -> T {
    if T::BITS <= 32 {
        T::cast(next_test_random(rnd, T::BITS) as u64)
    } else {
        T::cast(next_test_random_ull(rnd))
    }
}

/// Define a pattern of negative and positive numbers to ensure we get some
/// of both within the small number of samples in a test string.
const NEGATIVES_PATTERN: u32 = 0x3246; // 00110010 01000110

fn fill_random_array<T: TestInt>(rnd: &mut RndState, arr: &mut [T]) {
    let mut neg_pattern = NEGATIVES_PATTERN;
    for a in arr.iter_mut() {
        *a = random_for_type(rnd);
        if T::SIGNED && neg_pattern & 1 != 0 {
            *a = a.neg();
        }
        neg_pattern >>= 1;
    }
}

/// `append_fmt()`: appends `fmt` of `arg` to `buf`; returns its length.
fn append_fmt(buf: &mut Vec<u8>, fmt: &str, args: &[Arg]) -> usize {
    let mut tmp = [0u8; 64];
    let n = vsnprintf(&mut tmp, fmt, args);
    buf.extend_from_slice(&tmp[..n]);
    n
}

/// `numbers_list_8()` and its field width variants: eight random values
/// separated by `delim`, scanned with `scan_fmt` (and a field width given
/// by `width`, if any).
fn numbers_list<T: TestInt>(
    line: u32,
    rnd: &mut RndState,
    gen_fmt: &str,
    delim: &str,
    width: Width,
    scan_fmt: &str,
) {
    let mut expect = [T::cast(0); 8];
    let mut result = [T::cast(0); 8];
    let mut test_buffer = Vec::new();
    let mut fmt_buffer = Vec::new();

    fill_random_array(rnd, &mut expect);

    for (i, &e) in expect.iter().enumerate() {
        if i != 0 {
            test_buffer.extend_from_slice(delim.as_bytes());
            fmt_buffer.extend_from_slice(delim.as_bytes());
        }
        let val_len = append_fmt(&mut test_buffer, gen_fmt, &[e.arg()]);
        match width {
            Width::None => append_fmt(&mut fmt_buffer, "%%%s", &[Arg::from(scan_fmt)]),
            Width::Fixed(w) => append_fmt(
                &mut fmt_buffer,
                "%%%u%s",
                &[Arg::from(w), Arg::from(scan_fmt)],
            ),
            Width::Value => append_fmt(
                &mut fmt_buffer,
                "%%%u%s",
                &[Arg::from(val_len), Arg::from(scan_fmt)],
            ),
        };
    }

    _test(line, &expect, &test_buffer, &fmt_buffer, 8, &mut result);
}

#[derive(Clone, Copy)]
enum Width {
    None,
    Fixed(u32),
    Value,
}

macro_rules! list {
    ($rnd:expr, $t:ty, $gen:expr, $delim:expr, $width:expr, $scan:expr) => {
        numbers_list::<$t>(line!(), $rnd, $gen, $delim, $width, $scan)
    };
}

fn numbers_list_ll(rnd: &mut RndState, delim: &str, w: Width) {
    list!(rnd, u64, "%llu", delim, w, "llu");
    list!(rnd, i64, "%lld", delim, w, "lld");
    list!(rnd, i64, "%lld", delim, w, "lli");
    list!(rnd, u64, "%llx", delim, w, "llx");
    list!(rnd, u64, "0x%llx", delim, w, "llx");
    list!(rnd, i64, "0x%llx", delim, w, "lli");
}

fn numbers_list_l(rnd: &mut RndState, delim: &str, w: Width) {
    list!(rnd, u64, "%lu", delim, w, "lu");
    list!(rnd, i64, "%ld", delim, w, "ld");
    list!(rnd, i64, "%ld", delim, w, "li");
    list!(rnd, u64, "%lx", delim, w, "lx");
    list!(rnd, u64, "0x%lx", delim, w, "lx");
    list!(rnd, i64, "0x%lx", delim, w, "li");
}

fn numbers_list_d(rnd: &mut RndState, delim: &str, w: Width) {
    list!(rnd, u32, "%u", delim, w, "u");
    list!(rnd, i32, "%d", delim, w, "d");
    list!(rnd, i32, "%d", delim, w, "i");
    list!(rnd, u32, "%x", delim, w, "x");
    list!(rnd, u32, "0x%x", delim, w, "x");
    list!(rnd, i32, "0x%x", delim, w, "i");
}

fn numbers_list_h(rnd: &mut RndState, delim: &str, w: Width) {
    list!(rnd, u16, "%hu", delim, w, "hu");
    list!(rnd, i16, "%hd", delim, w, "hd");
    list!(rnd, i16, "%hd", delim, w, "hi");
    list!(rnd, u16, "%hx", delim, w, "hx");
    list!(rnd, u16, "0x%hx", delim, w, "hx");
    list!(rnd, i16, "0x%hx", delim, w, "hi");
}

fn numbers_list_hh(rnd: &mut RndState, delim: &str, w: Width) {
    list!(rnd, u8, "%hhu", delim, w, "hhu");
    list!(rnd, i8, "%hhd", delim, w, "hhd");
    list!(rnd, i8, "%hhd", delim, w, "hhi");
    list!(rnd, u8, "%hhx", delim, w, "hhx");
    list!(rnd, u8, "0x%hhx", delim, w, "hhx");
    list!(rnd, i8, "0x%hhx", delim, w, "hhi");
}

/// Selection of common delimiters/separators between numbers in a string.
const NUMBER_DELIMITERS: &[&str] = &[" ", ":", ",", "-", "/"];

/// String with multiple numbers separated by delimiter.
#[test]
fn numbers_list_test() {
    let rnd = &mut new_rnd_state();
    for delim in NUMBER_DELIMITERS {
        numbers_list_ll(rnd, delim, Width::None);
        numbers_list_l(rnd, delim, Width::None);
        numbers_list_d(rnd, delim, Width::None);
        numbers_list_h(rnd, delim, Width::None);
        numbers_list_hh(rnd, delim, Width::None);
    }
}

/// Field width may be longer than actual field digits: each field width is
/// the maximum possible digits for the given type and base.
#[test]
fn numbers_list_field_width_typemax() {
    let rnd = &mut new_rnd_state();
    for delim in NUMBER_DELIMITERS {
        // numbers_list_field_width_ll()
        list!(rnd, u64, "%llu", delim, Width::Fixed(20), "llu");
        list!(rnd, i64, "%lld", delim, Width::Fixed(20), "lld");
        list!(rnd, i64, "%lld", delim, Width::Fixed(20), "lli");
        list!(rnd, u64, "%llx", delim, Width::Fixed(16), "llx");
        list!(rnd, u64, "0x%llx", delim, Width::Fixed(18), "llx");
        list!(rnd, i64, "0x%llx", delim, Width::Fixed(18), "lli");

        // numbers_list_field_width_l(), BITS_PER_LONG == 64
        list!(rnd, u64, "%lu", delim, Width::Fixed(20), "lu");
        list!(rnd, i64, "%ld", delim, Width::Fixed(20), "ld");
        list!(rnd, i64, "%ld", delim, Width::Fixed(20), "li");
        list!(rnd, u64, "%lx", delim, Width::Fixed(16), "lx");
        list!(rnd, u64, "0x%lx", delim, Width::Fixed(18), "lx");
        list!(rnd, i64, "0x%lx", delim, Width::Fixed(18), "li");

        // numbers_list_field_width_d()
        list!(rnd, u32, "%u", delim, Width::Fixed(10), "u");
        list!(rnd, i32, "%d", delim, Width::Fixed(11), "d");
        list!(rnd, i32, "%d", delim, Width::Fixed(11), "i");
        list!(rnd, u32, "%x", delim, Width::Fixed(8), "x");
        list!(rnd, u32, "0x%x", delim, Width::Fixed(10), "x");
        list!(rnd, i32, "0x%x", delim, Width::Fixed(10), "i");

        // numbers_list_field_width_h()
        list!(rnd, u16, "%hu", delim, Width::Fixed(5), "hu");
        list!(rnd, i16, "%hd", delim, Width::Fixed(6), "hd");
        list!(rnd, i16, "%hd", delim, Width::Fixed(6), "hi");
        list!(rnd, u16, "%hx", delim, Width::Fixed(4), "hx");
        list!(rnd, u16, "0x%hx", delim, Width::Fixed(6), "hx");
        list!(rnd, i16, "0x%hx", delim, Width::Fixed(6), "hi");

        // numbers_list_field_width_hh()
        list!(rnd, u8, "%hhu", delim, Width::Fixed(3), "hhu");
        list!(rnd, i8, "%hhd", delim, Width::Fixed(4), "hhd");
        list!(rnd, i8, "%hhd", delim, Width::Fixed(4), "hhi");
        list!(rnd, u8, "%hhx", delim, Width::Fixed(2), "hhx");
        list!(rnd, u8, "0x%hhx", delim, Width::Fixed(4), "hhx");
        list!(rnd, i8, "0x%hhx", delim, Width::Fixed(4), "hhi");
    }
}

fn numbers_list_field_width_val_width(rnd: &mut RndState, delim: &str) {
    numbers_list_ll(rnd, delim, Width::Value);
    numbers_list_l(rnd, delim, Width::Value);
    numbers_list_d(rnd, delim, Width::Value);
    numbers_list_h(rnd, delim, Width::Value);
    numbers_list_hh(rnd, delim, Width::Value);
}

/// Each field width exactly the length of the value's digits.
#[test]
fn numbers_list_field_width_val_width_test() {
    let rnd = &mut new_rnd_state();
    for delim in NUMBER_DELIMITERS {
        numbers_list_field_width_val_width(rnd, delim);
    }
}

/// Slice a continuous string of digits without field delimiters, containing
/// numbers of varying length, using the field width to extract each group
/// of digits. For example the hex values c0,3,bf01,303 would have a string
/// representation of "c03bf01303" and extracted with "%2x%1x%4x%3x".
#[test]
fn numbers_slice() {
    let rnd = &mut new_rnd_state();
    numbers_list_field_width_val_width(rnd, "");
}

fn test_number_prefix<T: TestInt>(
    line: u32,
    s: &str,
    scan_fmt: &str,
    expect0: u64,
    expect1: u64,
    n_args: usize,
) {
    let expect = [T::cast(expect0), T::cast(expect1)];
    let mut result = [expect[0].not(), expect[1].not()];
    _test(
        line,
        &expect,
        s.as_bytes(),
        scan_fmt.as_bytes(),
        n_args,
        &mut result,
    );
}

/// Number prefix is >= field width. Expected behaviour is derived from
/// testing userland sscanf.
#[test]
fn numbers_prefix_overflow() {
    macro_rules! p {
        ($t:ty, $s:expr, $f:expr, $e0:expr, $e1:expr, $n:expr) => {
            test_number_prefix::<$t>(line!(), $s, $f, $e0, $e1, $n)
        };
    }
    // Negative decimal with a field of width 1, should quit scanning and
    // return 0.
    p!(i64, "-1 1", "%1lld %lld", 0, 0, 0);
    p!(i64, "-1 1", "%1ld %ld", 0, 0, 0);
    p!(i32, "-1 1", "%1d %d", 0, 0, 0);
    p!(i16, "-1 1", "%1hd %hd", 0, 0, 0);
    p!(i8, "-1 1", "%1hhd %hhd", 0, 0, 0);

    p!(i64, "-1 1", "%1lli %lli", 0, 0, 0);
    p!(i64, "-1 1", "%1li %li", 0, 0, 0);
    p!(i32, "-1 1", "%1i %i", 0, 0, 0);
    p!(i16, "-1 1", "%1hi %hi", 0, 0, 0);
    p!(i8, "-1 1", "%1hhi %hhi", 0, 0, 0);

    // 0x prefix in a field of width 1: 0 is a valid digit so should
    // convert. Next field scan starts at the 'x' which isn't a digit so
    // scan quits with one field converted.
    p!(u64, "0xA7", "%1llx%llx", 0, 0, 1);
    p!(u64, "0xA7", "%1lx%lx", 0, 0, 1);
    p!(u32, "0xA7", "%1x%x", 0, 0, 1);
    p!(u16, "0xA7", "%1hx%hx", 0, 0, 1);
    p!(u8, "0xA7", "%1hhx%hhx", 0, 0, 1);
    p!(i64, "0xA7", "%1lli%llx", 0, 0, 1);
    p!(i64, "0xA7", "%1li%lx", 0, 0, 1);
    p!(i32, "0xA7", "%1i%x", 0, 0, 1);
    p!(i16, "0xA7", "%1hi%hx", 0, 0, 1);
    p!(i8, "0xA7", "%1hhi%hhx", 0, 0, 1);

    // 0x prefix in a field of width 2 using %x conversion: first field
    // converts to 0. Next field scan starts at the character after "0x".
    // Both fields will convert.
    p!(u64, "0xA7", "%2llx%llx", 0, 0xa7, 2);
    p!(u64, "0xA7", "%2lx%lx", 0, 0xa7, 2);
    p!(u32, "0xA7", "%2x%x", 0, 0xa7, 2);
    p!(u16, "0xA7", "%2hx%hx", 0, 0xa7, 2);
    p!(u8, "0xA7", "%2hhx%hhx", 0, 0xa7, 2);

    // 0x prefix in a field of width 2 using %i conversion: first field
    // converts to 0. Next field scan starts at the character after "0x",
    // which will convert if can be interpreted as decimal but will fail if
    // it contains any hex digits (since no 0x prefix).
    p!(i64, "0x67", "%2lli%lli", 0, 67, 2);
    p!(i64, "0x67", "%2li%li", 0, 67, 2);
    p!(i32, "0x67", "%2i%i", 0, 67, 2);
    p!(i16, "0x67", "%2hi%hi", 0, 67, 2);
    p!(i8, "0x67", "%2hhi%hhi", 0, 67, 2);

    p!(i64, "0xA7", "%2lli%lli", 0, 0, 1);
    p!(i64, "0xA7", "%2li%li", 0, 0, 1);
    p!(i32, "0xA7", "%2i%i", 0, 0, 1);
    p!(i16, "0xA7", "%2hi%hi", 0, 0, 1);
    p!(i8, "0xA7", "%2hhi%hhi", 0, 0, 1);
}

/// `_test_simple_strtoxx()`: prints `expect` with `gen_fmt` and parses it
/// back with `f`, which must consume the whole string.
fn _test_simple_strtoxx<T: TestInt>(
    gen_fmt: &str,
    expect: T,
    base: u32,
    f: impl Fn(&[u8], u32) -> (T, usize),
) {
    let mut test_buffer = [0u8; BUF_SIZE];
    let len = snprintf!(&mut test_buffer, gen_fmt, expect.arg());
    let (got, endp) = f(&test_buffer, base);
    let s = String::from_utf8_lossy(&test_buffer[..strlen(&test_buffer)]);
    assert_eq!(got, expect, "(\"{s}\", {base})");
    assert_eq!(endp, len, "(\"{s}\", {base}) endp");
}

fn test_simple_strtoxx<T: TestInt>(
    gen_fmt: &str,
    base: u32,
    f: impl Fn(&[u8], u32) -> (T, usize) + Copy,
) {
    for &n in NUMBERS {
        _test_simple_strtoxx(gen_fmt, T::cast(n), base, f);
        if T::SIGNED {
            _test_simple_strtoxx(gen_fmt, T::cast(n).neg(), base, f);
        }
    }
}

#[test]
fn test_simple_strtoull() {
    let f = |s: &[u8], b| simple_strtoull(s, b);
    test_simple_strtoxx::<u64>("%llu", 10, f);
    test_simple_strtoxx::<u64>("%llu", 0, f);
    test_simple_strtoxx::<u64>("%llx", 16, f);
    test_simple_strtoxx::<u64>("0x%llx", 16, f);
    test_simple_strtoxx::<u64>("0x%llx", 0, f);
}

#[test]
fn test_simple_strtoll() {
    let f = |s: &[u8], b| simple_strtoll(s, b);
    test_simple_strtoxx::<i64>("%lld", 10, f);
    test_simple_strtoxx::<i64>("%lld", 0, f);
    test_simple_strtoxx::<i64>("%llx", 16, f);
    test_simple_strtoxx::<i64>("0x%llx", 16, f);
    test_simple_strtoxx::<i64>("0x%llx", 0, f);
}

#[test]
fn test_simple_strtoul() {
    let f = |s: &[u8], b| simple_strtoul(s, b);
    test_simple_strtoxx::<u64>("%lu", 10, f);
    test_simple_strtoxx::<u64>("%lu", 0, f);
    test_simple_strtoxx::<u64>("%lx", 16, f);
    test_simple_strtoxx::<u64>("0x%lx", 16, f);
    test_simple_strtoxx::<u64>("0x%lx", 0, f);
}

#[test]
fn test_simple_strtol() {
    let f = |s: &[u8], b| simple_strtol(s, b);
    test_simple_strtoxx::<i64>("%ld", 10, f);
    test_simple_strtoxx::<i64>("%ld", 0, f);
    test_simple_strtoxx::<i64>("%lx", 16, f);
    test_simple_strtoxx::<i64>("0x%lx", 16, f);
    test_simple_strtoxx::<i64>("0x%lx", 0, f);
}
