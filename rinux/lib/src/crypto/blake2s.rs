// SPDX-License-Identifier: GPL-2.0
//! BLAKE2s hashing, unkeyed.
//!
//! Ported from `lib/crypto/blake2s.c` (`blake2s_compress_generic()`,
//! `blake2s_update()`, `blake2s_final()`) and the unkeyed `blake2s_init()` of
//! `include/crypto/blake2s.h`. The keyed initialisation (`blake2s_init_key()`)
//! is not ported; its only difference is the key block fed in first.
//!
//! The compression function is the reference one: ten rounds of the G mixing
//! function over the 16-word state, with the message schedule from
//! `blake2s_sigma`. The counter `t` and the last-block flag `f` are kept as in C.

/// `BLAKE2S_BLOCK_SIZE`.
pub const BLAKE2S_BLOCK_SIZE: usize = 64;
/// `BLAKE2S_HASH_SIZE`: the longest digest.
pub const BLAKE2S_HASH_SIZE: usize = 32;

const IV: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

/// `blake2s_sigma`: the message word order for each round.
const SIGMA: [[usize; 16]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
];

/// `struct blake2s_ctx`.
#[derive(Clone, Debug)]
pub struct Blake2sCtx {
    h: [u32; 8],
    t: [u32; 2],
    f: [u32; 2],
    buf: [u8; BLAKE2S_BLOCK_SIZE],
    buflen: usize,
    outlen: usize,
}

/// The mixing step `G` on four state words and two message words.
fn g(v: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(12);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(8);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(7);
}

impl Blake2sCtx {
    /// `blake2s_init()`: an unkeyed context producing `outlen` bytes.
    ///
    /// Panics unless `1 <= outlen <= 32`.
    pub fn new(outlen: usize) -> Self {
        assert!(
            (1..=BLAKE2S_HASH_SIZE).contains(&outlen),
            "BLAKE2s output length"
        );
        let mut h = IV;
        // The parameter block: fanout 1, depth 1, no key, digest length.
        h[0] ^= 0x0101_0000 | outlen as u32;
        Self {
            h,
            t: [0; 2],
            f: [0; 2],
            buf: [0; BLAKE2S_BLOCK_SIZE],
            buflen: 0,
            outlen,
        }
    }

    /// `blake2s_increment_counter()`: add `inc` bytes to the 64-bit counter.
    fn increment_counter(&mut self, inc: u32) {
        self.t[0] = self.t[0].wrapping_add(inc);
        self.t[1] = self.t[1].wrapping_add(u32::from(self.t[0] < inc));
    }

    /// `blake2s_compress_generic()`: compress `nblocks` whole blocks from
    /// `data`, each advancing the counter by `inc`.
    fn compress(&mut self, data: &[u8], nblocks: usize, inc: u32) {
        for block in data.chunks_exact(BLAKE2S_BLOCK_SIZE).take(nblocks) {
            self.increment_counter(inc);
            let mut m = [0u32; 16];
            for (word, bytes) in m.iter_mut().zip(block.chunks_exact(4)) {
                *word = u32::from_le_bytes(bytes.try_into().unwrap());
            }
            let mut v = [0u32; 16];
            v[..8].copy_from_slice(&self.h);
            v[8..12].copy_from_slice(&IV[..4]);
            v[12] = IV[4] ^ self.t[0];
            v[13] = IV[5] ^ self.t[1];
            v[14] = IV[6] ^ self.f[0];
            v[15] = IV[7] ^ self.f[1];

            for s in &SIGMA {
                g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
                g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
                g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
                g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
                g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
                g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
                g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
                g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
            }
            for i in 0..8 {
                self.h[i] ^= v[i] ^ v[i + 8];
            }
        }
    }

    /// `blake2s_update()`: absorb `input`.
    pub fn update(&mut self, mut input: &[u8]) {
        if input.is_empty() {
            return;
        }
        let fill = BLAKE2S_BLOCK_SIZE - self.buflen;
        if input.len() > fill {
            // Fill the buffer and compress it; it is not the last block.
            self.buf[self.buflen..].copy_from_slice(&input[..fill]);
            let buf = self.buf;
            self.compress(&buf, 1, BLAKE2S_BLOCK_SIZE as u32);
            self.buflen = 0;
            input = &input[fill..];
        }
        // Compress all whole blocks except the last, which stays buffered
        // because it may be the final block.
        if input.len() > BLAKE2S_BLOCK_SIZE {
            let nblocks = input.len().div_ceil(BLAKE2S_BLOCK_SIZE);
            let whole = nblocks - 1;
            self.compress(input, whole, BLAKE2S_BLOCK_SIZE as u32);
            input = &input[whole * BLAKE2S_BLOCK_SIZE..];
        }
        self.buf[self.buflen..self.buflen + input.len()].copy_from_slice(input);
        self.buflen += input.len();
    }

    /// `blake2s_final()`: pad, compress the last block with the last-block
    /// flag set, and write the digest into `out` (`outlen` bytes).
    ///
    /// Panics if `out` is shorter than the digest.
    pub fn finalize(mut self, out: &mut [u8]) {
        assert!(out.len() >= self.outlen, "digest buffer too short");
        // Last block: `f[0]` is all ones, as `blake2s_set_lastblock()` sets it.
        self.f[0] = u32::MAX;
        self.buf[self.buflen..].fill(0);
        let buf = self.buf;
        self.compress(&buf, 1, self.buflen as u32);

        let mut digest = [0u8; BLAKE2S_HASH_SIZE];
        for (chunk, word) in digest.chunks_exact_mut(4).zip(self.h) {
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        out[..self.outlen].copy_from_slice(&digest[..self.outlen]);
    }
}
