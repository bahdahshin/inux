// SPDX-License-Identifier: GPL-2.0
//! A tiny sysfs, with the top-level `/sys/kernel` attributes.
//!
//! A small port of `kernel/ksysfs.c`: the read-only `/sys/kernel` attributes
//! `cpu_byteorder`, `address_bits` and `uevent_seqnum`. Like procfs, each is
//! generated on read, so they reuse the procfs `file_operations`
//! ([`crate::fs::proc::PROC_FOP`]) and the `InodeKind::Proc` generator.
//!
//! Not ported: the kobject/kset model, device/class/bus hierarchies, writable
//! attributes, and uevents -- `/sys` here is a flat set of generated files.

use alloc::vec::Vec;

use crate::fs::proc::PROC_FOP;
use crate::fs::vfs::{InodeKind, graft, vfs_mkdir};
use crate::include::linux::fs::S_IFREG;

/// `cpu_byteorder_show()`: the CPU byte order. x86 is little-endian.
fn cpu_byteorder() -> Vec<u8> {
    b"little\n".to_vec()
}

/// `address_bits_show()`: `BITS_PER_LONG`.
fn address_bits() -> Vec<u8> {
    b"64\n".to_vec()
}

/// `uevent_seqnum_show()`: the uevent sequence number. rinux emits no
/// uevents, so it stays zero.
fn uevent_seqnum() -> Vec<u8> {
    b"0\n".to_vec()
}

/// `ksysfs_init()`: create `/sys/kernel` and register its attributes.
pub fn ksysfs_init() {
    let _ = vfs_mkdir(b"/sys", 0o555);
    let _ = vfs_mkdir(b"/sys/kernel", 0o555);
    for (name, generate) in [
        (
            b"cpu_byteorder".as_slice(),
            cpu_byteorder as fn() -> Vec<u8>,
        ),
        (b"address_bits".as_slice(), address_bits),
        (b"uevent_seqnum".as_slice(), uevent_seqnum),
    ] {
        let _ = graft(
            b"/sys/kernel",
            name,
            S_IFREG | 0o444,
            &PROC_FOP,
            InodeKind::Proc(generate),
        );
    }
}
