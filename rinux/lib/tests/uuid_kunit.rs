// SPDX-License-Identifier: BSD-3-Clause OR GPL-2.0
//! Port of `lib/tests/uuid_kunit.c`.
//!
//! The generators take their random bytes from a closure until the RNG is
//! ported; these tests feed them from a seeded `RndState`.

use rinux_lib::errno::EINVAL;
use rinux_lib::random32::RndState;
use rinux_lib::uuid::*;

struct TestUuidData {
    uuid: &'static str,
    le: Guid,
    be: Uuid,
}

const TEST_UUID_TEST_DATA: [TestUuidData; 3] = [
    TestUuidData {
        uuid: "c33f4995-3701-450e-9fbf-206a2e98e576",
        le: guid_init(
            0xc33f4995, 0x3701, 0x450e, 0x9f, 0xbf, 0x20, 0x6a, 0x2e, 0x98, 0xe5, 0x76,
        ),
        be: uuid_init(
            0xc33f4995, 0x3701, 0x450e, 0x9f, 0xbf, 0x20, 0x6a, 0x2e, 0x98, 0xe5, 0x76,
        ),
    },
    TestUuidData {
        uuid: "64b4371c-77c1-48f9-8221-29f054fc023b",
        le: guid_init(
            0x64b4371c, 0x77c1, 0x48f9, 0x82, 0x21, 0x29, 0xf0, 0x54, 0xfc, 0x02, 0x3b,
        ),
        be: uuid_init(
            0x64b4371c, 0x77c1, 0x48f9, 0x82, 0x21, 0x29, 0xf0, 0x54, 0xfc, 0x02, 0x3b,
        ),
    },
    TestUuidData {
        uuid: "0cb4ddff-a545-4401-9d06-688af53e7f84",
        le: guid_init(
            0x0cb4ddff, 0xa545, 0x4401, 0x9d, 0x06, 0x68, 0x8a, 0xf5, 0x3e, 0x7f, 0x84,
        ),
        be: uuid_init(
            0x0cb4ddff, 0xa545, 0x4401, 0x9d, 0x06, 0x68, 0x8a, 0xf5, 0x3e, 0x7f, 0x84,
        ),
    },
];

const TEST_UUID_WRONG_DATA: [&str; 3] = [
    "c33f4995-3701-450e-9fbf206a2e98e576 ", // no hyphen(s)
    "64b4371c-77c1-48f9-8221-29f054XX023b", // invalid character(s)
    "0cb4ddff-a545-4401-9d06-688af53e",     // not enough data
];

#[test]
fn uuid_test_guid_valid() {
    for data in &TEST_UUID_TEST_DATA {
        let le = guid_parse(data.uuid).unwrap();
        assert!(guid_equal(&data.le, &le));
    }
}

#[test]
fn uuid_test_uuid_valid() {
    for data in &TEST_UUID_TEST_DATA {
        let be = uuid_parse(data.uuid).unwrap();
        assert!(uuid_equal(&data.be, &be));
    }
}

#[test]
fn uuid_test_guid_invalid() {
    for uuid in TEST_UUID_WRONG_DATA {
        assert_eq!(guid_parse(uuid), Err(EINVAL));
    }
}

#[test]
fn uuid_test_uuid_invalid() {
    for uuid in TEST_UUID_WRONG_DATA {
        assert_eq!(uuid_parse(uuid), Err(EINVAL));
    }
}

/// RFC 4122 section 4.4 says random UUIDs/GUIDs (version 4) must have:
///   - version 4 in the high nibble of the version byte,
///   - variant DCE 1.1 (binary 10x) in the high bits of byte 8.
///
/// The version byte is byte 6 in the "wire" uuid_t layout and byte 7 in the
/// byte-swapped guid_t layout.
#[test]
fn uuid_test_uuid_gen() {
    let mut rnd = RndState::new(1);
    let mut u = Uuid::default();

    for _ in 0..8 {
        uuid_gen(&mut u, |b| rnd.fill_bytes(b));
        assert_eq!(u.b[6] & 0xf0, 0x40);
        assert_eq!(u.b[8] & 0xc0, 0x80);
    }
}

#[test]
fn uuid_test_guid_gen() {
    let mut rnd = RndState::new(2);
    let mut g = Guid::default();

    for _ in 0..8 {
        guid_gen(&mut g, |b| rnd.fill_bytes(b));
        assert_eq!(g.b[7] & 0xf0, 0x40);
        assert_eq!(g.b[8] & 0xc0, 0x80);
    }
}

#[test]
fn uuid_test_generate_random_uuid() {
    let mut rnd = RndState::new(3);
    let mut buf = [0u8; 16];

    for _ in 0..8 {
        generate_random_uuid(&mut buf, |b| rnd.fill_bytes(b));
        assert_eq!(buf[6] & 0xf0, 0x40);
        assert_eq!(buf[8] & 0xc0, 0x80);
    }
}

#[test]
fn uuid_test_generate_random_guid() {
    let mut rnd = RndState::new(4);
    let mut buf = [0u8; 16];

    for _ in 0..8 {
        generate_random_guid(&mut buf, |b| rnd.fill_bytes(b));
        assert_eq!(buf[7] & 0xf0, 0x40);
        assert_eq!(buf[8] & 0xc0, 0x80);
    }
}
