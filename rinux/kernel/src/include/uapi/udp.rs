// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! UDP header constants.
//!
//! A subset of `include/uapi/linux/udp.h`: the fixed 8-byte header offsets
//! (source port, destination port, length, checksum).

/// Length of the UDP header.
pub const UDP_HLEN: usize = 8;

pub const UDP_OFF_SOURCE: usize = 0;
pub const UDP_OFF_DEST: usize = 2;
pub const UDP_OFF_LEN: usize = 4;
pub const UDP_OFF_CHECK: usize = 6;
