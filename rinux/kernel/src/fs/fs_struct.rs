// SPDX-License-Identifier: GPL-2.0
//! A task's filesystem context.
//!
//! A small port of `fs/fs_struct.c` and `include/linux/fs_struct.h`: the root
//! and current working directories a task resolves paths against.
//!
//! Simplified heavily: rinux runs a single task whose root and cwd are both
//! the filesystem root, so the directories are held as fixed path strings
//! rather than reference-counted `struct path`s (dentry + mount), there is one
//! immutable [`INIT_FS`], and `chroot`/`chdir` are not modelled.

/// `struct fs_struct`: a task's root and working directories.
pub struct FsStruct {
    /// The root directory (`/` for rinux).
    pub root: &'static [u8],
    /// The current working directory (`/` for rinux).
    pub pwd: &'static [u8],
}

/// `init_fs`: the initial task's filesystem context -- rooted at `/` with `/`
/// as the working directory.
pub static INIT_FS: FsStruct = FsStruct {
    root: b"/",
    pwd: b"/",
};

/// `current->fs`: the running task's filesystem context. rinux has one task,
/// so this is always [`INIT_FS`].
pub fn current_fs() -> &'static FsStruct {
    &INIT_FS
}
