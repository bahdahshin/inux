// SPDX-License-Identifier: GPL-2.0
//! The virtio entropy-source driver.
//!
//! A small port of `drivers/char/hw_random/virtio-rng.c`: bind to a
//! virtio-rng device and fetch random bytes by handing the device a
//! write-only buffer on its single request queue and waiting for it to fill
//! it. Linux registers this as an `hwrng` backend feeding the kernel entropy
//! pool; rinux calls [`VirtioRng::read`] directly.
//!
//! Simplified from Linux's: synchronous and polled (no completion
//! interrupt), one buffer at a time, and no `hwrng` registration.

use crate::drivers::virtio::virtio_pci_legacy::VirtioPciLegacy;
use crate::drivers::virtio::virtio_ring::Vring;
use crate::include::uapi::virtio_ids::VIRTIO_ID_RNG;
use crate::include::uapi::virtio_ring::VRING_DESC_F_WRITE;
use crate::kernel::time::jiffies::{HZ, jiffies};
use crate::mm::__GFP_ZERO;
use crate::mm::page_alloc::{alloc_pages, free_pages};

/// The DMA bounce buffer is one page: the most we fetch per request.
const BUF_ORDER: u32 = 0;
const BUF_SIZE: usize = 4096;

/// A bound virtio-rng device.
pub struct VirtioRng {
    dev: VirtioPciLegacy,
    vq: Vring,
    /// Physical (== virtual) address of the request buffer.
    buf: u64,
}

impl VirtioRng {
    /// `virtrng_probe()`: find a virtio-rng device, bring it up, and set up
    /// its single request queue.
    pub fn probe() -> Option<VirtioRng> {
        let dev = VirtioPciLegacy::find(VIRTIO_ID_RNG)?;
        // The RNG negotiates no features.
        dev.begin(0);
        let vq = dev.setup_vq(0)?;
        dev.driver_ok();
        let buf = alloc_pages(__GFP_ZERO, BUF_ORDER);
        if buf == 0 {
            return None;
        }
        Some(VirtioRng { dev, vq, buf })
    }

    /// Fills `out` with random bytes from the device, returning how many it
    /// produced (0 on timeout).
    pub fn read(&self, out: &mut [u8]) -> usize {
        let want = out.len().min(BUF_SIZE);
        if want == 0 {
            return 0;
        }

        // Hand the device a single write-only buffer (descriptor 0).
        self.vq
            .set_desc(0, self.buf, want as u32, VRING_DESC_F_WRITE, 0);
        let before = self.vq.used_idx();
        self.vq.publish(0);
        self.dev.notify(0);

        // Poll the used ring until the device completes the request.
        let deadline = jiffies() + HZ;
        while self.vq.used_idx() == before {
            if jiffies() >= deadline {
                return 0;
            }
            core::hint::spin_loop();
        }
        self.dev.ack_isr();

        let elem = self.vq.used_elem((before as usize) % self.vq.num());
        let got = (elem.len as usize).min(want);
        // SAFETY: the device wrote `elem.len` bytes into our identity-mapped
        // buffer; we copy back the portion the caller asked for.
        unsafe {
            core::ptr::copy_nonoverlapping(self.buf as *const u8, out.as_mut_ptr(), got);
        }
        got
    }
}

impl Drop for VirtioRng {
    fn drop(&mut self) {
        // Reset the device before freeing its ring and buffer pages, so it
        // forgets the queue addresses and cannot DMA into freed memory.
        self.dev.reset();
        if self.buf != 0 {
            free_pages(self.buf, BUF_ORDER);
        }
    }
}
