// SPDX-License-Identifier: GPL-2.0
//! The virtio console driver.
//!
//! A small port of `drivers/char/virtio_console.c`: bind a virtio-console
//! device, perform the multiport control handshake, and send characters to
//! the host by handing device-readable bytes to port 0's transmit queue.
//!
//! The device lays its virtqueues out as port 0's receive (0) and transmit
//! (1) queues followed by the control receive (2) and transmit (3) queues
//! (`init_vqs()`). After `DRIVER_OK` the driver posts a receive buffer on the
//! control-receive queue and sends `VIRTIO_CONSOLE_DEVICE_READY`; the host
//! answers with `PORT_ADD` for each port, to which the driver replies
//! `PORT_READY`, then opens the port with `PORT_OPEN` so the host routes the
//! port's transmit data to its backing character device (`handle_control_
//! message()` / `send_control_msg()`).
//!
//! Simplified heavily from Linux's: a single console port, no hvc/tty layer,
//! no receive (console input) path, control messages handled only far enough
//! to open the port, one buffer in flight per queue, and everything polled
//! (no interrupts).

use alloc::vec::Vec;

use crate::drivers::virtio::virtio_pci_legacy::VirtioPciLegacy;
use crate::drivers::virtio::virtio_ring::Vring;
use crate::include::uapi::virtio_console::*;
use crate::include::uapi::virtio_ids::VIRTIO_ID_CONSOLE;
use crate::include::uapi::virtio_ring::VRING_DESC_F_WRITE;
use crate::kernel::time::jiffies::{HZ, jiffies};
use crate::mm::__GFP_ZERO;
use crate::mm::page_alloc::{alloc_pages, free_pages};

/// Virtqueue indices for port 0 and the control channel.
const RX_QUEUE: u16 = 0;
const TX_QUEUE: u16 = 1;
const CTL_RX_QUEUE: u16 = 2;
const CTL_TX_QUEUE: u16 = 3;
/// The device carries port 0's rx/tx queues (0, 1) and the two control queues
/// (2, 3); all four are programmed so the host never polls an unset queue.
const NR_BASE_QUEUES: u16 = 4;

/// Offsets of the sub-buffers within the one shared page.
const TX_DATA_OFF: u64 = 0;
const TX_DATA_MAX: usize = 2048;
const CTL_TX_OFF: u64 = 2048;
const CTL_RX_OFF: u64 = 2560;
const CTL_RX_LEN: u32 = 64;
const PORT_RX_OFF: u64 = 2688;
const PORT_RX_LEN: u32 = 256;

const BUF_ORDER: u32 = 0;

/// A bound virtio-console device (its single port 0).
pub struct VirtioConsole {
    dev: VirtioPciLegacy,
    /// Port 0's rx/tx and the two control queues (indices 0..4).
    queues: Vec<Vring>,
    /// One page split into the transmit buffer and the control tx/rx buffers.
    buf: u64,
    /// How many ports the device advertises (informational).
    pub max_nr_ports: u32,
}

impl VirtioConsole {
    /// `virtcons_probe()`: find a virtio-console device, negotiate multiport,
    /// set up its queues, and open port 0 through the control handshake.
    pub fn probe() -> Option<VirtioConsole> {
        let dev = VirtioPciLegacy::find(VIRTIO_ID_CONSOLE)?;
        // Negotiate multiport so the control channel is active.
        dev.begin(1 << VIRTIO_CONSOLE_F_MULTIPORT);
        let mut queues = Vec::new();
        for i in 0..NR_BASE_QUEUES {
            queues.push(dev.setup_vq(i)?);
        }
        dev.driver_ok();
        let max_nr_ports = dev.config_read32(VIRTIO_CONSOLE_CONFIG_MAX_NR_PORTS);
        let buf = alloc_pages(__GFP_ZERO, BUF_ORDER);
        if buf == 0 {
            return None;
        }
        let con = VirtioConsole {
            dev,
            queues,
            buf,
            max_nr_ports,
        };
        con.handshake();
        Some(con)
    }

    /// Posts the single control-receive buffer and kicks the queue, returning
    /// the used-ring index captured just before, for [`recv_control`] to wait
    /// on.
    fn post_control_rx(&self) -> u16 {
        let q = &self.queues[CTL_RX_QUEUE as usize];
        q.set_desc(0, self.buf + CTL_RX_OFF, CTL_RX_LEN, VRING_DESC_F_WRITE, 0);
        let before = q.used_idx();
        q.publish(0);
        self.dev.notify(CTL_RX_QUEUE);
        before
    }

    /// Waits for the control-receive buffer to be filled and returns the
    /// `(id, event, value)` of the control packet, or `None` on timeout.
    fn recv_control(&self, before: u16) -> Option<(u32, u16, u16)> {
        let q = &self.queues[CTL_RX_QUEUE as usize];
        let deadline = jiffies() + HZ / 4;
        while q.used_idx() == before {
            if jiffies() >= deadline {
                return None;
            }
            core::hint::spin_loop();
        }
        self.dev.ack_isr();
        let p = (self.buf + CTL_RX_OFF) as *const u8;
        // SAFETY: the device wrote a control packet into this identity-mapped
        // buffer; the first 8 bytes are id:u32, event:u16, value:u16 (LE).
        unsafe {
            let id = (p as *const u32).read_unaligned();
            let event = (p.add(4) as *const u16).read_unaligned();
            let value = (p.add(6) as *const u16).read_unaligned();
            Some((id, event, value))
        }
    }

    /// `__send_control_msg()`: send one control packet on the control-transmit
    /// queue and wait for the device to consume it.
    fn send_control(&self, id: u32, event: u16, value: u16) {
        let off = self.buf + CTL_TX_OFF;
        // SAFETY: CTL_TX_OFF is an 8-byte region of the identity-mapped page.
        unsafe {
            (off as *mut u32).write_unaligned(id);
            ((off + 4) as *mut u16).write_unaligned(event);
            ((off + 6) as *mut u16).write_unaligned(value);
        }
        let q = &self.queues[CTL_TX_QUEUE as usize];
        q.set_desc(0, off, VIRTIO_CONSOLE_CONTROL_SIZE as u32, 0, 0);
        let before = q.used_idx();
        q.publish(0);
        self.dev.notify(CTL_TX_QUEUE);
        let deadline = jiffies() + HZ / 4;
        while q.used_idx() == before {
            if jiffies() >= deadline {
                break;
            }
            core::hint::spin_loop();
        }
        self.dev.ack_isr();
    }

    /// Runs the port-open handshake: announce the driver is ready, then answer
    /// the host's `PORT_ADD` with `PORT_READY` and open the port, draining the
    /// control messages the host sends in response.
    fn handshake(&self) {
        // A receive buffer must be available before the host sends PORT_ADD.
        let mut before = self.post_control_rx();
        self.send_control(VIRTIO_CONSOLE_BAD_ID, VIRTIO_CONSOLE_DEVICE_READY, 1);

        // Process control messages until the host goes quiet. Bounded so a
        // misbehaving device cannot wedge the boot.
        for _ in 0..16 {
            let (id, event, _value) = match self.recv_control(before) {
                Some(m) => m,
                None => break,
            };
            if event == VIRTIO_CONSOLE_PORT_ADD {
                self.send_control(id, VIRTIO_CONSOLE_PORT_READY, 1);
                // Open the port so the host routes its transmit data.
                self.send_control(id, VIRTIO_CONSOLE_PORT_OPEN, 1);
            }
            before = self.post_control_rx();
        }
    }

    /// Sends `data` to the host console, returning the number of bytes
    /// accepted (0 on timeout). Mirrors `__send_to_port()`: a single
    /// device-readable descriptor on the transmit queue, kicked and waited on.
    pub fn write(&self, data: &[u8]) -> usize {
        let want = data.len().min(TX_DATA_MAX);
        if want == 0 {
            return 0;
        }
        // SAFETY: `TX_DATA_OFF..TX_DATA_MAX` is within the identity-mapped page.
        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr(),
                (self.buf + TX_DATA_OFF) as *mut u8,
                want,
            );
        }

        let txq = &self.queues[TX_QUEUE as usize];
        // One device-readable descriptor (no flags): the device reads it.
        txq.set_desc(0, self.buf + TX_DATA_OFF, want as u32, 0, 0);
        let before = txq.used_idx();
        txq.publish(0);
        self.dev.notify(TX_QUEUE);

        let deadline = jiffies() + HZ;
        while txq.used_idx() == before {
            if jiffies() >= deadline {
                return 0;
            }
            core::hint::spin_loop();
        }
        self.dev.ack_isr();
        want
    }

    /// Reads up to `out.len()` bytes the host has sent to the port, returning
    /// the number received (0 if none arrives within a short window). Mirrors
    /// the receive side of `fill_readbuf()`: hand the device a writable buffer
    /// on the receive queue, kick it, and wait for the host to fill it.
    pub fn read(&self, out: &mut [u8]) -> usize {
        let rxq = &self.queues[RX_QUEUE as usize];
        rxq.set_desc(
            0,
            self.buf + PORT_RX_OFF,
            PORT_RX_LEN,
            VRING_DESC_F_WRITE,
            0,
        );
        let before = rxq.used_idx();
        rxq.publish(0);
        self.dev.notify(RX_QUEUE);

        // Pending host input is delivered at once, so a short wait suffices and
        // keeps the boot from stalling when there is none (under UEFI the
        // firmware's own console driver consumes the port input first).
        let deadline = jiffies() + HZ / 2;
        while rxq.used_idx() == before {
            if jiffies() >= deadline {
                return 0;
            }
            core::hint::spin_loop();
        }
        self.dev.ack_isr();
        let elem = rxq.used_elem((before as usize) % rxq.num());
        let got = (elem.len as usize).min(out.len()).min(PORT_RX_LEN as usize);
        // SAFETY: the device wrote `elem.len` bytes into the identity-mapped
        // receive region; copy back what the caller asked for.
        unsafe {
            core::ptr::copy_nonoverlapping(
                (self.buf + PORT_RX_OFF) as *const u8,
                out.as_mut_ptr(),
                got,
            );
        }
        got
    }
}

impl Drop for VirtioConsole {
    fn drop(&mut self) {
        // Reset the device before freeing its ring and buffer pages, so it
        // forgets the queue addresses and cannot DMA into freed memory.
        self.dev.reset();
        if self.buf != 0 {
            free_pages(self.buf, BUF_ORDER);
        }
    }
}
