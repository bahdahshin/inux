// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2024, Vladimir Oltean <olteanv@gmail.com>
//! Port of `lib/packing_test.c`: the pack and unpack table, every entry kept.
//!
//! The `packing_test_pack_fields` and `packing_test_unpack_fields` cases go
//! with the struct field helpers, which are not ported.

use rinux_lib::packing::{
    QUIRK_LITTLE_ENDIAN, QUIRK_LSW32_IS_FIRST, QUIRK_MSB_ON_THE_RIGHT, pack, unpack,
};

struct PackingCase {
    desc: &'static str,
    pbuf: &'static [u8],
    uval: u64,
    start_bit: usize,
    end_bit: usize,
    quirks: u8,
}

const CASES: &[PackingCase] = &[
    PackingCase {
        desc: "no quirks, 16 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xca, 0xfe, 0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: 0,
    },
    PackingCase {
        desc: "lsw32 first, 16 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xbe, 0xef, 0xca, 0xfe, 0xca, 0xfe, 0xde, 0xad, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_LSW32_IS_FIRST,
    },
    PackingCase {
        desc: "little endian words, 16 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xad, 0xde, 0xfe, 0xca, 0xfe, 0xca, 0xef, 0xbe, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "lsw32 first + little endian words, 16 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xfe, 0xca, 0xef, 0xbe, 0xad, 0xde, 0xfe, 0xca, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "msb right, 16 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x53, 0x7f, 0x7b, 0xb5, 0x7d, 0xf7, 0x53, 0x7f, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_MSB_ON_THE_RIGHT,
    },
    PackingCase {
        desc: "msb right + lsw32 first, 16 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x7d, 0xf7, 0x53, 0x7f, 0x53, 0x7f, 0x7b, 0xb5, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_MSB_ON_THE_RIGHT | QUIRK_LSW32_IS_FIRST,
    },
    PackingCase {
        desc: "msb right + little endian words, 16 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xb5, 0x7b, 0x7f, 0x53, 0x7f, 0x53, 0xf7, 0x7d, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_MSB_ON_THE_RIGHT | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "msb right + lsw32 first + little endian words, 16 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x7f, 0x53, 0xf7, 0x7d, 0xb5, 0x7b, 0x7f, 0x53, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_MSB_ON_THE_RIGHT | QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "no quirks, 18 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xca, 0xfe, 0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe,
            0x00, 0x00, 0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: 0,
    },
    PackingCase {
        desc: "no quirks, 19 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xca, 0xfe, 0xde, 0xad, 0xbe, 0xef, 0xca,
            0xfe, 0x00, 0x00, 0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: 0,
    },
    PackingCase {
        desc: "no quirks, 20 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xca, 0xfe, 0xde, 0xad, 0xbe, 0xef,
            0xca, 0xfe, 0x00, 0x00, 0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: 0,
    },
    PackingCase {
        desc: "no quirks, 22 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xca, 0xfe, 0xde, 0xad,
            0xbe, 0xef, 0xca, 0xfe, 0x00, 0x00, 0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: 0,
    },
    PackingCase {
        desc: "no quirks, 24 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xca, 0xfe,
            0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0x00, 0x00, 0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: 0,
    },
    PackingCase {
        desc: "lsw32 first + little endian words, 18 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xfe, 0xca, 0xef, 0xbe, 0xad, 0xde, 0xfe, 0xca, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "lsw32 first + little endian words, 19 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xfe, 0xca, 0xef, 0xbe, 0xad, 0xde, 0xfe, 0xca, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "lsw32 first + little endian words, 20 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xfe, 0xca, 0xef, 0xbe, 0xad, 0xde, 0xfe, 0xca, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "lsw32 first + little endian words, 22 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xfe, 0xca, 0xef, 0xbe, 0xad, 0xde, 0xfe, 0xca, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "lsw32 first + little endian words, 24 bytes",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xfe, 0xca, 0xef, 0xbe, 0xad, 0xde, 0xfe, 0xca, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        uval: 0xcafedeadbeefcafe,
        start_bit: 95,
        end_bit: 32,
        quirks: QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "no quirks, 16 bytes, non-aligned",
        pbuf: &[
            0x00, 0x00, 0x00, 0x89, 0x11, 0x9a, 0x22, 0xab, 0x33, 0xbc, 0x40, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0x1122334455667788,
        start_bit: 106,
        end_bit: 43,
        quirks: 0,
    },
    PackingCase {
        desc: "lsw32 first, 16 bytes, non-aligned",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x33, 0xbc, 0x40, 0x00, 0x11, 0x9a, 0x22, 0xab, 0x00, 0x00,
            0x00, 0x89,
        ],
        uval: 0x1122334455667788,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_LSW32_IS_FIRST,
    },
    PackingCase {
        desc: "little endian words, 16 bytes, non-aligned",
        pbuf: &[
            0x89, 0x00, 0x00, 0x00, 0xab, 0x22, 0x9a, 0x11, 0x00, 0x40, 0xbc, 0x33, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0x1122334455667788,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "lsw32 first + little endian words, 16 bytes, non-aligned",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0xbc, 0x33, 0xab, 0x22, 0x9a, 0x11, 0x89, 0x00,
            0x00, 0x00,
        ],
        uval: 0x1122334455667788,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "msb right, 16 bytes, non-aligned",
        pbuf: &[
            0x00, 0x00, 0x00, 0x91, 0x88, 0x59, 0x44, 0xd5, 0xcc, 0x3d, 0x02, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0x1122334455667788,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_MSB_ON_THE_RIGHT,
    },
    PackingCase {
        desc: "msb right + lsw32 first, 16 bytes, non-aligned",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xcc, 0x3d, 0x02, 0x00, 0x88, 0x59, 0x44, 0xd5, 0x00, 0x00,
            0x00, 0x91,
        ],
        uval: 0x1122334455667788,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_MSB_ON_THE_RIGHT | QUIRK_LSW32_IS_FIRST,
    },
    PackingCase {
        desc: "msb right + little endian words, 16 bytes, non-aligned",
        pbuf: &[
            0x91, 0x00, 0x00, 0x00, 0xd5, 0x44, 0x59, 0x88, 0x00, 0x02, 0x3d, 0xcc, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0x1122334455667788,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_MSB_ON_THE_RIGHT | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "msb right + lsw32 first + little endian words, 16 bytes, non-aligned",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x3d, 0xcc, 0xd5, 0x44, 0x59, 0x88, 0x91, 0x00,
            0x00, 0x00,
        ],
        uval: 0x1122334455667788,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_MSB_ON_THE_RIGHT | QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "no quirks, 16 bytes, non-aligned, 0xff",
        pbuf: &[
            0x00, 0x00, 0x07, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf8, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xffffffffffffffff,
        start_bit: 106,
        end_bit: 43,
        quirks: 0,
    },
    PackingCase {
        desc: "lsw32 first, 16 bytes, non-aligned, 0xff",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xf8, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00,
            0x07, 0xff,
        ],
        uval: 0xffffffffffffffff,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_LSW32_IS_FIRST,
    },
    PackingCase {
        desc: "little endian words, 16 bytes, non-aligned, 0xff",
        pbuf: &[
            0xff, 0x07, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0xf8, 0xff, 0xff, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xffffffffffffffff,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "lsw32 first + little endian words, 16 bytes, non-aligned, 0xff",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0xf8, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x07,
            0x00, 0x00,
        ],
        uval: 0xffffffffffffffff,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "msb right, 16 bytes, non-aligned, 0xff",
        pbuf: &[
            0x00, 0x00, 0xe0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x1f, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xffffffffffffffff,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_MSB_ON_THE_RIGHT,
    },
    PackingCase {
        desc: "msb right + lsw32 first, 16 bytes, non-aligned, 0xff",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0x1f, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00,
            0xe0, 0xff,
        ],
        uval: 0xffffffffffffffff,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_MSB_ON_THE_RIGHT | QUIRK_LSW32_IS_FIRST,
    },
    PackingCase {
        desc: "msb right + little endian words, 16 bytes, non-aligned, 0xff",
        pbuf: &[
            0xff, 0xe0, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x1f, 0xff, 0xff, 0x00, 0x00,
            0x00, 0x00,
        ],
        uval: 0xffffffffffffffff,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_MSB_ON_THE_RIGHT | QUIRK_LITTLE_ENDIAN,
    },
    PackingCase {
        desc: "msb right + lsw32 first + little endian words, 16 bytes, non-aligned, 0xff",
        pbuf: &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x1f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xe0,
            0x00, 0x00,
        ],
        uval: 0xffffffffffffffff,
        start_bit: 106,
        end_bit: 43,
        quirks: QUIRK_MSB_ON_THE_RIGHT | QUIRK_LSW32_IS_FIRST | QUIRK_LITTLE_ENDIAN,
    },
];

#[test]
fn packing_pack() {
    for c in CASES {
        let mut buf = vec![0u8; c.pbuf.len()];
        assert_eq!(
            pack(&mut buf, c.uval, c.start_bit, c.end_bit, c.quirks),
            Ok(()),
            "{}",
            c.desc
        );
        assert_eq!(buf, c.pbuf, "{}", c.desc);
    }
}

#[test]
fn packing_unpack() {
    for c in CASES {
        let mut uval = 0;
        assert_eq!(
            unpack(c.pbuf, &mut uval, c.start_bit, c.end_bit, c.quirks),
            Ok(()),
            "{}",
            c.desc
        );
        assert_eq!(uval, c.uval, "{}", c.desc);
    }
}

#[test]
fn packing_rejects_bad_ranges() {
    use rinux_lib::errno::{EINVAL, ERANGE};
    let mut buf = [0u8; 2];
    // startbit below endbit, or startbit past the end of the buffer.
    assert_eq!(pack(&mut buf, 1, 3, 4, 0), Err(EINVAL));
    assert_eq!(pack(&mut buf, 1, 16, 0, 0), Err(EINVAL));
    let mut uval = 0;
    assert_eq!(unpack(&buf, &mut uval, 3, 4, 0), Err(EINVAL));
    assert_eq!(unpack(&buf, &mut uval, 16, 0, 0), Err(EINVAL));
    // A 65-bit field is too wide for a u64.
    let big = [0u8; 9];
    assert_eq!(unpack(&big, &mut uval, 64, 0, 0), Err(ERANGE));
    assert_eq!(pack(&mut [0u8; 9], 0, 64, 0, 0), Err(ERANGE));
}
