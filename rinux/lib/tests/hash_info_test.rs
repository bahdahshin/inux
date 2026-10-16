// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the hash algorithm table. Linux has no KUnit file for it;
//! these pin the names and sizes to the C tables.

use rinux_lib::crypto::hash_info::HashAlgo;

#[test]
fn names_match_the_c_table() {
    assert_eq!(HashAlgo::RipeMd160.name(), "rmd160");
    assert_eq!(HashAlgo::Sm3_256.name(), "sm3");
    assert_eq!(HashAlgo::Sha3_384.name(), "sha3-384");
    assert_eq!(HashAlgo::Streebog512.name(), "streebog512");
}

#[test]
fn digest_sizes_match_the_c_table() {
    let expected = [
        (HashAlgo::Md4, 16),
        (HashAlgo::Md5, 16),
        (HashAlgo::Sha1, 20),
        (HashAlgo::RipeMd160, 20),
        (HashAlgo::Sha256, 32),
        (HashAlgo::Sha384, 48),
        (HashAlgo::Sha512, 64),
        (HashAlgo::Sha224, 28),
        (HashAlgo::RipeMd128, 16),
        (HashAlgo::RipeMd256, 32),
        (HashAlgo::RipeMd320, 40),
        (HashAlgo::Wp256, 32),
        (HashAlgo::Wp384, 48),
        (HashAlgo::Wp512, 64),
        (HashAlgo::Tgr128, 16),
        (HashAlgo::Tgr160, 20),
        (HashAlgo::Tgr192, 24),
        (HashAlgo::Sm3_256, 32),
        (HashAlgo::Streebog256, 32),
        (HashAlgo::Streebog512, 64),
        (HashAlgo::Sha3_256, 32),
        (HashAlgo::Sha3_384, 48),
        (HashAlgo::Sha3_512, 64),
    ];
    for (algo, size) in expected {
        assert_eq!(algo.digest_size(), size, "{}", algo.name());
    }
}

#[test]
fn every_name_is_distinct() {
    let all = [
        HashAlgo::Md4,
        HashAlgo::Md5,
        HashAlgo::Sha1,
        HashAlgo::RipeMd160,
        HashAlgo::Sha256,
        HashAlgo::Sha384,
        HashAlgo::Sha512,
        HashAlgo::Sha224,
        HashAlgo::RipeMd128,
        HashAlgo::RipeMd256,
        HashAlgo::RipeMd320,
        HashAlgo::Wp256,
        HashAlgo::Wp384,
        HashAlgo::Wp512,
        HashAlgo::Tgr128,
        HashAlgo::Tgr160,
        HashAlgo::Tgr192,
        HashAlgo::Sm3_256,
        HashAlgo::Streebog256,
        HashAlgo::Streebog512,
        HashAlgo::Sha3_256,
        HashAlgo::Sha3_384,
        HashAlgo::Sha3_512,
    ];
    let mut names: Vec<&str> = all.iter().map(|a| a.name()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), all.len());
}
