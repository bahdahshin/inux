// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! ICMP message constants.
//!
//! A subset of `include/uapi/linux/icmp.h`: the echo request/reply types and
//! the fixed 8-byte header layout (`type`, `code`, `checksum`, then a 4-byte
//! union; for echo it is `id` and `sequence`).

/// Echo Reply.
pub const ICMP_ECHOREPLY: u8 = 0;
/// Echo Request.
pub const ICMP_ECHO: u8 = 8;
/// Destination Unreachable.
pub const ICMP_DEST_UNREACH: u8 = 3;

/// Length of the fixed ICMP header.
pub const ICMP_HLEN: usize = 8;

// Byte offsets within the ICMP header.
pub const ICMP_OFF_TYPE: usize = 0;
pub const ICMP_OFF_CODE: usize = 1;
pub const ICMP_OFF_CHECKSUM: usize = 2;
pub const ICMP_OFF_ID: usize = 4;
pub const ICMP_OFF_SEQUENCE: usize = 6;
