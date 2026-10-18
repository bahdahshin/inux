// SPDX-License-Identifier: GPL-2.0
//! Cipher and hash primitives from `lib/crypto/`.
//!
//! The hash modules sit at the crate root (`sha1`, `sha256`, `md5`, ...), as
//! they were added first. The cipher cores live here.

pub mod arc4;
pub mod blake2b;
pub mod blake2s;
pub mod chacha;
pub mod hash_info;
pub mod memneq;
pub mod mpi;
pub mod mpi_add;
pub mod mpi_bit;
pub mod mpi_mul;
pub mod mpi_obj;
pub mod mpi_shift;
pub mod mpi_sqr;
pub mod mpi_sub_ui;
pub mod nh;
pub mod poly1305;
pub mod sm3;
pub mod sm4;
pub mod sm4_sbox;
pub mod utils;

// Modules registered by the /batch run (units 6-12). Each is an empty stub
// until its worker fills it in.
pub mod aes;
pub mod aescfb;
pub mod anubis;
pub mod aria;
pub mod blowfish;
pub mod camellia;
pub mod cast5;
pub mod cast6;
pub mod chacha20poly1305;
pub mod curve25519;
pub mod des;
pub mod gf128hash;
pub mod gf128mul;
pub mod khazad;
pub mod md4;
pub mod michael_mic;
pub mod rmd160;
pub mod seed;
pub mod serpent;
pub mod sha3;
pub mod sha512;
pub mod streebog;
pub mod tea;
pub mod twofish;
pub mod wp512;
