// SPDX-License-Identifier: GPL-2.0-only
//! CRC library functions.
//!
//! Ported from `lib/crc/`. Only the generic table-driven code is ported;
//! the per-architecture accelerated versions (`lib/crc/x86/` and friends)
//! will come with the arch code.
//!
//! The tables for CRC32 and CRC64 are computed at compile time, ported from
//! the build-time generators `gen_crc32table.c` and `gen_crc64table.c`. The
//! other tables are copied from the C sources by `scripts/gen_tables.py`.

mod tables;

pub use tables::{
    CRC_CCITT_TABLE, CRC_ITU_T_TABLE, CRC4_TAB, CRC7_BE_SYNDROME_TABLE, CRC16_TABLE,
    T10_DIF_CRC_TABLE,
};

use crate::bitrev::bitrev32;

/// Bit-reversed CRC-32 (Ethernet) polynomial, `crc32poly.h`.
pub const CRC32_POLY_LE: u32 = 0xedb8_8320;
/// CRC-32 (Ethernet) polynomial, `crc32poly.h`.
pub const CRC32_POLY_BE: u32 = 0x04c1_1db7;
/// Bit-reversed CRC-32C (Castagnoli) polynomial, `crc32poly.h`.
pub const CRC32C_POLY_LE: u32 = 0x82f6_3b78;
/// CRC-64 polynomial from ECMA-182.
pub const CRC64_ECMA182_POLY: u64 = 0x42f0_e1eb_a9ea_3693;
/// Bit-reversed CRC-64 polynomial from the NVMe NVM Command Set.
pub const CRC64_NVME_POLY: u64 = 0x9a6c_9329_ac4b_c9b5;

/// `crc32init_le_generic()` from `gen_crc32table.c`.
///
/// `crc` is the CRC of the byte `i`; other entries follow from
/// `table[i ^ j] == table[i] ^ table[j]`.
const fn crc32init_le_generic(polynomial: u32) -> [u32; 256] {
    let mut tab = [0u32; 256];
    let mut crc: u32 = 1;
    let mut i = 128;
    while i != 0 {
        crc = (crc >> 1) ^ if crc & 1 != 0 { polynomial } else { 0 };
        let mut j = 0;
        while j < 256 {
            tab[i + j] = crc ^ tab[j];
            j += 2 * i;
        }
        i >>= 1;
    }
    tab
}

/// `crc32init_be()` from `gen_crc32table.c`.
const fn crc32init_be() -> [u32; 256] {
    let mut tab = [0u32; 256];
    let mut crc: u32 = 0x8000_0000;
    let mut i = 1;
    while i < 256 {
        crc = (crc << 1)
            ^ if crc & 0x8000_0000 != 0 {
                CRC32_POLY_BE
            } else {
                0
            };
        let mut j = 0;
        while j < i {
            tab[i + j] = crc ^ tab[j];
            j += 1;
        }
        i <<= 1;
    }
    tab
}

/// `generate_crc64_table()` from `gen_crc64table.c`.
const fn generate_crc64_table(poly: u64) -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc: u64 = 0;
        let mut c = (i as u64) << 56;
        let mut j = 0;
        while j < 8 {
            if (crc ^ c) & 0x8000_0000_0000_0000 != 0 {
                crc = (crc << 1) ^ poly;
            } else {
                crc <<= 1;
            }
            c <<= 1;
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// `generate_reflected_crc64_table()` from `gen_crc64table.c`.
const fn generate_reflected_crc64_table(poly: u64) -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc: u64 = 0;
        let c = i as u64;
        let mut j = 0;
        while j < 8 {
            if (crc ^ (c >> j)) & 1 != 0 {
                crc = (crc >> 1) ^ poly;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// `crc32table_le[]`.
pub static CRC32TABLE_LE: [u32; 256] = crc32init_le_generic(CRC32_POLY_LE);
/// `crc32table_be[]`.
pub static CRC32TABLE_BE: [u32; 256] = crc32init_be();
/// `crc32ctable_le[]`.
pub static CRC32CTABLE_LE: [u32; 256] = crc32init_le_generic(CRC32C_POLY_LE);
/// `crc64table[]`.
pub static CRC64TABLE: [u64; 256] = generate_crc64_table(CRC64_ECMA182_POLY);
/// `crc64nvmetable[]`.
pub static CRC64NVMETABLE: [u64; 256] = generate_reflected_crc64_table(CRC64_NVME_POLY);

/// `crc32_le()`: little-endian (bit-reversed) CRC-32, as used by Ethernet.
///
/// The seed is passed in and the result is not inverted; callers add the
/// `~0` pre- and post-conditioning they need.
pub fn crc32_le(mut crc: u32, p: &[u8]) -> u32 {
    for &b in p {
        crc = (crc >> 8) ^ CRC32TABLE_LE[((crc & 255) as u8 ^ b) as usize];
    }
    crc
}

/// `crc32()`: alias of [`crc32_le`].
#[inline]
pub fn crc32(crc: u32, p: &[u8]) -> u32 {
    crc32_le(crc, p)
}

/// `crc32_be()`: big-endian CRC-32.
pub fn crc32_be(mut crc: u32, p: &[u8]) -> u32 {
    for &b in p {
        crc = (crc << 8) ^ CRC32TABLE_BE[((crc >> 24) as u8 ^ b) as usize];
    }
    crc
}

/// `crc32c()`: CRC-32C (Castagnoli), as used by iSCSI, ext4 and btrfs.
pub fn crc32c(mut crc: u32, p: &[u8]) -> u32 {
    for &b in p {
        crc = (crc >> 8) ^ CRC32CTABLE_LE[((crc & 255) as u8 ^ b) as usize];
    }
    crc
}

/// `crc32_optimizations()`: which CRC-32 variants are accelerated. None yet.
#[inline]
pub fn crc32_optimizations() -> u32 {
    0
}

/// `ether_crc()`: the Ethernet FCS in big-endian bit order.
#[inline]
pub fn ether_crc(data: &[u8]) -> u32 {
    bitrev32(crc32_le(!0, data))
}

/// `ether_crc_le()`: the Ethernet FCS in little-endian bit order.
#[inline]
pub fn ether_crc_le(data: &[u8]) -> u32 {
    crc32_le(!0, data)
}

/// `crc64_be()`: CRC-64 with the ECMA-182 polynomial, no inversion.
pub fn crc64_be(mut crc: u64, p: &[u8]) -> u64 {
    for &b in p {
        crc = (crc << 8) ^ CRC64TABLE[((crc >> 56) as u8 ^ b) as usize];
    }
    crc
}

/// `crc64_nvme()`: CRC-64 as defined by NVMe, with `~0` pre- and
/// post-inversion.
pub fn crc64_nvme(crc: u64, p: &[u8]) -> u64 {
    let mut crc = !crc;
    for &b in p {
        crc = (crc >> 8) ^ CRC64NVMETABLE[((crc & 0xff) as u8 ^ b) as usize];
    }
    !crc
}

/// `crc16()`: CRC-16 with polynomial `x^16 + x^15 + x^2 + 1`.
pub fn crc16(mut crc: u16, p: &[u8]) -> u16 {
    for &b in p {
        crc = (crc >> 8) ^ CRC16_TABLE[((crc & 0xff) as u8 ^ b) as usize];
    }
    crc
}

/// `crc_ccitt_byte()`: one byte of CRC-CCITT.
#[inline]
pub fn crc_ccitt_byte(crc: u16, c: u8) -> u16 {
    (crc >> 8) ^ CRC_CCITT_TABLE[((crc ^ c as u16) & 0xff) as usize]
}

/// `crc_ccitt()`: CRC-CCITT (bit-reversed `x^16 + x^12 + x^5 + 1`).
pub fn crc_ccitt(mut crc: u16, buffer: &[u8]) -> u16 {
    for &b in buffer {
        crc = crc_ccitt_byte(crc, b);
    }
    crc
}

/// `crc_itu_t_byte()`: one byte of CRC ITU-T V.41.
#[inline]
pub fn crc_itu_t_byte(crc: u16, data: u8) -> u16 {
    (crc << 8) ^ CRC_ITU_T_TABLE[(((crc >> 8) ^ data as u16) & 0xff) as usize]
}

/// `crc_itu_t()`: CRC ITU-T V.41 (`x^16 + x^12 + x^5 + 1`, MSB first).
pub fn crc_itu_t(mut crc: u16, buffer: &[u8]) -> u16 {
    for &b in buffer {
        crc = crc_itu_t_byte(crc, b);
    }
    crc
}

/// `crc_t10dif_update()`: T10 Data Integrity Field CRC-16.
pub fn crc_t10dif_update(mut crc: u16, p: &[u8]) -> u16 {
    for &b in p {
        crc = (crc << 8) ^ T10_DIF_CRC_TABLE[((crc >> 8) as u8 ^ b) as usize];
    }
    crc
}

/// `crc_t10dif()`: [`crc_t10dif_update`] starting from 0.
#[inline]
pub fn crc_t10dif(p: &[u8]) -> u16 {
    crc_t10dif_update(0, p)
}

/// `crc7_be()`: CRC-7 (`x^7 + x^3 + 1`), as used by SD/MMC.
///
/// The CRC is left-aligned in the byte; the low bit is always 0.
pub fn crc7_be(mut crc: u8, buffer: &[u8]) -> u8 {
    for &b in buffer {
        crc = CRC7_BE_SYNDROME_TABLE[(crc ^ b) as usize];
    }
    crc
}

/// `crc4()`: 4-bit CRC (polynomial 0b10111) of the low `bits` bits of `x`,
/// processed from the most significant nibble.
pub fn crc4(mut c: u8, mut x: u64, bits: u32) -> u8 {
    // Mask off anything above the top bit. (C shifts by `bits`, which is
    // undefined for 64; this keeps all 64 bits.)
    x &= 1u64.checked_shl(bits).map_or(u64::MAX, |v| v - 1);

    // Align to four bits.
    let bits = (bits + 3) & !3;

    let mut i = bits as i32 - 4;
    while i >= 0 {
        c = CRC4_TAB[(c ^ ((x >> i) & 0xf) as u8) as usize];
        i -= 4;
    }
    c
}

/// Size of a [`crc8`] lookup table.
pub const CRC8_TABLE_SIZE: usize = 256;
/// Conventional initial value for [`crc8`].
pub const CRC8_INIT_VALUE: u8 = 0xff;

/// `CRC8_GOOD_VALUE()`: the CRC of a message followed by its correct CRC.
#[inline]
pub fn crc8_good_value(table: &[u8; CRC8_TABLE_SIZE]) -> u8 {
    table[0xff]
}

/// `crc8_populate_msb()`: builds a table for `polynomial`, MSB first.
pub const fn crc8_populate_msb(polynomial: u8) -> [u8; CRC8_TABLE_SIZE] {
    let mut table = [0u8; CRC8_TABLE_SIZE];
    const MSBIT: u8 = 0x80;
    let mut t = MSBIT;
    let mut i = 1;
    while i < CRC8_TABLE_SIZE {
        t = (t << 1) ^ if t & MSBIT != 0 { polynomial } else { 0 };
        let mut j = 0;
        while j < i {
            table[i + j] = table[j] ^ t;
            j += 1;
        }
        i *= 2;
    }
    table
}

/// `crc8_populate_lsb()`: builds a table for `polynomial`, LSB first.
pub const fn crc8_populate_lsb(polynomial: u8) -> [u8; CRC8_TABLE_SIZE] {
    let mut table = [0u8; CRC8_TABLE_SIZE];
    let mut t: u8 = 1;
    let mut i = CRC8_TABLE_SIZE >> 1;
    while i != 0 {
        t = (t >> 1) ^ if t & 1 != 0 { polynomial } else { 0 };
        let mut j = 0;
        while j < CRC8_TABLE_SIZE {
            table[i + j] = table[j] ^ t;
            j += 2 * i;
        }
        i >>= 1;
    }
    table
}

/// `crc8()`: CRC-8 of `pdata` using a table from `crc8_populate_*()`.
pub fn crc8(table: &[u8; CRC8_TABLE_SIZE], pdata: &[u8], mut crc: u8) -> u8 {
    for &b in pdata {
        crc = table[(crc ^ b) as usize];
    }
    crc
}
