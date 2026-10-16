// SPDX-License-Identifier: GPL-2.0
//! The virtio network driver.
//!
//! A small port of `drivers/net/virtio_net.c`: bind a virtio-net device,
//! read its MAC, and move Ethernet frames over the receive (queue 0) and
//! transmit (queue 1) virtqueues. Each frame is prefixed on the wire with a
//! 10-byte `virtio_net_hdr`, which the driver zeroes on transmit and strips
//! on receive.
//!
//! Simplified from Linux's: polled (no NAPI or interrupts), one buffer in
//! flight per queue, no checksum/GSO offload, no mergeable receive buffers,
//! no control queue or multiqueue, and no `net_device`/napi registration --
//! the caller sends and receives frames directly.

use crate::drivers::virtio::virtio_pci_legacy::VirtioPciLegacy;
use crate::drivers::virtio::virtio_ring::Vring;
use crate::include::uapi::if_ether::{ETH_ALEN, ETH_FRAME_LEN};
use crate::include::uapi::virtio_ids::VIRTIO_ID_NET;
use crate::include::uapi::virtio_net::*;
use crate::include::uapi::virtio_ring::VRING_DESC_F_WRITE;
use crate::kernel::time::jiffies::{HZ, jiffies};
use crate::mm::__GFP_ZERO;
use crate::mm::page_alloc::{alloc_pages, free_pages};

/// A receive buffer holds the header plus a full frame.
const RX_CAP: usize = VIRTIO_NET_HDR_LEN + ETH_FRAME_LEN;

/// A bound virtio-net device.
pub struct VirtioNet {
    dev: VirtioPciLegacy,
    rx: Vring,
    tx: Vring,
    /// One page for the posted receive buffer (header + frame).
    rxbuf: u64,
    /// One page for the frame being transmitted (header + frame).
    txbuf: u64,
    /// The device's used-ring index we have already consumed on RX.
    rx_seen: u16,
    /// The device's MAC address.
    pub mac: [u8; ETH_ALEN],
}

impl VirtioNet {
    /// `virtnet_probe()`: find a virtio-net device, read its MAC, set up the
    /// receive and transmit queues, and post the first receive buffer.
    pub fn probe() -> Option<VirtioNet> {
        let dev = VirtioPciLegacy::find(VIRTIO_ID_NET)?;
        // Negotiate only the MAC feature, so the header stays 10 bytes
        // (no mergeable receive buffers).
        dev.begin(1 << VIRTIO_NET_F_MAC);

        // Read the MAC out of device config (two 32-bit reads cover 6 bytes).
        let a = dev.config_read32(VIRTIO_NET_CONFIG_MAC).to_le_bytes();
        let b = dev.config_read32(VIRTIO_NET_CONFIG_MAC + 4).to_le_bytes();
        let mac = [a[0], a[1], a[2], a[3], b[0], b[1]];

        let rx = dev.setup_vq(VIRTIO_NET_RX_QUEUE)?;
        let tx = dev.setup_vq(VIRTIO_NET_TX_QUEUE)?;
        dev.driver_ok();

        let rxbuf = alloc_pages(__GFP_ZERO, 0);
        let txbuf = alloc_pages(__GFP_ZERO, 0);
        if rxbuf == 0 || txbuf == 0 {
            return None;
        }

        let net = VirtioNet {
            dev,
            rx,
            tx,
            rxbuf,
            txbuf,
            rx_seen: 0,
            mac,
        };
        net.post_rx();
        Some(net)
    }

    /// Hands the device a receive buffer on queue 0.
    fn post_rx(&self) {
        self.rx
            .set_desc(0, self.rxbuf, RX_CAP as u32, VRING_DESC_F_WRITE, 0);
        self.rx.publish(0);
        self.dev.notify(VIRTIO_NET_RX_QUEUE);
    }

    /// Transmits one Ethernet `frame`, waiting for the device to consume it.
    pub fn send(&self, frame: &[u8]) -> bool {
        let len = frame.len().min(ETH_FRAME_LEN);
        // SAFETY: txbuf is an identity-mapped page large enough for the
        // 10-byte header plus a full frame.
        unsafe {
            core::ptr::write_bytes(self.txbuf as *mut u8, 0, VIRTIO_NET_HDR_LEN);
            core::ptr::copy_nonoverlapping(
                frame.as_ptr(),
                (self.txbuf + VIRTIO_NET_HDR_LEN as u64) as *mut u8,
                len,
            );
        }
        // The whole buffer is device-readable (no WRITE flag).
        self.tx
            .set_desc(0, self.txbuf, (VIRTIO_NET_HDR_LEN + len) as u32, 0, 0);
        let before = self.tx.used_idx();
        self.tx.publish(0);
        self.dev.notify(VIRTIO_NET_TX_QUEUE);

        let deadline = jiffies() + HZ;
        while self.tx.used_idx() == before {
            if jiffies() >= deadline {
                return false;
            }
            core::hint::spin_loop();
        }
        true
    }

    /// Receives one Ethernet frame into `out` (the `virtio_net_hdr` is
    /// stripped), re-arming the receive queue. Returns the frame length, or
    /// `None` on timeout.
    pub fn recv(&mut self, out: &mut [u8], timeout_ticks: u64) -> Option<usize> {
        let deadline = jiffies() + timeout_ticks;
        while self.rx.used_idx() == self.rx_seen {
            if jiffies() >= deadline {
                return None;
            }
            core::hint::spin_loop();
        }
        let elem = self.rx.used_elem((self.rx_seen as usize) % self.rx.num());
        self.rx_seen = self.rx_seen.wrapping_add(1);
        self.dev.ack_isr();

        let frame_len = (elem.len as usize).saturating_sub(VIRTIO_NET_HDR_LEN);
        let n = frame_len.min(out.len());
        // SAFETY: the device wrote header + frame into our identity-mapped
        // receive buffer; copy the frame out, past the 10-byte header.
        unsafe {
            core::ptr::copy_nonoverlapping(
                (self.rxbuf + VIRTIO_NET_HDR_LEN as u64) as *const u8,
                out.as_mut_ptr(),
                n,
            );
        }
        self.post_rx();
        Some(frame_len)
    }
}

impl Drop for VirtioNet {
    fn drop(&mut self) {
        // Reset the device first (as `virtnet_remove()` does via the
        // transport's reset): this clears DRIVER_OK and drops the queue
        // addresses, so the device stops DMAing into the ring and buffer
        // pages that are about to be freed. Without it, a packet arriving
        // after teardown makes the host read freed memory as a vring.
        self.dev.reset();
        if self.rxbuf != 0 {
            free_pages(self.rxbuf, 0);
        }
        if self.txbuf != 0 {
            free_pages(self.txbuf, 0);
        }
    }
}
