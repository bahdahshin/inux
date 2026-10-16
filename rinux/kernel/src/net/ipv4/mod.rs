// SPDX-License-Identifier: GPL-2.0
//! IPv4, as in Linux's `net/ipv4/`.
//!
//! A tiny port of the fixed-header handling of `ip_output.c` (build an IPv4
//! header: `ip_send_check` computes the checksum) and `ip_input.c` (validate
//! and parse a received header: `ip_rcv_core`). No options, no fragment
//! reassembly, no routing table -- a single on-link peer reachable through
//! the one interface.

pub mod icmp;
pub mod ipconfig;
pub mod tcp;
pub mod udp;

use crate::include::uapi::ip::*;
use crate::net::checksum::ip_compute_csum;

/// `ip_build_and_send_pkt()` header part: write a 20-byte IPv4 header into
/// `buf` for a datagram of `payload_len` bytes, and fill in the checksum.
/// Returns [`IP_HLEN`].
pub fn build_header(
    buf: &mut [u8],
    src: &[u8; 4],
    dst: &[u8; 4],
    protocol: u8,
    payload_len: usize,
) -> usize {
    buf[IP_OFF_VER_IHL] = (IPVERSION << 4) | IP_IHL_NOOPTS;
    buf[IP_OFF_TOS] = 0;
    let tot_len = (IP_HLEN + payload_len) as u16;
    buf[IP_OFF_TOT_LEN..IP_OFF_TOT_LEN + 2].copy_from_slice(&tot_len.to_be_bytes());
    buf[IP_OFF_ID..IP_OFF_ID + 2].copy_from_slice(&0u16.to_be_bytes());
    // Don't Fragment, offset 0.
    buf[IP_OFF_FRAG_OFF..IP_OFF_FRAG_OFF + 2].copy_from_slice(&0x4000u16.to_be_bytes());
    buf[IP_OFF_TTL] = IPDEFTTL;
    buf[IP_OFF_PROTOCOL] = protocol;
    buf[IP_OFF_CHECK..IP_OFF_CHECK + 2].copy_from_slice(&[0, 0]);
    buf[IP_OFF_SADDR..IP_OFF_SADDR + 4].copy_from_slice(src);
    buf[IP_OFF_DADDR..IP_OFF_DADDR + 4].copy_from_slice(dst);
    // ip_send_check(): checksum over the header with the field zeroed.
    let check = ip_compute_csum(&buf[..IP_HLEN]);
    buf[IP_OFF_CHECK..IP_OFF_CHECK + 2].copy_from_slice(&check.to_be_bytes());
    IP_HLEN
}

/// A parsed IPv4 header.
pub struct Ipv4Header {
    pub protocol: u8,
    pub src: [u8; 4],
    pub dst: [u8; 4],
    /// Byte offset of the payload (`ihl * 4`).
    pub payload_off: usize,
    /// Total datagram length from the header.
    pub total_len: usize,
}

/// `ip_rcv_core()`: validate and parse the header of a received IPv4 packet.
pub fn parse_header(pkt: &[u8]) -> Option<Ipv4Header> {
    if pkt.len() < IP_HLEN {
        return None;
    }
    let ver = pkt[IP_OFF_VER_IHL] >> 4;
    let ihl = (pkt[IP_OFF_VER_IHL] & 0x0f) as usize;
    if ver != IPVERSION || ihl < 5 {
        return None;
    }
    let payload_off = ihl * 4;
    if pkt.len() < payload_off {
        return None;
    }
    // The header checksum must verify to zero.
    if ip_compute_csum(&pkt[..payload_off]) != 0 {
        return None;
    }
    let mut src = [0u8; 4];
    src.copy_from_slice(&pkt[IP_OFF_SADDR..IP_OFF_SADDR + 4]);
    let mut dst = [0u8; 4];
    dst.copy_from_slice(&pkt[IP_OFF_DADDR..IP_OFF_DADDR + 4]);
    let total_len = u16::from_be_bytes([pkt[IP_OFF_TOT_LEN], pkt[IP_OFF_TOT_LEN + 1]]) as usize;
    Some(Ipv4Header {
        protocol: pkt[IP_OFF_PROTOCOL],
        src,
        dst,
        payload_off,
        total_len,
    })
}
