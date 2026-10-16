// SPDX-License-Identifier: BSD-3-Clause
//! Virtio console device definitions.
//!
//! A port of the constants and config layout from
//! `include/uapi/linux/virtio_console.h`. Only the feature bits and the
//! device config fields rinux looks at are carried over; the multiport
//! control-message protocol is not ported (rinux drives the device as a
//! single, non-multiport console).

/// `VIRTIO_CONSOLE_F_SIZE`: the host reports the console size.
pub const VIRTIO_CONSOLE_F_SIZE: u32 = 0;
/// `VIRTIO_CONSOLE_F_MULTIPORT`: the host offers multiple ports.
pub const VIRTIO_CONSOLE_F_MULTIPORT: u32 = 1;
/// `VIRTIO_CONSOLE_F_EMERG_WRITE`: the host supports emergency write.
pub const VIRTIO_CONSOLE_F_EMERG_WRITE: u32 = 2;

/// `struct virtio_console_config` field offsets within the device config
/// block: `cols`:u16\@0, `rows`:u16\@2, `max_nr_ports`:u32\@4,
/// `emerg_wr`:u32\@8.
pub const VIRTIO_CONSOLE_CONFIG_COLS: u16 = 0;
pub const VIRTIO_CONSOLE_CONFIG_ROWS: u16 = 2;
pub const VIRTIO_CONSOLE_CONFIG_MAX_NR_PORTS: u16 = 4;
pub const VIRTIO_CONSOLE_CONFIG_EMERG_WR: u16 = 8;

/// `VIRTIO_CONSOLE_BAD_ID`: the port id used for device-wide control messages.
pub const VIRTIO_CONSOLE_BAD_ID: u32 = !0;

/// Control-message events (`virtio_console_control.event`).
pub const VIRTIO_CONSOLE_DEVICE_READY: u16 = 0;
pub const VIRTIO_CONSOLE_PORT_ADD: u16 = 1;
pub const VIRTIO_CONSOLE_PORT_REMOVE: u16 = 2;
pub const VIRTIO_CONSOLE_PORT_READY: u16 = 3;
pub const VIRTIO_CONSOLE_CONSOLE_PORT: u16 = 4;
pub const VIRTIO_CONSOLE_RESIZE: u16 = 5;
pub const VIRTIO_CONSOLE_PORT_OPEN: u16 = 6;
pub const VIRTIO_CONSOLE_PORT_NAME: u16 = 7;

/// `sizeof(struct virtio_console_control)`: `id`:u32, `event`:u16, `value`:u16.
pub const VIRTIO_CONSOLE_CONTROL_SIZE: usize = 8;
