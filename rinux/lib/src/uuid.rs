// SPDX-License-Identifier: GPL-2.0-only
//! UUIDs and GUIDs.
//!
//! Ported from `lib/uuid.c` and `include/linux/uuid.h`.
//!
//! Copyright (C) 2009, 2010, 2016 Intel Corp.
//! Huang Ying <ying.huang@intel.com>
//!
//! A [`Uuid`] stores its fields big-endian ("wire" order); a [`Guid`] stores
//! the first three fields little-endian, as EFI and Microsoft do. Both print
//! as `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`.
//!
//! The generators need `get_random_bytes()`. Until the random number
//! generator is ported they take the random source as a closure that fills
//! a buffer; the kernel will pass `get_random_bytes`.

use crate::ctype::isxdigit;
use crate::errno::{EINVAL, Result};
use crate::hexdump::hex_to_bin;

/// `UUID_SIZE`.
pub const UUID_SIZE: usize = 16;

/// `UUID_STRING_LEN`: length of `"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"`.
pub const UUID_STRING_LEN: usize = 36;

/// `guid_t`: a GUID, first three fields little-endian.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Guid {
    pub b: [u8; UUID_SIZE],
}

/// `uuid_t`: a UUID, all fields big-endian.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Uuid {
    pub b: [u8; UUID_SIZE],
}

/// `GUID_INIT()`.
#[allow(clippy::too_many_arguments)]
pub const fn guid_init(
    a: u32,
    b: u16,
    c: u16,
    d0: u8,
    d1: u8,
    d2: u8,
    d3: u8,
    d4: u8,
    d5: u8,
    d6: u8,
    d7: u8,
) -> Guid {
    let a = a.to_le_bytes();
    let b = b.to_le_bytes();
    let c = c.to_le_bytes();
    Guid {
        b: [
            a[0], a[1], a[2], a[3], b[0], b[1], c[0], c[1], d0, d1, d2, d3, d4, d5, d6, d7,
        ],
    }
}

/// `UUID_INIT()`.
#[allow(clippy::too_many_arguments)]
pub const fn uuid_init(
    a: u32,
    b: u16,
    c: u16,
    d0: u8,
    d1: u8,
    d2: u8,
    d3: u8,
    d4: u8,
    d5: u8,
    d6: u8,
    d7: u8,
) -> Uuid {
    let a = a.to_be_bytes();
    let b = b.to_be_bytes();
    let c = c.to_be_bytes();
    Uuid {
        b: [
            a[0], a[1], a[2], a[3], b[0], b[1], c[0], c[1], d0, d1, d2, d3, d4, d5, d6, d7,
        ],
    }
}

/// `guid_null`.
pub const GUID_NULL: Guid = Guid { b: [0; UUID_SIZE] };
/// `uuid_null`.
pub const UUID_NULL: Uuid = Uuid { b: [0; UUID_SIZE] };

/// `guid_index[]`: byte `i` of the printed form is byte `GUID_INDEX[i]` of a
/// [`Guid`].
pub const GUID_INDEX: [u8; 16] = [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15];
/// `uuid_index[]`: the identity, for [`Uuid`].
pub const UUID_INDEX: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

/// `guid_equal()`.
pub fn guid_equal(u1: &Guid, u2: &Guid) -> bool {
    u1 == u2
}

/// `guid_copy()`.
pub fn guid_copy(dst: &mut Guid, src: &Guid) {
    *dst = *src;
}

/// `import_guid()`.
pub fn import_guid(dst: &mut Guid, src: &[u8; UUID_SIZE]) {
    dst.b = *src;
}

/// `export_guid()`.
pub fn export_guid(dst: &mut [u8; UUID_SIZE], src: &Guid) {
    *dst = src.b;
}

/// `guid_is_null()`.
pub fn guid_is_null(guid: &Guid) -> bool {
    *guid == GUID_NULL
}

/// `uuid_equal()`.
pub fn uuid_equal(u1: &Uuid, u2: &Uuid) -> bool {
    u1 == u2
}

/// `uuid_copy()`.
pub fn uuid_copy(dst: &mut Uuid, src: &Uuid) {
    *dst = *src;
}

/// `import_uuid()`.
pub fn import_uuid(dst: &mut Uuid, src: &[u8; UUID_SIZE]) {
    dst.b = *src;
}

/// `export_uuid()`.
pub fn export_uuid(dst: &mut [u8; UUID_SIZE], src: &Uuid) {
    *dst = src.b;
}

/// `uuid_is_null()`.
pub fn uuid_is_null(uuid: &Uuid) -> bool {
    *uuid == UUID_NULL
}

/// `generate_random_uuid()`: a random (version 4, DCE variant) UUID.
pub fn generate_random_uuid(uuid: &mut [u8; 16], get_random_bytes: impl FnOnce(&mut [u8])) {
    get_random_bytes(uuid);
    // Set UUID version to 4 --- truly random generation.
    uuid[6] = (uuid[6] & 0x0F) | 0x40;
    // Set the UUID variant to DCE.
    uuid[8] = (uuid[8] & 0x3F) | 0x80;
}

/// `generate_random_guid()`: a random (version 4, DCE variant) GUID.
pub fn generate_random_guid(guid: &mut [u8; 16], get_random_bytes: impl FnOnce(&mut [u8])) {
    get_random_bytes(guid);
    // Set GUID version to 4 --- truly random generation.
    guid[7] = (guid[7] & 0x0F) | 0x40;
    // Set the GUID variant to DCE.
    guid[8] = (guid[8] & 0x3F) | 0x80;
}

fn __uuid_gen_common(b: &mut [u8; 16], get_random_bytes: impl FnOnce(&mut [u8])) {
    get_random_bytes(b);
    // Revision 0b10.
    b[8] = (b[8] & 0x3F) | 0x80;
}

/// `guid_gen()`.
pub fn guid_gen(lu: &mut Guid, get_random_bytes: impl FnOnce(&mut [u8])) {
    __uuid_gen_common(&mut lu.b, get_random_bytes);
    // Version 4: random generation.
    lu.b[7] = (lu.b[7] & 0x0F) | 0x40;
}

/// `uuid_gen()`.
pub fn uuid_gen(bu: &mut Uuid, get_random_bytes: impl FnOnce(&mut [u8])) {
    __uuid_gen_common(&mut bu.b, get_random_bytes);
    // Version 4: random generation.
    bu.b[6] = (bu.b[6] & 0x0F) | 0x40;
}

/// `uuid_is_valid()`: does `uuid` start with
/// `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`, `x` being hex digits?
pub fn uuid_is_valid(uuid: &[u8]) -> bool {
    (0..UUID_STRING_LEN).all(|i| {
        let c = uuid.get(i).copied().unwrap_or(0);
        if i == 8 || i == 13 || i == 18 || i == 23 {
            c == b'-'
        } else {
            isxdigit(c)
        }
    })
}

fn __uuid_parse(uuid: &[u8], ei: &[u8; 16]) -> Result<[u8; 16]> {
    const SI: [u8; 16] = [0, 2, 4, 6, 9, 11, 14, 16, 19, 21, 24, 26, 28, 30, 32, 34];

    if !uuid_is_valid(uuid) {
        return Err(EINVAL);
    }
    let mut b = [0; 16];
    for i in 0..16 {
        let hi = hex_to_bin(uuid[SI[i] as usize]);
        let lo = hex_to_bin(uuid[SI[i] as usize + 1]);
        b[ei[i] as usize] = ((hi << 4) | lo) as u8;
    }
    Ok(b)
}

/// `guid_parse()`: parses a GUID string.
pub fn guid_parse(uuid: impl AsRef<[u8]>) -> Result<Guid> {
    Ok(Guid {
        b: __uuid_parse(uuid.as_ref(), &GUID_INDEX)?,
    })
}

/// `uuid_parse()`: parses a UUID string.
pub fn uuid_parse(uuid: impl AsRef<[u8]>) -> Result<Uuid> {
    Ok(Uuid {
        b: __uuid_parse(uuid.as_ref(), &UUID_INDEX)?,
    })
}
