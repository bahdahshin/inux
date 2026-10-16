// SPDX-License-Identifier: GPL-2.0
//! ICMP for IPv4, as in `net/ipv4/icmp.c`.
//!
//! A tiny port of the echo path: build an echo request (what `ping` sends)
//! and recognize an echo reply (`icmp_echo`/`icmp_rcv`). Error messages
//! (unreachable, time-exceeded) and rate limiting are not ported.

use crate::include::uapi::icmp::*;
use crate::net::checksum::ip_compute_csum;

/// Builds an ICMP echo request into `buf`: the 8-byte header with `id`/`seq`
/// followed by `payload`, with the checksum filled in. Returns the total
/// ICMP message length.
pub fn build_echo_request(buf: &mut [u8], id: u16, seq: u16, payload: &[u8]) -> usize {
    buf[ICMP_OFF_TYPE] = ICMP_ECHO;
    buf[ICMP_OFF_CODE] = 0;
    buf[ICMP_OFF_CHECKSUM..ICMP_OFF_CHECKSUM + 2].copy_from_slice(&[0, 0]);
    buf[ICMP_OFF_ID..ICMP_OFF_ID + 2].copy_from_slice(&id.to_be_bytes());
    buf[ICMP_OFF_SEQUENCE..ICMP_OFF_SEQUENCE + 2].copy_from_slice(&seq.to_be_bytes());
    buf[ICMP_HLEN..ICMP_HLEN + payload.len()].copy_from_slice(payload);
    let total = ICMP_HLEN + payload.len();
    // The checksum covers the whole ICMP message with the field zeroed.
    let check = ip_compute_csum(&buf[..total]);
    buf[ICMP_OFF_CHECKSUM..ICMP_OFF_CHECKSUM + 2].copy_from_slice(&check.to_be_bytes());
    total
}

/// An echo reply's identifier and sequence number.
pub struct EchoReply {
    pub id: u16,
    pub seq: u16,
}

/// Parses `msg` (the ICMP part of a received packet) as an echo reply.
pub fn parse_echo_reply(msg: &[u8]) -> Option<EchoReply> {
    if msg.len() < ICMP_HLEN || msg[ICMP_OFF_TYPE] != ICMP_ECHOREPLY {
        return None;
    }
    let id = u16::from_be_bytes([msg[ICMP_OFF_ID], msg[ICMP_OFF_ID + 1]]);
    let seq = u16::from_be_bytes([msg[ICMP_OFF_SEQUENCE], msg[ICMP_OFF_SEQUENCE + 1]]);
    Some(EchoReply { id, seq })
}
