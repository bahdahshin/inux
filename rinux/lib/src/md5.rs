// SPDX-License-Identifier: GPL-2.0
//! MD5.
//!
//! A port of the portable core of `lib/crypto/md5.c` (the `md5_transform`
//! block function) and the init/update/final streaming API, plus a one-shot
//! [`md5`]. MD5 is cryptographically broken, but the kernel keeps it for
//! legacy and non-security uses (checksums, some older protocols).
//!
//! Note MD5 is little-endian: message words are read and the length and
//! digest are written least-significant byte first, unlike the SHA family.

/// Size of an MD5 digest in bytes.
pub const MD5_DIGEST_SIZE: usize = 16;
/// MD5 processes the message in 64-byte blocks.
pub const MD5_BLOCK_SIZE: usize = 64;

/// The initial state (`MD5_H0..H3`).
const MD5_IV: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];

/// The per-round constants: `floor(2^32 * abs(sin(i + 1)))`.
#[rustfmt::skip]
const K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

/// The per-round left-rotation amounts.
#[rustfmt::skip]
const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// `md5_transform()`: fold one 64-byte `block` into `state`.
fn md5_block(state: &mut [u32; 4], block: &[u8; MD5_BLOCK_SIZE]) {
    let mut m = [0u32; 16];
    for (i, word) in m.iter_mut().enumerate() {
        let j = i * 4;
        *word = u32::from_le_bytes([block[j], block[j + 1], block[j + 2], block[j + 3]]);
    }

    let [mut a, mut b, mut c, mut d] = *state;
    for i in 0..64 {
        let (f, g) = match i {
            0..=15 => ((b & c) | (!b & d), i),
            16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let f = f.wrapping_add(a).wrapping_add(K[i]).wrapping_add(m[g]);
        a = d;
        d = c;
        c = b;
        b = b.wrapping_add(f.rotate_left(S[i]));
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
}

/// A streaming MD5 hash.
pub struct Md5 {
    state: [u32; 4],
    count: u64,
    buf: [u8; MD5_BLOCK_SIZE],
    buf_len: usize,
}

impl Default for Md5 {
    fn default() -> Self {
        Self::new()
    }
}

impl Md5 {
    /// `md5_init()`.
    pub fn new() -> Md5 {
        Md5 {
            state: MD5_IV,
            count: 0,
            buf: [0; MD5_BLOCK_SIZE],
            buf_len: 0,
        }
    }

    /// `md5_update()`: absorb more data.
    pub fn update(&mut self, mut data: &[u8]) {
        self.count = self.count.wrapping_add(data.len() as u64);

        if self.buf_len > 0 {
            let need = MD5_BLOCK_SIZE - self.buf_len;
            let take = need.min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == MD5_BLOCK_SIZE {
                let block = self.buf;
                md5_block(&mut self.state, &block);
                self.buf_len = 0;
            }
        }

        while data.len() >= MD5_BLOCK_SIZE {
            let mut block = [0u8; MD5_BLOCK_SIZE];
            block.copy_from_slice(&data[..MD5_BLOCK_SIZE]);
            md5_block(&mut self.state, &block);
            data = &data[MD5_BLOCK_SIZE..];
        }

        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    /// `md5_final()`: pad and produce the digest.
    pub fn finalize(mut self) -> [u8; MD5_DIGEST_SIZE] {
        let bit_len = self.count.wrapping_mul(8);

        let mut pad = [0u8; MD5_BLOCK_SIZE];
        pad[0] = 0x80;
        let pad_len = if self.buf_len < 56 {
            56 - self.buf_len
        } else {
            120 - self.buf_len
        };
        self.update(&pad[..pad_len]);
        // The length is appended little-endian for MD5.
        self.update(&bit_len.to_le_bytes());

        let mut out = [0u8; MD5_DIGEST_SIZE];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        out
    }
}

/// `md5()`: hash `data` in one shot.
pub fn md5(data: &[u8]) -> [u8; MD5_DIGEST_SIZE] {
    let mut h = Md5::new();
    h.update(data);
    h.finalize()
}
