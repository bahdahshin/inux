// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! Socket address families, types, and the IPv4 socket address.
//!
//! A subset of `include/linux/socket.h` (the `AF_*` families), `linux/net.h`
//! (the `SOCK_*` types) and `include/uapi/linux/in.h` (`struct sockaddr_in`).

/// Address family: Internet IPv4.
pub const AF_INET: u16 = 2;

/// Socket type: a reliable byte stream (TCP).
pub const SOCK_STREAM: i32 = 1;
/// Socket type: connectionless datagrams (UDP).
pub const SOCK_DGRAM: i32 = 2;

/// `struct sockaddr_in`: an IPv4 socket address. 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SockAddrIn {
    /// Address family, always `AF_INET`.
    pub sin_family: u16,
    /// Port number, in network byte order.
    pub sin_port: u16,
    /// IPv4 address, in network byte order.
    pub sin_addr: [u8; 4],
    /// Padding to the size of `struct sockaddr`.
    pub sin_zero: [u8; 8],
}

impl SockAddrIn {
    /// Builds an `AF_INET` address from host-order `ip` and `port`.
    pub fn new(ip: [u8; 4], port: u16) -> SockAddrIn {
        SockAddrIn {
            sin_family: AF_INET,
            sin_port: port.to_be(),
            sin_addr: ip,
            sin_zero: [0; 8],
        }
    }

    /// The port in host byte order.
    pub fn port(&self) -> u16 {
        u16::from_be(self.sin_port)
    }
}
