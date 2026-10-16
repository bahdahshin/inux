// SPDX-License-Identifier: GPL-2.0-only
//! SM3, the Chinese hash function (GB/T 32905-2016).
//!
//! Ported from `lib/crypto/sm3.c` (`sm3_block_generic()`, `sm3_update()`,
//! `__sm3_final()`) and `include/crypto/sm3_base.h`. The C code unrolls the 64
//! rounds with macros; here the same rounds are a loop over the round index,
//! with the round constant rotated by the index as C's `K[]` table is, and the
//! message expansion written out. The state, the padding and the big-endian
//! length match the C.

/// `SM3_BLOCK_SIZE`.
pub const SM3_BLOCK_SIZE: usize = 64;
/// `SM3_DIGEST_SIZE`.
pub const SM3_DIGEST_SIZE: usize = 32;

const IV: [u32; 8] = [
    0x7380_166f,
    0x4914_b2b9,
    0x1724_42d7,
    0xda8a_0600,
    0xa96f_30bc,
    0x1631_38aa,
    0xe38d_ee4d,
    0xb0fb_0e4e,
];

/// The compression function over one 64-byte block.
fn block(state: &mut [u32; 8], data: &[u8]) {
    let mut w = [0u32; 68];
    for (i, word) in w[..16].iter_mut().enumerate() {
        *word = u32::from_be_bytes(data[4 * i..4 * i + 4].try_into().unwrap());
    }
    for j in 16..68 {
        let x = w[j - 16] ^ w[j - 9] ^ w[j - 3].rotate_left(15);
        w[j] = p1(x) ^ w[j - 13].rotate_left(7) ^ w[j - 6];
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for j in 0..64 {
        let t = if j < 16 {
            0x79cc_4519u32
        } else {
            0x7a87_9d8au32
        }
        .rotate_left(j as u32 % 32);
        let a12 = a.rotate_left(12);
        let ss1 = a12.wrapping_add(e).wrapping_add(t).rotate_left(7);
        let ss2 = ss1 ^ a12;
        let wj = w[j];
        let wp = wj ^ w[j + 4];
        let tt1 = ff(j, a, b, c)
            .wrapping_add(d)
            .wrapping_add(ss2)
            .wrapping_add(wp);
        let tt2 = gg(j, e, f, g)
            .wrapping_add(h)
            .wrapping_add(ss1)
            .wrapping_add(wj);
        d = c;
        c = b.rotate_left(9);
        b = a;
        a = tt1;
        h = g;
        g = f.rotate_left(19);
        f = e;
        e = p0(tt2);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s ^= v;
    }
}

fn p0(x: u32) -> u32 {
    x ^ x.rotate_left(9) ^ x.rotate_left(17)
}

fn p1(x: u32) -> u32 {
    x ^ x.rotate_left(15) ^ x.rotate_left(23)
}

fn ff(j: usize, x: u32, y: u32, z: u32) -> u32 {
    if j < 16 {
        x ^ y ^ z
    } else {
        (x & y) | (x & z) | (y & z)
    }
}

fn gg(j: usize, x: u32, y: u32, z: u32) -> u32 {
    if j < 16 {
        x ^ y ^ z
    } else {
        (x & y) | (!x & z)
    }
}

/// `struct sm3_ctx`: the state, a partial block and the byte count.
#[derive(Clone, Debug)]
pub struct Sm3Ctx {
    state: [u32; 8],
    buf: [u8; SM3_BLOCK_SIZE],
    bytecount: u64,
}

impl Default for Sm3Ctx {
    fn default() -> Self {
        Self::new()
    }
}

impl Sm3Ctx {
    /// `sm3_init()`.
    pub fn new() -> Self {
        Self {
            state: IV,
            buf: [0; SM3_BLOCK_SIZE],
            bytecount: 0,
        }
    }

    /// `sm3_update()`.
    pub fn update(&mut self, mut data: &[u8]) {
        let partial = (self.bytecount % SM3_BLOCK_SIZE as u64) as usize;
        self.bytecount += data.len() as u64;
        if partial + data.len() >= SM3_BLOCK_SIZE {
            let mut partial = partial;
            if partial != 0 {
                let l = SM3_BLOCK_SIZE - partial;
                self.buf[partial..].copy_from_slice(&data[..l]);
                data = &data[l..];
                let buf = self.buf;
                block(&mut self.state, &buf);
                partial = 0;
            }
            let nblocks = data.len() / SM3_BLOCK_SIZE;
            for blk in data.chunks_exact(SM3_BLOCK_SIZE).take(nblocks) {
                block(&mut self.state, blk);
            }
            data = &data[nblocks * SM3_BLOCK_SIZE..];
            self.buf[partial..partial + data.len()].copy_from_slice(data);
            return;
        }
        self.buf[partial..partial + data.len()].copy_from_slice(data);
    }

    /// `sm3_final()`: pad with a 1 bit, the zeros, and the 64-bit big-endian
    /// bit length; return the 32-byte digest.
    pub fn finalize(mut self) -> [u8; SM3_DIGEST_SIZE] {
        let bitcount = self.bytecount << 3;
        let mut partial = (self.bytecount % SM3_BLOCK_SIZE as u64) as usize;
        self.buf[partial] = 0x80;
        partial += 1;
        if partial > SM3_BLOCK_SIZE - 8 {
            self.buf[partial..].fill(0);
            let buf = self.buf;
            block(&mut self.state, &buf);
            partial = 0;
        }
        self.buf[partial..SM3_BLOCK_SIZE - 8].fill(0);
        self.buf[SM3_BLOCK_SIZE - 8..].copy_from_slice(&bitcount.to_be_bytes());
        let buf = self.buf;
        block(&mut self.state, &buf);

        let mut out = [0u8; SM3_DIGEST_SIZE];
        for (chunk, word) in out.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

/// `sm3()`: the digest of `data` in one call.
pub fn sm3(data: &[u8]) -> [u8; SM3_DIGEST_SIZE] {
    let mut ctx = Sm3Ctx::new();
    ctx.update(data);
    ctx.finalize()
}
