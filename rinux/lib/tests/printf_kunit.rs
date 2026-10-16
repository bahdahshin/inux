// SPDX-License-Identifier: GPL-2.0-only
//! Port of `lib/tests/printf_kunit.c`.
//!
//! Every format is run with a 256-byte buffer, with every size from 1 to
//! its length + 1 (C picks one at random) and with size 0. The
//! `kvasprintf()` check needs the allocator and is not ported.
//!
//! Cases for `%p` extensions that are not ported yet are not run:
//! `struct_resource` (`%pr`, `%pR`), `struct_range` (`%pra`), the sockaddr
//! lines of `ip4` and `ip6` (`%p[iI]S`), `dentry` (`%pd`, `%pD`) apart from
//! its NULL and invalid pointer lines, `time_and_date` (`%pt`) apart from
//! `(%pt?)`, `flags` (`%pG`) and `fwnode_pointer` (`%pfw`). `symbol_ptr`,
//! `addr`, `escaped_str`, `struct_va_format`, `struct_clk` and
//! `netdev_features` are empty in C.

use rinux_lib::bitmap::{bitmap_fill, bitmap_set, bitmap_zero};
use rinux_lib::bitops::set_bit;
use rinux_lib::errno::{EAGAIN, EIO, ENOTSOCK, EPROBE_DEFER, EWOULDBLOCK};
use rinux_lib::siphash::SiphashKey;
use rinux_lib::snprintf;
use rinux_lib::string::strlen;
use rinux_lib::vsprintf::{
    Arg, fill_ptr_key, kptr_restrict, no_hash_pointers, set_kptr_restrict, vsnprintf,
};

const BUF_SIZE: usize = 256;
const PAD_SIZE: usize = 16;
const FILL_CHAR: u8 = b'$';

fn show(s: &[u8]) -> String {
    String::from_utf8_lossy(s).into_owned()
}

fn do_test(line: u32, bufsize: usize, expect: &[u8], fmt: &[u8], args: &[Arg]) {
    let mut alloced_buffer = [FILL_CHAR; BUF_SIZE + 2 * PAD_SIZE];
    let elen = expect.len();
    let fmt_s = show(fmt);

    let ret = vsnprintf(&mut alloced_buffer[PAD_SIZE..PAD_SIZE + bufsize], fmt, args);
    assert_eq!(
        ret, elen,
        "line {line}: vsnprintf(buf, {bufsize}, \"{fmt_s}\", ...) returned {ret}, expected {elen}"
    );

    assert!(
        alloced_buffer[..PAD_SIZE].iter().all(|&c| c == FILL_CHAR),
        "line {line}: vsnprintf(buf, {bufsize}, \"{fmt_s}\", ...) wrote before buffer"
    );

    let test_buffer = &alloced_buffer[PAD_SIZE..];
    if bufsize == 0 {
        assert!(
            test_buffer.iter().all(|&c| c == FILL_CHAR),
            "line {line}: vsnprintf(buf, 0, \"{fmt_s}\", ...) wrote to buffer"
        );
        return;
    }

    let written = (bufsize - 1).min(elen);
    assert_eq!(
        test_buffer[written], 0,
        "line {line}: vsnprintf(buf, {bufsize}, \"{fmt_s}\", ...) did not nul-terminate buffer"
    );
    assert!(
        test_buffer[written + 1..bufsize]
            .iter()
            .all(|&c| c == FILL_CHAR),
        "line {line}: vsnprintf(buf, {bufsize}, \"{fmt_s}\", ...) wrote beyond the nul-terminator"
    );
    assert!(
        test_buffer[bufsize..BUF_SIZE + PAD_SIZE]
            .iter()
            .all(|&c| c == FILL_CHAR),
        "line {line}: vsnprintf(buf, {bufsize}, \"{fmt_s}\", ...) wrote beyond buffer"
    );
    assert_eq!(
        show(&test_buffer[..written]),
        show(&expect[..written]),
        "line {line}: vsnprintf(buf, {bufsize}, \"{fmt_s}\", ...)"
    );
}

fn __test(line: u32, expect: &[u8], fmt: &[u8], args: &[Arg]) {
    let elen = expect.len();
    assert!(
        elen < BUF_SIZE,
        "line {line}: error in test suite: expected length ({elen}) >= BUF_SIZE"
    );

    do_test(line, BUF_SIZE, expect, fmt, args);
    for size in 1..=elen + 1 {
        do_test(line, size, expect, fmt, args);
    }
    do_test(line, 0, expect, fmt, args);
}

/// `test(expect, fmt, ...)`.
macro_rules! test {
    ($expect:expr, $fmt:expr $(, $arg:expr)* $(,)?) => {
        __test(
            line!(),
            AsRef::<[u8]>::as_ref(&$expect),
            AsRef::<[u8]>::as_ref(&$fmt),
            &[$(Arg::from($arg)),*],
        )
    };
}

#[test]
fn test_basic() {
    // Work around annoying "warning: zero-length gnu_printf format string".
    test!("", b"\0");
    test!("100%", "100%%");
    test!("xxx%yyy", "xxx%cyyy", '%');
    test!(b"xxx\0yyy", "xxx%cyyy", '\0');
}

#[test]
fn test_number() {
    test!("0x1234abcd  ", "%#-12x", 0x1234abcd);
    test!("  0x1234abcd", "%#12x", 0x1234abcd);
    test!(
        "0|001| 12|+123| 1234|-123|-1234",
        "%d|%03d|%3d|%+d|% d|%+d|% d",
        0,
        1,
        12,
        123,
        1234,
        -123,
        -1234
    );
    // Intentionally test narrowing conversion specifiers.
    test!(
        "0|1|1|128|255",
        "%hhu|%hhu|%hhu|%hhu|%hhu",
        0,
        1,
        257,
        128,
        -1
    );
    test!(
        "0|1|1|-128|-1",
        "%hhd|%hhd|%hhd|%hhd|%hhd",
        0,
        1,
        257,
        128,
        -1
    );
    test!("2015122420151225", "%ho%ho%#ho", 1037, 5282, -11627);
    // POSIX/C99: "The result of converting zero with an explicit precision
    // of zero shall be no characters." Hence the output from the below test
    // should really be "00|0||| ". However, the kernel's printf also
    // produces a single 0 in that case. This test case simply documents the
    // current behaviour.
    test!("00|0|0|0|0", "%.2d|%.1d|%.0d|%.*d|%1.0d", 0, 0, 0, 0, 0, 0);
}

#[test]
fn test_string() {
    test!("", "%s%.0s", "", "123");
    test!(
        "ABCD|abc|123",
        "%s|%.3s|%.*s",
        "ABCD",
        "abcdef",
        3,
        "123456"
    );
    test!(
        "1  |  2|3  |  4|5  ",
        "%-3s|%3s|%-*s|%*s|%*s",
        "1",
        "2",
        3,
        "3",
        3,
        "4",
        -3,
        "5"
    );
    test!("1234      ", "%-10.4s", "123456");
    test!("      1234", "%10.4s", "123456");
    // POSIX and C99 say that a negative precision (which is only possible to
    // pass via a * argument) should be treated as if the precision wasn't
    // present, and that if the precision is omitted (as in %.s), the
    // precision should be taken to be 0. However, the kernel's printf
    // behave exactly opposite, treating a negative precision as 0 and
    // treating an omitted precision specifier as if no precision was given.
    //
    // These test cases document the current behaviour; should anyone ever
    // feel the need to follow the standards more closely, this can be
    // revisited.
    test!("    ", "%4.*s", -5, "123456");
    test!("123456", "%.s", "123456");
    test!("a||", "%.s|%.0s|%.*s", "a", "b", 0, "c");
    test!("a  |   |   ", "%-3.s|%-3.0s|%-3.*s", "a", "b", 0, "c");
}

const PLAIN_BUF_SIZE: usize = 64;
const PTR_WIDTH: usize = 16;
const PTR: Arg = Arg::Ptr(0xffff0123456789ab);
const PTR_STR: &str = "ffff0123456789ab";
const PTR_VAL_NO_CRNG: &[u8] = b"(____ptrval____)";
const ZEROS: &str = "00000000"; // hex 32 zero bits
const ONES: &str = "ffffffff"; // hex 32 one bits
const PTR_INVALID: Arg = Arg::Ptr(0x000000ab);

/// `plain_hash_to_buffer()`: `%p` of `p`. Returns false (KUnit skips) if
/// the pointer key is not set yet.
fn plain_hash_to_buffer(p: Arg, buf: &mut [u8; PLAIN_BUF_SIZE]) -> bool {
    assert_eq!(snprintf!(buf, "%p", p), PTR_WIDTH);
    &buf[..PTR_WIDTH] != PTR_VAL_NO_CRNG
}

fn hash_pointer() {
    if no_hash_pointers() {
        return; // hash pointers disabled
    }
    let mut buf = [0u8; PLAIN_BUF_SIZE];
    assert!(plain_hash_to_buffer(PTR, &mut buf));

    // The hash of %p is unpredictable, therefore test() cannot be used.
    //
    // Instead verify that the first 32 bits are zeros on a 64-bit system
    // and that the non-hashed value is not printed.
    assert_eq!(&buf[..ZEROS.len()], ZEROS.as_bytes());
    assert_ne!(&buf[..PTR_WIDTH], PTR_STR.as_bytes());
}

/// `test_hashed(fmt, p)`: `fmt` prints `p` like `%p` does.
fn test_hashed(line: u32, fmt: &str, p: Arg) {
    let mut buf = [0u8; PLAIN_BUF_SIZE];
    assert!(plain_hash_to_buffer(p, &mut buf));
    __test(line, &buf[..strlen(&buf)], fmt.as_bytes(), &[p]);
}

#[test]
fn null_pointer() {
    // NULL pointers aren't hashed.
    test!(format!("{ZEROS}00000000"), "%p", Arg::Ptr(0));
    test!(format!("{ZEROS}00000000"), "%px", Arg::Ptr(0));
    test!("(null)", "%pE", Arg::Ptr(0));
}

#[test]
fn error_pointer() {
    // Error pointers aren't hashed.
    test!(format!("{ONES}fffffff5"), "%p", Arg::err_ptr(-11));
    test!(format!("{ONES}fffffff5"), "%px", Arg::err_ptr(-11));
    test!("(efault)", "%pE", Arg::err_ptr(-11));
}

fn invalid_pointer() {
    test_hashed(line!(), "%p", PTR_INVALID);
    test!(format!("{ZEROS}000000ab"), "%px", PTR_INVALID);
    test!("(efault)", "%pE", PTR_INVALID);
}

fn kernel_ptr() {
    match kptr_restrict() {
        0 => {
            if no_hash_pointers() {
                test!(PTR_STR, "%pK", PTR);
            } else {
                let mut buf = [0u8; PLAIN_BUF_SIZE];
                assert!(plain_hash_to_buffer(PTR, &mut buf));
                // %pK behaves the same as hashing.
                test!(&buf[..strlen(&buf)], "%pK", PTR);
            }
        }
        // The KUnit kthread has all capabilities, including CAP_SYSLOG.
        1 => test!(PTR_STR, "%pK", PTR),
        _ => test!(format!("{ZEROS}00000000"), "%pK", PTR),
    }
}

/// The tests that depend on the pointer key and `kptr_restrict`. They run
/// in one test function because both are global.
#[test]
fn hashing() {
    // Before the key is set, %p prints a placeholder (the C test skips).
    let mut buf = [0u8; PLAIN_BUF_SIZE];
    assert!(!plain_hash_to_buffer(PTR, &mut buf));

    fill_ptr_key(SiphashKey {
        key: [0x0706050403020100, 0x0f0e0d0c0b0a0908],
    });
    hash_pointer();
    invalid_pointer();
    for restrict in [0, 1, 2] {
        set_kptr_restrict(restrict);
        kernel_ptr();
    }
    set_kptr_restrict(0);

    // From errptr(): %pe with a non-ERR_PTR gets treated as ordinary %p.
    test_hashed(line!(), "%pe", PTR);
}

#[test]
fn hex_string() {
    let buf: [u8; 3] = [0xc0, 0xff, 0xee];

    test!(
        "c0 ff ee|c0:ff:ee|c0-ff-ee|c0ffee",
        "%3ph|%3phC|%3phD|%3phN",
        &buf,
        &buf,
        &buf,
        &buf
    );
    test!(
        "c0 ff ee|c0:ff:ee|c0-ff-ee|c0ffee",
        "%*ph|%*phC|%*phD|%*phN",
        3,
        &buf,
        3,
        &buf,
        3,
        &buf,
        3,
        &buf
    );
}

#[test]
fn mac() {
    let addr: [u8; 6] = [0x2d, 0x48, 0xd6, 0xfc, 0x7a, 0x05];

    test!("2d:48:d6:fc:7a:05", "%pM", &addr);
    test!("05:7a:fc:d6:48:2d", "%pMR", &addr);
    test!("05:7A:FC:D6:48:2D", "%pMRU", &addr);
    test!("2d-48-d6-fc-7a-05", "%pMF", &addr);
    test!("2d48d6fc7a05", "%pm", &addr);
    test!("2D48D6FC7A05", "%pmU", &addr);
    test!("057afcd6482d", "%pmR", &addr);
}

#[test]
fn ip4() {
    // sa.sin_addr.s_addr = cpu_to_be32(0x7f000001)
    let sin_addr = 0x7f000001u32.to_be_bytes();
    test!(
        "127.000.000.001|127.0.0.1",
        "%pi4|%pI4",
        &sin_addr,
        &sin_addr
    );
}

#[test]
fn ip6() {
    let addr: [u8; 16] = [
        0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00, 0x05, 0x00, 0x06, 0x00, 0x07, 0x00,
        0x08,
    ];
    let single_zero: [u8; 16] = [
        0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x05, 0x00, 0x06, 0x00, 0x07, 0x00,
        0x08,
    ];

    test!(
        "00010002000300040005000600070008|0001:0002:0003:0004:0005:0006:0007:0008",
        "%pi6|%pI6",
        &addr,
        &addr
    );
    test!("1:2:3:4:5:6:7:8", "%pI6c", &addr);
    test!("1:0:3:4:5:6:7:8", "%pI6c", &single_zero);
}

#[test]
fn uuid() {
    let uuid: [u8; 16] = [
        0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf,
    ];

    test!("00010203-0405-0607-0809-0a0b0c0d0e0f", "%pUb", &uuid);
    test!("00010203-0405-0607-0809-0A0B0C0D0E0F", "%pUB", &uuid);
    test!("03020100-0504-0706-0809-0a0b0c0d0e0f", "%pUl", &uuid);
    test!("03020100-0504-0706-0809-0A0B0C0D0E0F", "%pUL", &uuid);
}

#[test]
fn dentry() {
    test!("(null)", "%pd", Arg::Ptr(0));
    test!("(efault)", "%pd", PTR_INVALID);
    test!("(null)", "%pD", Arg::Ptr(0));
    test!("(efault)", "%pD", PTR_INVALID);
}

#[test]
fn time_and_date() {
    // 1543210543: struct rtc_time { 43, 35, 5, 26, 10, 118 }
    let tm: [u8; 36] = [0; 36];
    test!("(%pt?)", "%pt", &tm);
}

fn large_bitmap() {
    const NBITS: usize = 1 << 16;
    let mut bits = vec![0u64; NBITS / 64];

    bitmap_set(&mut bits, 1, 20);
    bitmap_set(&mut bits, 60000, 15);
    test!("1-20,60000-60014", "%*pbl", NBITS, &bits[..]);
}

#[test]
fn bitmap() {
    let mut bits = [0u64; 1];
    let primes = [2, 3, 5, 7, 11, 13, 17, 19];

    bitmap_zero(&mut bits, 20);
    test!("00000|00000", "%20pb|%*pb", &bits, 20, &bits);
    test!("|", "%20pbl|%*pbl", &bits, 20, &bits);

    for p in primes {
        set_bit(p, &mut bits);
    }
    test!("a28ac|a28ac", "%20pb|%*pb", &bits, 20, &bits);
    test!(
        "2-3,5,7,11,13,17,19|2-3,5,7,11,13,17,19",
        "%20pbl|%*pbl",
        &bits,
        20,
        &bits
    );

    bitmap_fill(&mut bits, 20);
    test!("fffff|fffff", "%20pb|%*pb", &bits, 20, &bits);
    test!("0-19|0-19", "%20pbl|%*pbl", &bits, 20, &bits);

    large_bitmap();
}

#[test]
fn errptr() {
    test!("-1234", "%pe", Arg::err_ptr(-1234));

    // CONFIG_SYMBOLIC_ERRNAME
    test!("(-ENOTSOCK)", "(%pe)", Arg::err_ptr(ENOTSOCK.to_errno()));
    test!("(-EAGAIN)", "(%pe)", Arg::err_ptr(EAGAIN.to_errno()));
    assert_eq!(EAGAIN, EWOULDBLOCK);
    test!("(-EAGAIN)", "(%pe)", Arg::err_ptr(EWOULDBLOCK.to_errno()));
    test!("[-EIO    ]", "[%-8pe]", Arg::err_ptr(EIO.to_errno()));
    test!("[    -EIO]", "[%8pe]", Arg::err_ptr(EIO.to_errno()));
    test!(
        "-EPROBE_DEFER",
        "%pe",
        Arg::err_ptr(EPROBE_DEFER.to_errno())
    );
}

fn fourcc_pointer_test(fc: &[(u32, &str)], fmt: &str) {
    for &(code, s) in fc {
        __test(
            line!(),
            s.as_bytes(),
            fmt.as_bytes(),
            &[Arg::from(&code.to_ne_bytes())],
        );
    }
}

#[test]
fn fourcc_pointer() {
    let try_cc = [
        (0x3231564e, "NV12 little-endian (0x3231564e)"),
        (0xb231564e, "NV12 big-endian (0xb231564e)"),
        (0x10111213, ".... little-endian (0x10111213)"),
        (0x20303159, "Y10  little-endian (0x20303159)"),
    ];
    let try_ch = [(0x41424344, "ABCD (0x41424344)")];
    let try_ch_r = [(0x41424344, "DCBA (0x44434241)")];
    let try_cl = [(0x41424344u32.to_le(), "ABCD (0x41424344)")];
    let try_cb = [(0x41424344u32.to_be(), "ABCD (0x41424344)")];

    fourcc_pointer_test(&try_cc, "%p4cc");
    fourcc_pointer_test(&try_ch, "%p4ch");
    fourcc_pointer_test(&try_ch_r, "%p4chR");
    fourcc_pointer_test(&try_cl, "%p4cl");
    fourcc_pointer_test(&try_cb, "%p4cb");
}
