// SPDX-License-Identifier: GPL-2.0
//! Hash algorithm identifiers, their names and their digest sizes.
//!
//! Ported from `lib/crypto/hash_info.c` and the `enum hash_algo` of
//! `include/uapi/linux/hash_info.h`. The C file keeps two arrays indexed by
//! the enum; here each is a `match` on [`HashAlgo`], so every variant has a
//! name and a size by construction. The variant order is the C enum's, which
//! the module signature format relies on.

/// `enum hash_algo`: the digest algorithms the kernel knows by number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashAlgo {
    Md4,
    Md5,
    Sha1,
    RipeMd160,
    Sha256,
    Sha384,
    Sha512,
    Sha224,
    RipeMd128,
    RipeMd256,
    RipeMd320,
    Wp256,
    Wp384,
    Wp512,
    Tgr128,
    Tgr160,
    Tgr192,
    Sm3_256,
    Streebog256,
    Streebog512,
    Sha3_256,
    Sha3_384,
    Sha3_512,
}

impl HashAlgo {
    /// `hash_algo_name[]`: the algorithm's name in the crypto API.
    pub const fn name(self) -> &'static str {
        match self {
            HashAlgo::Md4 => "md4",
            HashAlgo::Md5 => "md5",
            HashAlgo::Sha1 => "sha1",
            HashAlgo::RipeMd160 => "rmd160",
            HashAlgo::Sha256 => "sha256",
            HashAlgo::Sha384 => "sha384",
            HashAlgo::Sha512 => "sha512",
            HashAlgo::Sha224 => "sha224",
            HashAlgo::RipeMd128 => "rmd128",
            HashAlgo::RipeMd256 => "rmd256",
            HashAlgo::RipeMd320 => "rmd320",
            HashAlgo::Wp256 => "wp256",
            HashAlgo::Wp384 => "wp384",
            HashAlgo::Wp512 => "wp512",
            HashAlgo::Tgr128 => "tgr128",
            HashAlgo::Tgr160 => "tgr160",
            HashAlgo::Tgr192 => "tgr192",
            HashAlgo::Sm3_256 => "sm3",
            HashAlgo::Streebog256 => "streebog256",
            HashAlgo::Streebog512 => "streebog512",
            HashAlgo::Sha3_256 => "sha3-256",
            HashAlgo::Sha3_384 => "sha3-384",
            HashAlgo::Sha3_512 => "sha3-512",
        }
    }

    /// `hash_digest_size[]`: the digest length in bytes.
    pub const fn digest_size(self) -> usize {
        match self {
            HashAlgo::Md4 | HashAlgo::Md5 | HashAlgo::RipeMd128 => 16,
            HashAlgo::Sha1 | HashAlgo::RipeMd160 | HashAlgo::Tgr160 => 20,
            HashAlgo::Sha224 => 28,
            HashAlgo::Sha256
            | HashAlgo::RipeMd256
            | HashAlgo::Wp256
            | HashAlgo::Sm3_256
            | HashAlgo::Streebog256
            | HashAlgo::Sha3_256 => 32,
            HashAlgo::RipeMd320 => 40,
            HashAlgo::Tgr192 => 24,
            HashAlgo::Sha384 | HashAlgo::Wp384 | HashAlgo::Sha3_384 => 48,
            HashAlgo::Sha512 | HashAlgo::Wp512 | HashAlgo::Streebog512 | HashAlgo::Sha3_512 => 64,
            HashAlgo::Tgr128 => 16,
        }
    }
}
