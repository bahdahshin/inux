// SPDX-License-Identifier: GPL-2.0
//! Task credentials.
//!
//! A small port of `kernel/cred.c` and `include/linux/cred.h`: the identity a
//! task acts under -- its user and group ids and its capability sets.
//!
//! Simplified heavily: rinux runs a single task as root, so there is one
//! immutable [`INIT_CRED`] rather than a per-task, reference-counted `cred`;
//! [`current_cred`] always returns it. Supplementary groups, user namespaces
//! and the securebits are not modelled.

/// `CAP_LAST_CAP` (`CAP_CHECKPOINT_RESTORE`): the highest defined capability.
pub const CAP_LAST_CAP: u32 = 40;
/// The full capability set (`CAP_FULL_SET`): every defined capability.
pub const CAP_FULL_SET: u64 = (1u64 << (CAP_LAST_CAP + 1)) - 1;

/// `struct cred`: a task's identity. `kuid_t`/`kgid_t` are 32-bit ids and
/// `kernel_cap_t` is a 64-bit capability mask.
pub struct Cred {
    pub uid: u32,
    pub gid: u32,
    pub suid: u32,
    pub sgid: u32,
    pub euid: u32,
    pub egid: u32,
    pub fsuid: u32,
    pub fsgid: u32,
    pub cap_inheritable: u64,
    pub cap_permitted: u64,
    pub cap_effective: u64,
    pub cap_bset: u64,
    pub cap_ambient: u64,
}

/// `INIT_CRED`: the credentials of the initial task -- root (all ids 0) with
/// the full capability set permitted, effective and bounding, and nothing
/// inheritable or ambient.
pub static INIT_CRED: Cred = Cred {
    uid: 0,
    gid: 0,
    suid: 0,
    sgid: 0,
    euid: 0,
    egid: 0,
    fsuid: 0,
    fsgid: 0,
    cap_inheritable: 0,
    cap_permitted: CAP_FULL_SET,
    cap_effective: CAP_FULL_SET,
    cap_bset: CAP_FULL_SET,
    cap_ambient: 0,
};

/// `current_cred()`: the credentials of the running task. rinux has one task,
/// so this is always [`INIT_CRED`].
pub fn current_cred() -> &'static Cred {
    &INIT_CRED
}
