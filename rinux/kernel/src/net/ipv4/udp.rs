// SPDX-License-Identifier: GPL-2.0
//! UDP datagrams, as in `net/ipv4/udp.c`.
//!
//! A tiny port of the datagram header handling: build a UDP header over a
//! payload with the IPv4 pseudo-header checksum (`udp4_hwcsum`/`csum_tcpudp`),
//! and read the ports off a received datagram. The socket layer, ports table
//! and corking are not ported.

use crate::include::uapi::in_::IPPROTO_UDP;
use crate::include::uapi::udp::*;
use crate::net::checksum::{csum_fold, csum_partial};

/// Writes the UDP header of `seg` (whose payload is already in `seg[8..]`),
/// filling in the length and the pseudo-header checksum. `seg.len()` is the
/// whole datagram (header + payload).
pub fn build(seg: &mut [u8], src_port: u16, dst_port: u16, src_ip: &[u8; 4], dst_ip: &[u8; 4]) {
    let total = seg.len() as u16;
    seg[UDP_OFF_SOURCE..UDP_OFF_SOURCE + 2].copy_from_slice(&src_port.to_be_bytes());
    seg[UDP_OFF_DEST..UDP_OFF_DEST + 2].copy_from_slice(&dst_port.to_be_bytes());
    seg[UDP_OFF_LEN..UDP_OFF_LEN + 2].copy_from_slice(&total.to_be_bytes());
    seg[UDP_OFF_CHECK..UDP_OFF_CHECK + 2].copy_from_slice(&[0, 0]);

    // Pseudo-header: saddr, daddr, zero, protocol, UDP length.
    let mut sum = csum_partial(src_ip, 0);
    sum = csum_partial(dst_ip, sum);
    sum = csum_partial(&[0, IPPROTO_UDP], sum);
    sum = csum_partial(&total.to_be_bytes(), sum);
    sum = csum_partial(seg, sum);
    // A computed checksum of zero is transmitted as all-ones.
    let check = match csum_fold(sum) {
        0 => 0xffff,
        c => c,
    };
    seg[UDP_OFF_CHECK..UDP_OFF_CHECK + 2].copy_from_slice(&check.to_be_bytes());
}

/// The (source port, destination port) of a received UDP datagram.
pub fn ports(seg: &[u8]) -> Option<(u16, u16)> {
    if seg.len() < UDP_HLEN {
        return None;
    }
    Some((
        u16::from_be_bytes([seg[UDP_OFF_SOURCE], seg[UDP_OFF_SOURCE + 1]]),
        u16::from_be_bytes([seg[UDP_OFF_DEST], seg[UDP_OFF_DEST + 1]]),
    ))
}
