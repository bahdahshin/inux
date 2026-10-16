// SPDX-License-Identifier: GPL-2.0
//! Port of `lib/math/test_div64.c`: every entry of the 12 x 12 result table
//! for 64-by-32 division and modulo, checked through `do_div()` and the
//! generic `__div64_32()` that 32-bit kernels use.
//!
//! The C test runs each divisor both as a constant and as a variable, to
//! cover the compiler's two code paths, and repeats the table 1024 times
//! for timing. Here the table runs once through each divide function.

use rinux_lib::math::{div64_32, do_div};

const DIVIDENDS: [u64; 12] = [
    0x00000000ab275080,
    0x0000000fe73c1959,
    0x000000e54c0a74b1,
    0x00000d4398ff1ef9,
    0x0000a18c2ee1c097,
    0x00079fb80b072e4a,
    0x0072db27380dd689,
    0x0842f488162e2284,
    0xf66745411d8ab063,
    0xfffffffffffffffb,
    0xfffffffffffffffc,
    0xffffffffffffffff,
];

const DIVISORS: [u32; 12] = [
    0x00000009, 0x0000007c, 0x00000204, 0x0000cb5b, 0x00010000, 0x0008a880, 0x003fd3ae, 0x0b658fac,
    0x80000001, 0xdc08b349, 0xfffffffe, 0xffffffff,
];

/// `test_div64_results[i][j]`: `(quotient, remainder)` of
/// `DIVIDENDS[i] / DIVISORS[j]`.
const RESULTS: [[(u64, u32); 12]; 12] = [
    [
        (0x0000000013045e47, 0x00000001),
        (0x000000000161596c, 0x00000030),
        (0x000000000054e9d4, 0x00000130),
        (0x000000000000d776, 0x0000278e),
        (0x000000000000ab27, 0x00005080),
        (0x00000000000013c4, 0x0004ce80),
        (0x00000000000002ae, 0x001e143c),
        (0x000000000000000f, 0x0033e56c),
        (0x0000000000000001, 0x2b27507f),
        (0x0000000000000000, 0xab275080),
        (0x0000000000000000, 0xab275080),
        (0x0000000000000000, 0xab275080),
    ],
    [
        (0x00000001c45c02d1, 0x00000000),
        (0x0000000020d5213c, 0x00000049),
        (0x0000000007e3d65f, 0x000001dd),
        (0x0000000000140531, 0x000065ee),
        (0x00000000000fe73c, 0x00001959),
        (0x000000000001d637, 0x0004e5d9),
        (0x0000000000003fc9, 0x000713bb),
        (0x0000000000000165, 0x029abe7d),
        (0x000000000000001f, 0x673c193a),
        (0x0000000000000012, 0x6e9f7e37),
        (0x000000000000000f, 0xe73c1977),
        (0x000000000000000f, 0xe73c1968),
    ],
    [
        (0x000000197a3a0cf7, 0x00000002),
        (0x00000001d9632e5c, 0x00000021),
        (0x0000000071c28039, 0x000001cd),
        (0x000000000120a844, 0x0000b885),
        (0x0000000000e54c0a, 0x000074b1),
        (0x00000000001a7bb3, 0x00072331),
        (0x00000000000397ad, 0x0002c61b),
        (0x000000000000141e, 0x06ea2e89),
        (0x00000000000001ca, 0x4c0a72e7),
        (0x000000000000010a, 0xab002ad7),
        (0x00000000000000e5, 0x4c0a767b),
        (0x00000000000000e5, 0x4c0a7596),
    ],
    [
        (0x0000017949e37538, 0x00000001),
        (0x0000001b62441f37, 0x00000055),
        (0x0000000694a3391d, 0x00000085),
        (0x0000000010b2a5d2, 0x0000a753),
        (0x000000000d4398ff, 0x00001ef9),
        (0x0000000001882ec6, 0x0005cbf9),
        (0x000000000035333b, 0x0017abdf),
        (0x00000000000129f1, 0x0ab4520d),
        (0x0000000000001a87, 0x18ff0472),
        (0x0000000000000f6e, 0x8ac0ce9b),
        (0x0000000000000d43, 0x98ff397f),
        (0x0000000000000d43, 0x98ff2c3c),
    ],
    [
        (0x000011f321a74e49, 0x00000006),
        (0x0000014d8481d211, 0x0000005b),
        (0x0000005025cbd92d, 0x000001e3),
        (0x00000000cb5e71e3, 0x000043e6),
        (0x00000000a18c2ee1, 0x0000c097),
        (0x0000000012a88828, 0x00036c97),
        (0x000000000287f16f, 0x002c2a25),
        (0x00000000000e2cc7, 0x02d581e3),
        (0x0000000000014318, 0x2ee07d7f),
        (0x000000000000bbf4, 0x1ba08c03),
        (0x000000000000a18c, 0x2ee303af),
        (0x000000000000a18c, 0x2ee26223),
    ],
    [
        (0x0000d8db8f72935d, 0x00000005),
        (0x00000fbd5aed7a2e, 0x00000002),
        (0x000003c84b6ea64a, 0x00000122),
        (0x0000000998fa8829, 0x000044b7),
        (0x000000079fb80b07, 0x00002e4a),
        (0x00000000e16b20fa, 0x0002a14a),
        (0x000000001e940d22, 0x00353b2e),
        (0x0000000000ab40ac, 0x06fba6ba),
        (0x00000000000f3f70, 0x0af7eeda),
        (0x000000000008debd, 0x72d98365),
        (0x0000000000079fb8, 0x0b166dba),
        (0x0000000000079fb8, 0x0b0ece02),
    ],
    [
        (0x000cc3045b8fc281, 0x00000000),
        (0x0000ed1f48b5c9fc, 0x00000079),
        (0x000038fb9c63406a, 0x000000e1),
        (0x000000909705b825, 0x00000a62),
        (0x00000072db27380d, 0x0000d689),
        (0x0000000d43fce827, 0x00082b09),
        (0x00000001ccaba11a, 0x0037e8dd),
        (0x000000000a13f729, 0x0566dffd),
        (0x0000000000e5b64e, 0x3728203b),
        (0x000000000085a14b, 0x23d36726),
        (0x000000000072db27, 0x38f38cd7),
        (0x000000000072db27, 0x3880b1b0),
    ],
    [
        (0x00eafeb9c993592b, 0x00000001),
        (0x00110e5befa9a991, 0x00000048),
        (0x00041947b4a1d36a, 0x000000dc),
        (0x00000a6679327311, 0x0000c079),
        (0x00000842f488162e, 0x00002284),
        (0x000000f4459740fc, 0x00084484),
        (0x0000002122c47bf9, 0x002ca446),
        (0x00000000b9936290, 0x004979c4),
        (0x000000001085e910, 0x05a83974),
        (0x00000000099ca89d, 0x9db446bf),
        (0x000000000842f488, 0x26b40b94),
        (0x000000000842f488, 0x1e71170c),
    ],
    [
        (0x1b60cece589da1d2, 0x00000001),
        (0x01fcb42be1453f5b, 0x0000004f),
        (0x007a3f2457df0749, 0x0000013f),
        (0x0001363130e3ec7b, 0x000017aa),
        (0x0000f66745411d8a, 0x0000b063),
        (0x00001c757dfab350, 0x00048863),
        (0x000003dc4979c652, 0x00224ea7),
        (0x000000159edc3144, 0x06409ab3),
        (0x00000001ecce8a7e, 0x30bc25e5),
        (0x000000011eadfee3, 0xa99c48a8),
        (0x00000000f6674543, 0x0a593ae9),
        (0x00000000f6674542, 0x13f1f5a5),
    ],
    [
        (0x1c71c71c71c71c71, 0x00000002),
        (0x0210842108421084, 0x0000000b),
        (0x007f01fc07f01fc0, 0x000000fb),
        (0x00014245eabf1f9a, 0x0000a63d),
        (0x0000ffffffffffff, 0x0000fffb),
        (0x00001d913cecc509, 0x0007937b),
        (0x00000402c70c678f, 0x0005bfc9),
        (0x00000016766cb70b, 0x045edf97),
        (0x00000001fffffffb, 0x80000000),
        (0x0000000129d84b3a, 0xa2e8fe71),
        (0x0000000100000001, 0xfffffffd),
        (0x0000000100000000, 0xfffffffb),
    ],
    [
        (0x1c71c71c71c71c71, 0x00000003),
        (0x0210842108421084, 0x0000000c),
        (0x007f01fc07f01fc0, 0x000000fc),
        (0x00014245eabf1f9a, 0x0000a63e),
        (0x0000ffffffffffff, 0x0000fffc),
        (0x00001d913cecc509, 0x0007937c),
        (0x00000402c70c678f, 0x0005bfca),
        (0x00000016766cb70b, 0x045edf98),
        (0x00000001fffffffc, 0x00000000),
        (0x0000000129d84b3a, 0xa2e8fe72),
        (0x0000000100000002, 0x00000000),
        (0x0000000100000000, 0xfffffffc),
    ],
    [
        (0x1c71c71c71c71c71, 0x00000006),
        (0x0210842108421084, 0x0000000f),
        (0x007f01fc07f01fc0, 0x000000ff),
        (0x00014245eabf1f9a, 0x0000a641),
        (0x0000ffffffffffff, 0x0000ffff),
        (0x00001d913cecc509, 0x0007937f),
        (0x00000402c70c678f, 0x0005bfcd),
        (0x00000016766cb70b, 0x045edf9b),
        (0x00000001fffffffc, 0x00000003),
        (0x0000000129d84b3a, 0xa2e8fe75),
        (0x0000000100000002, 0x00000003),
        (0x0000000100000001, 0x00000000),
    ],
];

/// `test_div64_one()` over the whole table with `do_div()`.
#[test]
fn do_div_matches_the_table() {
    for (i, &dividend) in DIVIDENDS.iter().enumerate() {
        for (j, &divisor) in DIVISORS.iter().enumerate() {
            let mut quotient = dividend;
            let remainder = do_div(&mut quotient, divisor);
            assert_eq!(
                (quotient, remainder),
                RESULTS[i][j],
                "{dividend:#018x} / {divisor:#010x}"
            );
        }
    }
}

/// The same table through the generic shift-and-subtract `__div64_32()`.
#[test]
fn div64_32_matches_the_table() {
    for (i, &dividend) in DIVIDENDS.iter().enumerate() {
        for (j, &divisor) in DIVISORS.iter().enumerate() {
            let mut quotient = dividend;
            let remainder = div64_32(&mut quotient, divisor);
            assert_eq!(
                (quotient, remainder),
                RESULTS[i][j],
                "{dividend:#018x} / {divisor:#010x}"
            );
        }
    }
}
