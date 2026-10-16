// SPDX-License-Identifier: GPL-2.0
//! Host port of `lib/test_memcat_p.c`.
//!
//! The C test builds two NULL-terminated arrays of 127 elements and checks
//! the concatenation: the total, the magic on each element, the output
//! length, and the order. Here an array is a slice, so its length replaces
//! the NULL terminator and the result goes into a caller buffer.

use rinux_lib::memcat_p::memcat_p;

const MAGIC: u32 = 0xf00f_f00f;
/// Elements per input array, without the NULL terminator.
const INPUT_LEN: usize = 127;
/// Expected number of elements in the output.
const EXPECT: usize = INPUT_LEN * 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TestStruct {
    num: i32,
    magic: u32,
}

#[test]
fn memcat_p_concatenates_in_order() {
    let mut in0 = [TestStruct { num: 0, magic: 0 }; INPUT_LEN];
    let mut in1 = [TestStruct { num: 0, magic: 0 }; INPUT_LEN];
    // Same generator as the C test ("lifted from test_sort.c").
    let mut r: i64 = 1;
    for i in 0..INPUT_LEN {
        r = (r * 725861) % 6599;
        in0[i] = TestStruct {
            num: r as i32,
            magic: MAGIC,
        };
        in1[i] = TestStruct {
            num: -(r as i32),
            magic: MAGIC,
        };
    }

    let mut out = [TestStruct { num: 0, magic: 0 }; EXPECT];
    let n = memcat_p(&in0, &in1, &mut out).expect("output fits");
    assert_eq!(n, EXPECT);

    let total: i64 = out.iter().map(|e| e.num as i64).sum();
    assert_eq!(total, 0);
    assert!(out.iter().all(|e| e.magic == MAGIC));
    for i in 0..INPUT_LEN {
        assert_eq!(out[i], in0[i], "wrong element order at {i}");
        assert_eq!(out[i + INPUT_LEN], in1[i], "wrong element order at {i}");
    }
}
