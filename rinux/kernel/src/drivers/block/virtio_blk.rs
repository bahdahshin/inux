// SPDX-License-Identifier: GPL-2.0
//! The virtio block driver.
//!
//! A small port of `drivers/block/virtio_blk.c`: bind a virtio-blk device
//! and read sectors by building the three-part request chain the device
//! expects on its request queue -- a read-only header, the data buffer the
//! device writes, and a one-byte status it writes last -- then polling for
//! completion.
//!
//! Simplified from Linux's: reads only, one request at a time (polled, no
//! completion interrupt or `blk-mq`), no feature negotiation beyond the
//! defaults, and no partition scanning.

use rinux_lib::errno::{EIO, Result};

use crate::block::BlockDevice;
use crate::drivers::virtio::virtio_pci_legacy::VirtioPciLegacy;
use crate::drivers::virtio::virtio_ring::Vring;
use crate::include::uapi::virtio_blk::*;
use crate::include::uapi::virtio_ids::VIRTIO_ID_BLOCK;
use crate::include::uapi::virtio_ring::{VRING_DESC_F_NEXT, VRING_DESC_F_WRITE};
use crate::kernel::time::jiffies::{HZ, jiffies};
use crate::mm::__GFP_ZERO;
use crate::mm::page_alloc::{alloc_pages, free_pages};

/// Within the one request page: header at 0, status at 16, data at 512.
const HDR_OFF: u64 = 0;
const STATUS_OFF: u64 = 16;
const DATA_OFF: u64 = 512;

/// A bound virtio-blk device.
pub struct VirtioBlk {
    dev: VirtioPciLegacy,
    vq: Vring,
    /// One page holding the request header, status byte and data buffer.
    buf: u64,
    /// Capacity in 512-byte sectors.
    capacity: u64,
}

impl VirtioBlk {
    /// `virtblk_probe()`: find a virtio-blk device, bring it up, read its
    /// capacity and set up the request queue.
    pub fn probe() -> Option<VirtioBlk> {
        let dev = VirtioPciLegacy::find(VIRTIO_ID_BLOCK)?;
        dev.begin(0);
        let vq = dev.setup_vq(0)?;
        dev.driver_ok();
        let capacity = dev.config_read64(VIRTIO_BLK_CONFIG_CAPACITY);
        let buf = alloc_pages(__GFP_ZERO, 0);
        if buf == 0 {
            return None;
        }
        Some(VirtioBlk {
            dev,
            vq,
            buf,
            capacity,
        })
    }

    /// Brings up every virtio-blk device on the bus (a machine may have more
    /// than one disk).
    pub fn probe_all() -> alloc::vec::Vec<VirtioBlk> {
        let mut out = alloc::vec::Vec::new();
        for dev in VirtioPciLegacy::find_all(VIRTIO_ID_BLOCK) {
            dev.begin(0);
            let vq = match dev.setup_vq(0) {
                Some(v) => v,
                None => continue,
            };
            dev.driver_ok();
            let capacity = dev.config_read64(VIRTIO_BLK_CONFIG_CAPACITY);
            let buf = alloc_pages(__GFP_ZERO, 0);
            if buf == 0 {
                continue;
            }
            out.push(VirtioBlk {
                dev,
                vq,
                buf,
                capacity,
            });
        }
        out
    }

    /// Reads one sector into `out` using the three-descriptor request chain.
    fn read_one(&self, sector: u64, out: &mut [u8]) -> Result {
        let hdr = self.buf + HDR_OFF;
        let status = self.buf + STATUS_OFF;
        let data = self.buf + DATA_OFF;

        // SAFETY: the page is identity-mapped and large enough for all three
        // regions; the device reads the header and writes data + status.
        unsafe {
            (hdr as *mut VirtioBlkOutHdr).write_volatile(VirtioBlkOutHdr {
                type_: VIRTIO_BLK_T_IN,
                ioprio: 0,
                sector,
            });
            (status as *mut u8).write_volatile(0xff); // sentinel until written
        }

        // desc 0: header (device-readable), desc 1: data (device-writable),
        // desc 2: status (device-writable).
        self.vq.set_desc(0, hdr, 16, VRING_DESC_F_NEXT, 1);
        self.vq.set_desc(
            1,
            data,
            SECTOR_SIZE as u32,
            VRING_DESC_F_WRITE | VRING_DESC_F_NEXT,
            2,
        );
        self.vq.set_desc(2, status, 1, VRING_DESC_F_WRITE, 0);

        let before = self.vq.used_idx();
        self.vq.publish(0);
        self.dev.notify(0);

        let deadline = jiffies() + HZ;
        while self.vq.used_idx() == before {
            if jiffies() >= deadline {
                return Err(EIO);
            }
            core::hint::spin_loop();
        }
        self.dev.ack_isr();

        // SAFETY: the device has completed the request; read back status/data.
        let st = unsafe { (status as *const u8).read_volatile() };
        if st != VIRTIO_BLK_S_OK {
            return Err(EIO);
        }
        let n = out.len().min(SECTOR_SIZE);
        // SAFETY: data holds a full sector in identity-mapped memory.
        unsafe { core::ptr::copy_nonoverlapping(data as *const u8, out.as_mut_ptr(), n) };
        Ok(())
    }
}

impl BlockDevice for VirtioBlk {
    fn nr_sectors(&self) -> u64 {
        self.capacity
    }

    fn read(&self, sector: u64, buf: &mut [u8]) -> Result {
        if buf.is_empty() || buf.len() % SECTOR_SIZE != 0 {
            return Err(EIO);
        }
        for (i, chunk) in buf.chunks_mut(SECTOR_SIZE).enumerate() {
            self.read_one(sector + i as u64, chunk)?;
        }
        Ok(())
    }
}

impl Drop for VirtioBlk {
    fn drop(&mut self) {
        // Reset the device before freeing its ring and buffer pages, so it
        // forgets the queue addresses and cannot DMA into freed memory.
        self.dev.reset();
        if self.buf != 0 {
            free_pages(self.buf, 0);
        }
    }
}
