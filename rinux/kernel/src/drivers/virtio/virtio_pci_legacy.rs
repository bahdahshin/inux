// SPDX-License-Identifier: GPL-2.0
//! The legacy virtio-pci transport.
//!
//! A small port of `drivers/virtio/virtio_pci_legacy.c` (and the parts of
//! `virtio_pci_common.c` it leans on): find a virtio function on the PCI
//! bus, take its BAR0 I/O region, and drive the legacy register block —
//! status handshake, feature negotiation and queue setup. Modern virtio-pci
//! (the capability-based MMIO layout) and MSI-X are not ported.

use crate::arch::x86::include::io::{inb, inl, inw, outb, outl, outw};
use crate::drivers::pci::{PciDev, scan_root};
use crate::drivers::virtio::virtio_ring::Vring;
use crate::include::uapi::pci_regs::{
    PCI_COMMAND, PCI_COMMAND_IO, PCI_COMMAND_MASTER, PCI_SUBSYSTEM_ID,
};
use crate::include::uapi::virtio_config::*;
use crate::include::uapi::virtio_pci::*;

/// A virtio device reached through its legacy PCI I/O region.
pub struct VirtioPciLegacy {
    io_base: u16,
    pub bus: u8,
    pub devfn: u8,
}

impl VirtioPciLegacy {
    /// Finds the first virtio function of type `virtio_id` (e.g.
    /// [`VIRTIO_ID_RNG`](crate::include::uapi::virtio_ids::VIRTIO_ID_RNG)),
    /// enables its I/O and bus-master, and returns a transport for it.
    pub fn find(virtio_id: u16) -> Option<VirtioPciLegacy> {
        Self::find_all(virtio_id).into_iter().next()
    }

    /// Like [`find`](Self::find), but returns a transport for every virtio
    /// function of type `virtio_id` on the bus (a machine may have several of
    /// the same device, e.g. two disks).
    pub fn find_all(virtio_id: u16) -> alloc::vec::Vec<VirtioPciLegacy> {
        let mut out = alloc::vec::Vec::new();
        for d in &scan_root() {
            if d.vendor != VIRTIO_PCI_VENDOR
                || d.device < VIRTIO_PCI_LEGACY_ID_MIN
                || d.device > VIRTIO_PCI_LEGACY_ID_MAX
            {
                continue;
            }
            // For legacy/transitional devices the virtio type is the PCI
            // subsystem ID.
            let subsys = PciDev::read_config_word(d.bus, d.devfn, PCI_SUBSYSTEM_ID);
            if subsys != virtio_id {
                continue;
            }
            let (bar0, is_io) = d.bar_address(0);
            if !is_io || bar0 == 0 {
                continue;
            }
            // Enable I/O-space decoding and bus mastering (for DMA).
            let cmd = PciDev::read_config_word(d.bus, d.devfn, PCI_COMMAND);
            PciDev::write_config_word(
                d.bus,
                d.devfn,
                PCI_COMMAND,
                cmd | PCI_COMMAND_IO | PCI_COMMAND_MASTER,
            );
            out.push(VirtioPciLegacy {
                io_base: bar0 as u16,
                bus: d.bus,
                devfn: d.devfn,
            });
        }
        out
    }

    #[inline]
    fn read8(&self, off: u16) -> u8 {
        // SAFETY: off is within BAR0's I/O region, which we own.
        unsafe { inb(self.io_base + off) }
    }
    #[inline]
    fn write8(&self, off: u16, v: u8) {
        // SAFETY: as read8; a legacy register write only configures the device.
        unsafe { outb(v, self.io_base + off) }
    }
    #[inline]
    fn read16(&self, off: u16) -> u16 {
        // SAFETY: as read8.
        unsafe { inw(self.io_base + off) }
    }
    #[inline]
    fn write16(&self, off: u16, v: u16) {
        // SAFETY: as read8.
        unsafe { outw(v, self.io_base + off) }
    }
    #[inline]
    fn read32(&self, off: u16) -> u32 {
        // SAFETY: as read8.
        unsafe { inl(self.io_base + off) }
    }
    #[inline]
    fn write32(&self, off: u16, v: u32) {
        // SAFETY: as read8.
        unsafe { outl(v, self.io_base + off) }
    }

    /// Resets the device by writing 0 to the status register.
    pub fn reset(&self) {
        self.write8(VIRTIO_PCI_STATUS, 0);
    }

    /// Reads the current status byte.
    pub fn status(&self) -> u8 {
        self.read8(VIRTIO_PCI_STATUS)
    }

    /// Sets a status bit (`add_status`), leaving the others.
    pub fn add_status(&self, bits: u8) {
        self.write8(VIRTIO_PCI_STATUS, self.status() | bits);
    }

    /// The 32-bit feature bits the device offers.
    pub fn host_features(&self) -> u32 {
        self.read32(VIRTIO_PCI_HOST_FEATURES)
    }

    /// Writes the 32-bit feature bits the driver accepts.
    pub fn set_guest_features(&self, features: u32) {
        self.write32(VIRTIO_PCI_GUEST_FEATURES, features);
    }

    /// `setup_vq()`: select queue `index`, allocate its vring and tell the
    /// device where it lives. Returns `None` if the queue does not exist.
    pub fn setup_vq(&self, index: u16) -> Option<Vring> {
        self.write16(VIRTIO_PCI_QUEUE_SEL, index);
        let num = self.read16(VIRTIO_PCI_QUEUE_NUM) as usize;
        if num == 0 {
            return None;
        }
        let vring = Vring::new(num)?;
        self.write32(VIRTIO_PCI_QUEUE_PFN, vring.pfn());
        Some(vring)
    }

    /// Kicks queue `index`: tell the device there are new available buffers.
    pub fn notify(&self, index: u16) {
        self.write16(VIRTIO_PCI_QUEUE_NOTIFY, index);
    }

    /// Reads and acknowledges the ISR status (clears it as a side effect).
    pub fn ack_isr(&self) -> u8 {
        self.read8(VIRTIO_PCI_ISR)
    }

    /// Brings the device up to `ACKNOWLEDGE | DRIVER`, negotiating `features`
    /// (masked against what the device offers). Legacy devices have no
    /// `FEATURES_OK` step.
    pub fn begin(&self, features: u32) -> u32 {
        self.reset();
        self.add_status(VIRTIO_CONFIG_S_ACKNOWLEDGE);
        self.add_status(VIRTIO_CONFIG_S_DRIVER);
        let negotiated = self.host_features() & features;
        self.set_guest_features(negotiated);
        negotiated
    }

    /// Signals the device the driver is ready (`DRIVER_OK`).
    pub fn driver_ok(&self) {
        self.add_status(VIRTIO_CONFIG_S_DRIVER_OK);
    }

    /// Reads a 32-bit field of the device-specific config block at `off`.
    pub fn config_read32(&self, off: u16) -> u32 {
        self.read32(VIRTIO_PCI_CONFIG_OFF + off)
    }

    /// Reads a 64-bit field of the device-specific config block at `off`
    /// (two 32-bit reads, little-endian).
    pub fn config_read64(&self, off: u16) -> u64 {
        let lo = self.config_read32(off) as u64;
        let hi = self.config_read32(off + 4) as u64;
        lo | (hi << 32)
    }
}
