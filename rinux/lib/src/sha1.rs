// SPDX-License-Identifier: GPL-2.0
//! SHA-1.
//!
//! A port of the portable core of `lib/crypto/sha1.c`: the block transform
//! (`sha1_transform`) and the init/update/final streaming API, plus a
//! one-shot [`sha1`]. SHA-1 is cryptographically broken for collision
//! resistance, but it is still used for non-security digests (e.g. Git object
//! IDs) and legacy protocols, which is why the kernel keeps it.

/// Size of a SHA-1 digest in bytes.
pub const SHA1_DIGEST_SIZE: usize = 20;
/// SHA-1 processes the message in 64-byte blocks.
pub const SHA1_BLOCK_SIZE: usize = 64;

/// The initial hash state (`SHA1_H0..H4`).
const SHA1_IV: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

/// `sha1_transform()`: fold one 64-byte `block` into `state`.
fn sha1_block(state: &mut [u32; 5], block: &[u8; SHA1_BLOCK_SIZE]) {
    let mut w = [0u32; 80];
    for (i, word) in w.iter_mut().take(16).enumerate() {
        let j = i * 4;
        *word = u32::from_be_bytes([block[j], block[j + 1], block[j + 2], block[j + 3]]);
    }
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }

    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (i, &wi) in w.iter().enumerate() {
        let (f, k) = match i {
            0..=19 => ((b & c) | (!b & d), 0x5A827999u32),
            20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
            40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
            _ => (b ^ c ^ d, 0xCA62C1D6),
        };
        let temp = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(wi);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
}

/// A streaming SHA-1 hash.
pub struct Sha1 {
    state: [u32; 5],
    count: u64,
    buf: [u8; SHA1_BLOCK_SIZE],
    buf_len: usize,
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha1 {
    /// `sha1_init()`.
    pub fn new() -> Sha1 {
        Sha1 {
            state: SHA1_IV,
            count: 0,
            buf: [0; SHA1_BLOCK_SIZE],
            buf_len: 0,
        }
    }

    /// `sha1_update()`: absorb more data.
    pub fn update(&mut self, mut data: &[u8]) {
        self.count = self.count.wrapping_add(data.len() as u64);

        if self.buf_len > 0 {
            let need = SHA1_BLOCK_SIZE - self.buf_len;
            let take = need.min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == SHA1_BLOCK_SIZE {
                let block = self.buf;
                sha1_block(&mut self.state, &block);
                self.buf_len = 0;
            }
        }

        while data.len() >= SHA1_BLOCK_SIZE {
            let mut block = [0u8; SHA1_BLOCK_SIZE];
            block.copy_from_slice(&data[..SHA1_BLOCK_SIZE]);
            sha1_block(&mut self.state, &block);
            data = &data[SHA1_BLOCK_SIZE..];
        }

        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    /// `sha1_final()`: pad and produce the digest.
    pub fn finalize(mut self) -> [u8; SHA1_DIGEST_SIZE] {
        let bit_len = self.count.wrapping_mul(8);

        let mut pad = [0u8; SHA1_BLOCK_SIZE];
        pad[0] = 0x80;
        let pad_len = if self.buf_len < 56 {
            56 - self.buf_len
        } else {
            120 - self.buf_len
        };
        self.update(&pad[..pad_len]);
        self.update(&bit_len.to_be_bytes());

        let mut out = [0u8; SHA1_DIGEST_SIZE];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

/// `sha1()`: hash `data` in one shot.
pub fn sha1(data: &[u8]) -> [u8; SHA1_DIGEST_SIZE] {
    let mut h = Sha1::new();
    h.update(data);
    h.finalize()
}
