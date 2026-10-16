// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the OID registry. Linux has no KUnit file for it, so these
//! check that every generated table entry round-trips and the printer's edges.

use rinux_lib::errno::{EBADMSG, ENOBUFS};
use rinux_lib::oid_registry::{Oid, look_up_oid, parse_oid, sprint_oid};
use rinux_lib::oid_registry_data::{OID_DATA, OID_DOTTED, OID_INDEX, OID_NR, OID_VARIANTS};

fn content(i: usize) -> &'static [u8] {
    &OID_DATA[usize::from(OID_INDEX[i])..usize::from(OID_INDEX[i + 1])]
}

#[test]
fn every_registered_oid_is_found() {
    for i in 0..OID_NR {
        assert_eq!(
            look_up_oid(content(i)),
            OID_VARIANTS[i],
            "{}",
            OID_DOTTED[i]
        );
    }
}

#[test]
fn every_registered_oid_prints_its_dotted_form() {
    let mut buf = [0u8; 64];
    for i in 0..OID_NR {
        let n = sprint_oid(content(i), &mut buf).unwrap();
        assert_eq!(&buf[..n], OID_DOTTED[i].as_bytes());
        assert_eq!(buf[n], 0);
    }
}

#[test]
fn rsa_encryption_by_hand() {
    // 1.2.840.113549.1.1.1
    let data = [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
    assert_eq!(look_up_oid(&data), Oid::OID_rsaEncryption);
    let mut buf = [0u8; 32];
    assert_eq!(sprint_oid(&data, &mut buf), Ok(20));
    assert_eq!(&buf[..21], b"1.2.840.113549.1.1.1\0");
}

#[test]
fn unknown_and_empty_oids_are_not_held_here() {
    assert_eq!(look_up_oid(&[]), Oid::OID__NR);
    assert_eq!(look_up_oid(&[0x2a, 0x86, 0x48]), Oid::OID__NR);
    assert_eq!(
        look_up_oid(&[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x02]),
        Oid::OID__NR
    );
}

#[test]
fn print_truncation_is_enobufs() {
    let data = [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
    let mut buf = [0u8; 5];
    assert_eq!(sprint_oid(&data, &mut buf), Err(ENOBUFS));
    let mut empty: [u8; 0] = [];
    assert_eq!(sprint_oid(&data, &mut empty), Err(ENOBUFS));
}

#[test]
fn malformed_content_is_ebadmsg_and_writes_bad() {
    let mut buf = [0u8; 16];
    // A multi-byte arc whose continuation runs off the end.
    assert_eq!(sprint_oid(&[0x2a, 0x86], &mut buf), Err(EBADMSG));
    // C has already printed "1.2" when it finds the bad octet.
    assert_eq!(&buf[..9], b"1.2(bad)\0");
    assert_eq!(sprint_oid(&[], &mut buf), Err(EBADMSG));
}

#[test]
fn parse_checks_the_tlv_header() {
    assert_eq!(parse_oid(&[6, 3, 0x2a, 0x03, 0x04]), Ok(()));
    assert_eq!(parse_oid(&[6, 4, 0x2a, 0x03, 0x04]), Err(EBADMSG));
    assert_eq!(parse_oid(&[5, 3, 0x2a, 0x03, 0x04]), Err(EBADMSG));
    assert_eq!(parse_oid(&[6, 0]), Err(EBADMSG));
}
