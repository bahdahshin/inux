// SPDX-License-Identifier: GPL-2.0-or-later
//! Port of `lib/crc/tests/crc_kunit.c`.
//!
//! Copyright 2024 Google LLC
//! Author: Eric Biggers <ebiggers@google.com>
//!
//! Each CRC is checked against a bit-at-a-time reference implementation on
//! random data, lengths, offsets and initial values, using the same seeded
//! generator as the C test. The interrupt-context and benchmark cases are
//! not ported; they need a running kernel.

use rinux_lib::crc::*;
use rinux_lib::random32::RndState;

const CRC_KUNIT_SEED: u64 = 42;
const CRC_KUNIT_MAX_LEN: usize = 16384;
const CRC_KUNIT_NUM_TEST_ITERS: usize = 1000;
const PAGE_SIZE: usize = 4096;

/// `struct crc_variant`.
struct CrcVariant {
    /// Number of bits in the CRC.
    bits: u32,
    /// A "little endian" CRC: reversed mapping between bits and polynomial
    /// coefficients in each byte.
    le: bool,
    /// The generator polynomial without its highest-order term,
    /// bit-reversed if `le`.
    poly: u64,
    /// Computes the CRC without pre- or post-inversion.
    func: fn(u64, &[u8]) -> u64,
}

impl CrcVariant {
    fn mask(&self) -> u64 {
        u64::MAX >> (64 - self.bits)
    }

    /// `crc_ref()`: reference implementation of any CRC variant.
    fn reference(&self, mut crc: u64, p: &[u8]) -> u64 {
        for &byte in p {
            for j in 0..8 {
                if self.le {
                    crc ^= ((byte >> j) & 1) as u64;
                    crc = (crc >> 1) ^ if crc & 1 != 0 { self.poly } else { 0 };
                } else {
                    crc ^= (((byte >> (7 - j)) & 1) as u64) << (self.bits - 1);
                    if crc & (1u64 << (self.bits - 1)) != 0 {
                        crc = ((crc << 1) ^ self.poly) & self.mask();
                    } else {
                        crc <<= 1;
                    }
                }
            }
        }
        crc
    }
}

struct Suite {
    rng: RndState,
    buffer: Vec<u8>,
}

impl Suite {
    /// `crc_suite_init()`. The C test puts a guard page after a vmalloc
    /// buffer; slices are bounds-checked, so a plain buffer will do.
    fn new() -> Suite {
        let buflen = CRC_KUNIT_MAX_LEN.div_ceil(PAGE_SIZE) * PAGE_SIZE;
        let mut rng = RndState::new(CRC_KUNIT_SEED);
        let mut buffer = vec![0u8; buflen];
        rng.fill_bytes(&mut buffer);
        Suite { rng, buffer }
    }

    fn rand32(&mut self) -> u32 {
        self.rng.next_u32()
    }

    fn rand64(&mut self) -> u64 {
        let n = self.rand32() as u64;
        (n << 32) | self.rand32() as u64
    }

    /// `generate_random_initial_crc()`.
    fn random_initial_crc(&mut self, v: &CrcVariant) -> u64 {
        match self.rand32() % 4 {
            0 => 0,
            1 => v.mask(),
            _ => self.rand64() & v.mask(),
        }
    }

    /// `generate_random_length()`: prefers small lengths.
    fn random_length(&mut self, max_length: usize) -> usize {
        let len = match self.rand32() % 3 {
            0 => (self.rand32() % 128) as usize,
            1 => (self.rand32() % 3072) as usize,
            _ => self.rand32() as usize,
        };
        len % (max_length + 1)
    }

    /// `crc_test()`: `v.func` must agree with the reference implementation.
    fn crc_test(&mut self, v: &CrcVariant) {
        for _ in 0..CRC_KUNIT_NUM_TEST_ITERS {
            let init_crc = self.random_initial_crc(v);
            let len = self.random_length(CRC_KUNIT_MAX_LEN);

            let offset = if self.rand32() % 2 == 0 {
                // A random alignment mod 64.
                ((self.rand32() % 64) as usize).min(CRC_KUNIT_MAX_LEN - len)
            } else {
                // Right up to the end of the buffer.
                self.buffer.len() - len
            };

            if self.rand32() % 8 == 0 {
                // Refresh the data occasionally.
                let Suite { rng, buffer } = self;
                rng.fill_bytes(&mut buffer[offset..offset + len]);
            }

            let data = &self.buffer[offset..offset + len];
            let expected = v.reference(init_crc, data);
            let actual = (v.func)(init_crc, data);
            assert_eq!(
                expected, actual,
                "wrong result with len={len} offset={offset}"
            );
        }
    }
}

fn run(v: &CrcVariant) {
    Suite::new().crc_test(v);
}

#[test]
fn crc7_be_test() {
    run(&CrcVariant {
        bits: 7,
        le: false,
        poly: 0x9,
        // crc7_be() left-aligns the CRC in a byte; the test wants it
        // right-aligned.
        func: |crc, p| (crc7_be((crc << 1) as u8, p) >> 1) as u64,
    });
}

#[test]
fn crc16_test() {
    run(&CrcVariant {
        bits: 16,
        le: true,
        poly: 0xa001,
        func: |crc, p| crc16(crc as u16, p) as u64,
    });
}

#[test]
fn crc_t10dif_test() {
    run(&CrcVariant {
        bits: 16,
        le: false,
        poly: 0x8bb7,
        func: |crc, p| crc_t10dif_update(crc as u16, p) as u64,
    });
}

#[test]
fn crc32_le_test() {
    run(&CrcVariant {
        bits: 32,
        le: true,
        poly: 0xedb8_8320,
        func: |crc, p| crc32_le(crc as u32, p) as u64,
    });
}

#[test]
fn crc32_be_test() {
    run(&CrcVariant {
        bits: 32,
        le: false,
        poly: 0x04c1_1db7,
        func: |crc, p| crc32_be(crc as u32, p) as u64,
    });
}

#[test]
fn crc32c_test() {
    run(&CrcVariant {
        bits: 32,
        le: true,
        poly: 0x82f6_3b78,
        func: |crc, p| crc32c(crc as u32, p) as u64,
    });
}

#[test]
fn crc64_be_test() {
    run(&CrcVariant {
        bits: 64,
        le: false,
        poly: 0x42f0_e1eb_a9ea_3693,
        func: crc64_be,
    });
}

#[test]
fn crc64_nvme_test() {
    run(&CrcVariant {
        bits: 64,
        le: true,
        poly: 0x9a6c_9329_ac4b_c9b5,
        // Undo the inversions crc64_nvme() does.
        func: |crc, p| !crc64_nvme(!crc, p),
    });
}

// Not in the C test: the CRCs it does not cover, and well-known check
// values (the CRC of "123456789") from the CRC catalogue.

#[test]
fn crc_ccitt_and_itu_t() {
    let ccitt = CrcVariant {
        bits: 16,
        le: true,
        poly: 0x8408,
        func: |crc, p| crc_ccitt(crc as u16, p) as u64,
    };
    run(&ccitt);
    let itu_t = CrcVariant {
        bits: 16,
        le: false,
        poly: 0x1021,
        func: |crc, p| crc_itu_t(crc as u16, p) as u64,
    };
    run(&itu_t);
}

#[test]
fn check_values() {
    let check = b"123456789";
    assert_eq!(!crc32_le(!0, check), 0xcbf4_3926); // CRC-32/ISO-HDLC
    assert_eq!(!crc32c(!0, check), 0xe306_9283); // CRC-32/ISCSI
    assert_eq!(!crc32_be(!0, check), 0xfc89_1918); // CRC-32/BZIP2
    assert_eq!(crc16(0, check), 0xbb3d); // CRC-16/ARC
    assert_eq!(crc_t10dif(check), 0xd0db); // CRC-16/T10-DIF
    assert_eq!(crc_itu_t(0, check), 0x31c3); // CRC-16/XMODEM
    assert_eq!(crc_ccitt(0, check), 0x2189); // CRC-16/KERMIT
    assert_eq!(crc7_be(0, check) >> 1, 0x75); // CRC-7/MMC
    assert_eq!(crc64_be(0, check), 0x6c40_df5f_0b49_7347); // CRC-64/ECMA-182
    assert_eq!(crc64_nvme(0, check), 0xae8b_1486_0a79_9888); // CRC-64/NVME
    assert_eq!(ether_crc_le(check), crc32_le(!0, check));
}

#[test]
fn crc8_tables() {
    // CRC-8/SMBUS: polynomial 0x07, MSB first, init 0. Check value 0xf4.
    let msb = crc8_populate_msb(0x07);
    assert_eq!(crc8(&msb, b"123456789", 0), 0xf4);
    // CRC-8/MAXIM-DOW: polynomial 0x31 reflected to 0x8c, LSB first, init 0.
    let lsb = crc8_populate_lsb(0x8c);
    assert_eq!(crc8(&lsb, b"123456789", 0), 0xa1);
    // A message followed by its CRC leaves the table's good value.
    let crc = crc8(&msb, b"hello", CRC8_INIT_VALUE);
    let mut with_crc = b"hello".to_vec();
    with_crc.push(crc);
    assert_eq!(crc8(&msb, &with_crc, CRC8_INIT_VALUE), 0);
    assert_eq!(crc8_good_value(&msb), msb[0xff]);
}

#[test]
fn crc4_values() {
    // A bit-at-a-time CRC-4 with polynomial x^4 + x^2 + x + 1 (0b10111).
    fn reference(mut c: u8, x: u64, bits: u32) -> u8 {
        for i in (0..bits).rev() {
            let bit = ((x >> i) & 1) as u8 ^ (c >> 3);
            c = (c << 1) & 0xf;
            if bit != 0 {
                c ^= 0b0111;
            }
        }
        c
    }
    let mut rng = RndState::new(7);
    for _ in 0..2000 {
        let x = (rng.next_u32() as u64) << 32 | rng.next_u32() as u64;
        let bits = 4 * (rng.next_u32() % 17);
        let c = (rng.next_u32() & 0xf) as u8;
        let masked = if bits == 64 { x } else { x & ((1 << bits) - 1) };
        assert_eq!(
            crc4(c, x, bits),
            reference(c, masked, bits),
            "x={x:#x} bits={bits} c={c}"
        );
    }
}
