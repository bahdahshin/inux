// SPDX-License-Identifier: GPL-2.0
//! The `/dev/random` and `/dev/urandom` character devices.
//!
//! A reduced port of `drivers/char/random.c`: reads return random bytes and
//! writes add to the pool. Linux's generator is a cryptographically secure
//! CRNG seeded from many entropy sources; rinux instead drives the ported
//! `prandom` PRNG ([`rinux_lib::random32`]) seeded from the TSC and the RTC.
//! This is good enough to make the devices behave, but it is **not**
//! cryptographically secure, and (unlike the real `/dev/random`) it never
//! blocks for entropy.

use alloc::vec::Vec;

use rinux_lib::errno::Result;
use rinux_lib::random32::RndState;

use crate::fs::vfs::{InodeKind, graft};
use crate::include::linux::fs::{FileOperations, S_IFCHR};
use crate::kernel::locking::SpinLock;

/// The shared PRNG pool (`struct crng` in Linux, vastly simplified).
static POOL: SpinLock<RndState> = SpinLock::new(RndState::new(0));

/// `get_random_bytes()`: fill `buf` with random bytes from the pool.
///
/// The in-kernel entropy interface (`drivers/char/random.c`), used by
/// `/dev/urandom` and the `getrandom(2)` syscall. rinux's pool never blocks
/// and is not a CSPRNG (see the module note).
pub fn get_random_bytes(buf: &mut [u8]) {
    POOL.lock().fill_bytes(buf);
}

/// `/dev/random` and `/dev/urandom`: both read from the pool and never block.
pub struct RandomFop;

impl FileOperations for RandomFop {
    fn read(&self, _ino: u64, _pos: u64, buf: &mut [u8]) -> Result<usize> {
        // Fill the buffer from the pool.
        get_random_bytes(buf);
        Ok(buf.len())
    }

    fn write(&self, _ino: u64, _pos: u64, data: &[u8]) -> Result<usize> {
        // write_pool(): mix the written bytes into the pool state.
        let mut pool = POOL.lock();
        for &b in data {
            let r = pool.next_u32();
            pool.s1 ^= r ^ (b as u32);
        }
        Ok(data.len())
    }
}

static RANDOM_FOP: RandomFop = RandomFop;

/// `random_init()`: seed the pool from the TSC and the wall-clock time, then
/// register `/dev/random` and `/dev/urandom`. Must run after `/dev` exists.
pub fn random_init() {
    use crate::arch::x86::include::processor::rdtsc;
    use crate::drivers::rtc::rtc_cmos::read_time;

    let t = read_time();
    let wall = ((t.year as u64) << 40)
        | ((t.month as u64) << 32)
        | ((t.mday as u64) << 24)
        | ((t.hour as u64) << 16)
        | ((t.min as u64) << 8)
        | (t.sec as u64);
    let seed = rdtsc() ^ wall;
    {
        let mut pool = POOL.lock();
        *pool = RndState::new(seed);
        pool.warmup();
    }

    let _ = graft(
        b"/dev",
        b"random",
        S_IFCHR | 0o444,
        &RANDOM_FOP,
        InodeKind::Reg(Vec::new()),
    );
    let _ = graft(
        b"/dev",
        b"urandom",
        S_IFCHR | 0o444,
        &RANDOM_FOP,
        InodeKind::Reg(Vec::new()),
    );
}
