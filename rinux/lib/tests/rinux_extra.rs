// SPDX-License-Identifier: GPL-2.0
//! Tests for ported code that has no KUnit test in Linux: ctype, bcd,
//! bitrev, hweight and errno. Step 2's untested code is covered in
//! `rinux_extra_boot.rs`.

use rinux_lib::bcd::*;
use rinux_lib::bitrev::*;
use rinux_lib::ctype::*;
use rinux_lib::errno::{self, Errno};
use rinux_lib::hweight::*;

#[test]
fn ctype_matches_ascii_for_7_bit() {
    for c in 0u8..=0x7f {
        assert_eq!(isalnum(c), c.is_ascii_alphanumeric(), "{c:#x}");
        assert_eq!(isalpha(c), c.is_ascii_alphabetic(), "{c:#x}");
        assert_eq!(iscntrl(c), c.is_ascii_control(), "{c:#x}");
        assert_eq!(isdigit(c), c.is_ascii_digit(), "{c:#x}");
        assert_eq!(isgraph(c), c.is_ascii_graphic(), "{c:#x}");
        assert_eq!(islower(c), c.is_ascii_lowercase(), "{c:#x}");
        assert_eq!(isupper(c), c.is_ascii_uppercase(), "{c:#x}");
        assert_eq!(ispunct(c), c.is_ascii_punctuation(), "{c:#x}");
        assert_eq!(isxdigit(c), c.is_ascii_hexdigit(), "{c:#x}");
        assert_eq!(isprint(c), c.is_ascii_graphic() || c == b' ', "{c:#x}");
        assert_eq!(tolower(c), c.to_ascii_lowercase(), "{c:#x}");
        assert_eq!(toupper(c), c.to_ascii_uppercase(), "{c:#x}");
    }
    // Kernel isspace(): space, \t \n \v \f \r. Rust's omits \v.
    let spaces: Vec<u8> = (0u8..=0x7f).filter(|&c| isspace(c)).collect();
    assert_eq!(spaces, b"\t\n\x0b\x0c\r ");
    assert!(!isspace(0));
}

#[test]
fn ctype_latin1() {
    assert!(isupper(0xc0) && islower(0xe0)); // À à
    assert_eq!(tolower(0xc0), 0xe0);
    assert_eq!(toupper(0xe0), 0xc0);
    assert!(ispunct(0xd7) && ispunct(0xf7)); // × ÷
    assert!(isspace(0xa0) && isprint(0xa0)); // no-break space
    assert!(!isprint(0x80) && !iscntrl(0x80));
}

#[test]
fn bcd_round_trip() {
    for v in 0..=99u32 {
        let b = bin2bcd(v);
        assert!(bcd_is_valid(b));
        assert_eq!(bcd2bin(b), v);
        assert_eq!(b, (((v / 10) << 4) + v % 10) as u8);
    }
    assert!(!bcd_is_valid(0x1a));
    assert!(!bcd_is_valid(0xa1));
}

#[test]
fn bitrev_matches_reverse_bits() {
    for b in 0u8..=255 {
        assert_eq!(bitrev8(b), b.reverse_bits());
    }
    for x in [0u32, 1, 0x8000_0000, 0x1234_5678, 0xdead_beef, u32::MAX] {
        assert_eq!(bitrev32(x), x.reverse_bits());
        assert_eq!(bitrev16(x as u16), (x as u16).reverse_bits());
        assert_eq!(
            bitrev8x4(x),
            u32::from_be_bytes(x.to_be_bytes().map(u8::reverse_bits))
        );
    }
}

#[test]
fn hweight_matches_count_ones() {
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..10_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        assert_eq!(sw_hweight64(x), x.count_ones() as u64);
        assert_eq!(sw_hweight32(x as u32), (x as u32).count_ones());
        assert_eq!(sw_hweight16(x as u16 as u32), (x as u16).count_ones());
        assert_eq!(sw_hweight8(x as u8 as u32), (x as u8).count_ones());
    }
    assert_eq!(sw_hweight64(u64::MAX), 64);
    assert_eq!(sw_hweight32(u32::MAX), 32);
}

#[test]
fn errno_values() {
    assert_eq!(errno::EPERM.get(), 1);
    assert_eq!(errno::EINVAL.get(), 22);
    assert_eq!(errno::EINVAL.to_errno(), -22);
    assert_eq!(errno::ENOSYS.get(), 38);
    assert_eq!(errno::EHWPOISON.get(), 133);
    assert_eq!(errno::ERESTARTSYS.get(), 512);
    assert_eq!(errno::EWOULDBLOCK, errno::EAGAIN);
    assert_eq!(errno::EDEADLOCK, errno::EDEADLK);
    assert_eq!(format!("{:?}", errno::ENOENT), "ENOENT");
    assert_eq!(errno::ENOENT.to_string(), "No such file or directory");
    assert_eq!(Errno::from_errno(-2), Some(errno::ENOENT));
    assert_eq!(Errno::from_errno(0), None);
    assert_eq!(Errno::new(4096), None);
    assert_eq!(format!("{:?}", Errno::new(4000).unwrap()), "Errno(4000)");
    assert!(errno::is_err_value((-1i64) as u64));
    assert!(errno::is_err_value((-4095i64) as u64));
    assert!(!errno::is_err_value((-4096i64) as u64));
    assert!(!errno::is_err_value(0));
}
