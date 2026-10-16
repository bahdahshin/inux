// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! The virtio network device interface.
//!
//! A subset of `include/uapi/linux/virtio_net.h`: the per-packet header, the
//! MAC feature bit and the config-space MAC offset. Checksum/GSO offload,
//! mergeable receive buffers, the control queue and multiqueue are not
//! ported; the driver negotiates only `VIRTIO_NET_F_MAC`.

/// `VIRTIO_NET_F_MAC`: the device supplies a MAC address in its config.
pub const VIRTIO_NET_F_MAC: u32 = 5;
/// `VIRTIO_NET_F_MRG_RXBUF`: receive-buffer merging (deliberately not used,
/// so the per-packet header stays [`VIRTIO_NET_HDR_LEN`] bytes).
pub const VIRTIO_NET_F_MRG_RXBUF: u32 = 15;

/// `struct virtio_net_hdr` without mergeable buffers: 10 bytes that prefix
/// every transmitted and received frame.
pub const VIRTIO_NET_HDR_LEN: usize = 10;

/// Offset of the `mac[6]` field in the device config space.
pub const VIRTIO_NET_CONFIG_MAC: u16 = 0;

// Receive queue is 0, transmit queue is 1 (no control queue without the
// VIRTIO_NET_F_CTRL_VQ feature).
pub const VIRTIO_NET_RX_QUEUE: u16 = 0;
pub const VIRTIO_NET_TX_QUEUE: u16 = 1;
