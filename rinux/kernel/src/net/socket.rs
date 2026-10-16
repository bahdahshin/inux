// SPDX-License-Identifier: GPL-2.0
//! The socket layer, as in Linux's `net/socket.c` and `net/ipv4/af_inet.c`.
//!
//! A tiny port of the datagram socket path: a [`UdpSocket`] binds a local
//! port on the one network interface, resolves the gateway once, and then
//! sends and receives UDP datagrams -- the role of `sock_create`,
//! `inet_bind`, `udp_sendmsg` and `udp_recvmsg`. The generic `struct socket`
//! / `struct sock` split, the file-descriptor table, blocking/`poll`, and the
//! protocol-independent `sendmsg`/`recvmsg` plumbing are not ported; this is
//! one connectionless IPv4 socket over the virtio-net driver.

use rinux_lib::errno::{EIO, Result};

use crate::drivers::net::virtio_net::VirtioNet;
use crate::include::uapi::if_ether::{ETH_FRAME_LEN, ETH_HLEN, ETH_P_IP};
use crate::include::uapi::in_::IPPROTO_UDP;
use crate::include::uapi::ip::IP_HLEN;
use crate::include::uapi::udp::UDP_HLEN;
use crate::kernel::time::jiffies::HZ;
use crate::net::ipv4::{self, udp};
use crate::net::{arp, ethernet};

/// Our IPv4 address (statically configured, matching QEMU user networking).
const OUR_IP: [u8; 4] = [10, 0, 2, 15];
/// The gateway, used as the next hop for every destination.
const GATEWAY_IP: [u8; 4] = [10, 0, 2, 2];

/// A connectionless IPv4 (UDP) socket bound to a local port.
pub struct UdpSocket {
    nic: VirtioNet,
    local_port: u16,
    gw_mac: [u8; 6],
}

impl UdpSocket {
    /// `sock_create(AF_INET, SOCK_DGRAM)` + `inet_bind`: bring up the NIC,
    /// resolve the gateway, and bind `local_port`.
    pub fn bind(local_port: u16) -> Result<UdpSocket> {
        let mut nic = VirtioNet::probe().ok_or(EIO)?;
        let gw_mac = resolve_gateway(&mut nic).ok_or(EIO)?;
        Ok(UdpSocket {
            nic,
            local_port,
            gw_mac,
        })
    }

    /// `udp_sendmsg()`: send `data` to `dst_ip:dst_port`.
    pub fn send_to(&mut self, data: &[u8], dst_ip: &[u8; 4], dst_port: u16) -> Result {
        let udp_off = ETH_HLEN + IP_HLEN;
        let payload_off = udp_off + UDP_HLEN;
        let total = payload_off + data.len();
        if total > ETH_FRAME_LEN {
            return Err(EIO);
        }
        let mut frame = alloc::vec![0u8; total];
        frame[payload_off..total].copy_from_slice(data);
        let udp_total = UDP_HLEN + data.len();
        udp::build(
            &mut frame[udp_off..udp_off + udp_total],
            self.local_port,
            dst_port,
            &OUR_IP,
            dst_ip,
        );
        ipv4::build_header(
            &mut frame[ETH_HLEN..],
            &OUR_IP,
            dst_ip,
            IPPROTO_UDP,
            udp_total,
        );
        ethernet::eth_header(&mut frame, &self.gw_mac, &self.nic.mac, ETH_P_IP);
        if self.nic.send(&frame) {
            Ok(())
        } else {
            Err(EIO)
        }
    }

    /// `udp_recvmsg()`: receive one datagram addressed to our port into
    /// `buf`, returning `(len, src_ip, src_port)`.
    pub fn recv_from(&mut self, buf: &mut [u8]) -> Result<(usize, [u8; 4], u16)> {
        let mut rx = [0u8; ETH_FRAME_LEN];
        for _ in 0..32 {
            let n = self.nic.recv(&mut rx, HZ).ok_or(EIO)?;
            if ethernet::eth_type(&rx[..n]) != Some(ETH_P_IP) {
                continue;
            }
            let pkt = ethernet::eth_payload(&rx[..n]);
            let iph = match ipv4::parse_header(pkt) {
                Some(h) => h,
                None => continue,
            };
            if iph.protocol != IPPROTO_UDP || iph.dst != OUR_IP {
                continue;
            }
            let end = iph.total_len.min(pkt.len());
            if iph.payload_off + UDP_HLEN > end {
                continue;
            }
            let seg = &pkt[iph.payload_off..end];
            let (src_port, dst_port) = match udp::ports(seg) {
                Some(p) => p,
                None => continue,
            };
            if dst_port != self.local_port {
                continue;
            }
            let data = &seg[UDP_HLEN..];
            let len = data.len().min(buf.len());
            buf[..len].copy_from_slice(&data[..len]);
            return Ok((len, iph.src, src_port));
        }
        Err(EIO)
    }
}

/// Resolves the gateway's MAC with a broadcast ARP request (`arp_solicit`).
fn resolve_gateway(nic: &mut VirtioNet) -> Option<[u8; 6]> {
    let mut frame = [0u8; arp::ARP_FRAME_LEN];
    // build_request writes a broadcast Ethernet header with the ARP type.
    let len = arp::build_request(&mut frame, &nic.mac, &OUR_IP, &GATEWAY_IP);
    if !nic.send(&frame[..len]) {
        return None;
    }
    let mut rx = [0u8; ETH_FRAME_LEN];
    for _ in 0..16 {
        let n = nic.recv(&mut rx, HZ)?;
        if let Some(reply) = arp::parse_reply(&rx[..n])
            && reply.ip == GATEWAY_IP
        {
            return Some(reply.mac);
        }
    }
    None
}
