// SPDX-License-Identifier: GPL-2.0
//! The virtual filesystem, as in Linux's `fs/`.

pub mod binfmt_elf;
pub mod exec;
pub mod ext2;
pub mod ext4;
pub mod fat;
pub mod fs_struct;
pub mod proc;
pub mod ramfs;
pub mod sysfs;
pub mod vfs;
