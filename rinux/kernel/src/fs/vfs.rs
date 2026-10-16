// SPDX-License-Identifier: GPL-2.0
//! The VFS core: inodes, path lookup and the `vfs_*` operations.
//!
//! A heavily simplified port of `fs/namei.c`, `fs/open.c`, `fs/read_write.c`
//! and `fs/libfs.c`. Instead of the dcache, the mount tree and RCU path
//! walking, rinux keeps all inodes in one arena under a single lock, and a
//! directory inode maps names straight to inode numbers. Paths are walked
//! component by component from the root, as `path_lookupat()` does, and the
//! `vfs_*` helpers mirror the ones the rest of the kernel calls.
//!
//! What is faithful: the inode/file split, the `file_operations` vtable
//! (ramfs and procfs dispatch through it), the mode and open-flag
//! semantics, and the operation set. What is not: no real dentries, no
//! mounts (procfs is grafted into the tree), no permissions, no links
//! besides symlinks, and the whole thing is serialized by one lock.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use rinux_lib::errno::{EEXIST, EINVAL, EISDIR, ENOENT, ENOTDIR, Result};

use crate::include::linux::fs::*;
use crate::kernel::locking::SpinLock;

/// The backing store of an inode (`inode::i_private` by filesystem).
pub enum InodeKind {
    /// A directory: child name to inode number.
    Dir(BTreeMap<Vec<u8>, u64>),
    /// A regular ramfs file: its bytes.
    Reg(Vec<u8>),
    /// A symbolic link: its target path.
    Lnk(Vec<u8>),
    /// A procfs file: a function that generates its contents.
    Proc(fn() -> Vec<u8>),
}

/// `struct inode`.
pub struct Inode {
    pub ino: u64,
    /// Parent directory's inode (the root is its own parent).
    pub parent: u64,
    /// Type and permission bits (`i_mode`).
    pub mode: u32,
    pub nlink: u32,
    /// `i_fop`: the file operations.
    pub fop: &'static dyn FileOperations,
    pub kind: InodeKind,
}

impl Inode {
    /// `i_size_read()`.
    pub fn size(&self) -> u64 {
        match &self.kind {
            InodeKind::Reg(data) | InodeKind::Lnk(data) => data.len() as u64,
            InodeKind::Dir(children) => children.len() as u64,
            InodeKind::Proc(generate) => generate().len() as u64,
        }
    }

    fn d_type(&self) -> u8 {
        if s_isdir(self.mode) {
            DT_DIR
        } else if s_islnk(self.mode) {
            DT_LNK
        } else {
            DT_REG
        }
    }
}

/// The whole filesystem: an inode arena.
pub struct Vfs {
    inodes: Vec<Option<Inode>>,
}

/// `ROOT_INO`.
pub const ROOT_INO: u64 = 0;

/// The one lock protecting the VFS (a "big filesystem lock").
pub static VFS: SpinLock<Vfs> = SpinLock::new(Vfs { inodes: Vec::new() });

impl Vfs {
    fn get(&self, ino: u64) -> Result<&Inode> {
        self.inodes
            .get(ino as usize)
            .and_then(|i| i.as_ref())
            .ok_or(ENOENT)
    }

    fn get_mut(&mut self, ino: u64) -> Result<&mut Inode> {
        self.inodes
            .get_mut(ino as usize)
            .and_then(|i| i.as_mut())
            .ok_or(ENOENT)
    }

    /// Allocates an inode (`new_inode()`), returning its number.
    fn new_inode(
        &mut self,
        parent: u64,
        mode: u32,
        fop: &'static dyn FileOperations,
        kind: InodeKind,
    ) -> u64 {
        let ino = self.inodes.len() as u64;
        self.inodes.push(Some(Inode {
            ino,
            parent,
            mode,
            nlink: 1,
            fop,
            kind,
        }));
        ino
    }

    /// Resolves one path component within directory `dir`.
    fn lookup_one(&self, dir: u64, name: &[u8]) -> Result<u64> {
        match &self.get(dir)?.kind {
            InodeKind::Dir(children) => match name {
                b"" | b"." => Ok(dir),
                b".." => Ok(self.get(dir)?.parent),
                _ => children.get(name).copied().ok_or(ENOENT),
            },
            _ => Err(ENOTDIR),
        }
    }

    /// `path_lookupat()`: resolve an absolute path to an inode number.
    fn path_lookup(&self, path: &[u8]) -> Result<u64> {
        let mut ino = ROOT_INO;
        for comp in path.split(|&c| c == b'/') {
            if comp.is_empty() {
                continue;
            }
            ino = self.lookup_one(ino, comp)?;
        }
        Ok(ino)
    }

    /// Splits a path into (parent inode, final component).
    fn parent_and_name<'a>(&self, path: &'a [u8]) -> Result<(u64, &'a [u8])> {
        let path = path.strip_suffix(b"/").unwrap_or(path);
        let split = path.iter().rposition(|&c| c == b'/');
        match split {
            Some(i) => Ok((self.path_lookup(&path[..i + 1])?, &path[i + 1..])),
            None => Ok((ROOT_INO, path)),
        }
    }

    fn insert_child(&mut self, dir: u64, name: &[u8], child: u64) -> Result {
        match &mut self.get_mut(dir)?.kind {
            InodeKind::Dir(children) => {
                if children.contains_key(name) {
                    return Err(EEXIST);
                }
                children.insert(name.to_vec(), child);
                Ok(())
            }
            _ => Err(ENOTDIR),
        }
    }
}

/// `init_mount()`/`vfs_create_mount` for the root: create the ramfs root
/// inode. Returns once the root exists.
pub fn init_rootfs() {
    let mut vfs = VFS.lock();
    if !vfs.inodes.is_empty() {
        return;
    }
    let root = vfs.new_inode(
        ROOT_INO,
        S_IFDIR | 0o755,
        &crate::fs::ramfs::RAMFS_FOP,
        InodeKind::Dir(BTreeMap::new()),
    );
    debug_assert_eq!(root, ROOT_INO);
}

/// `vfs_mkdir()`: create directory `path`, creating missing parents.
pub fn vfs_mkdir(path: &[u8], mode: u32) -> Result<u64> {
    let mut vfs = VFS.lock();
    mkdir_p(&mut vfs, path, mode)
}

fn mkdir_p(vfs: &mut Vfs, path: &[u8], mode: u32) -> Result<u64> {
    let mut ino = ROOT_INO;
    for comp in path.split(|&c| c == b'/') {
        if comp.is_empty() || comp == b"." {
            continue;
        }
        match vfs.lookup_one(ino, comp) {
            Ok(child) => ino = child,
            Err(_) => {
                let child = vfs.new_inode(
                    ino,
                    S_IFDIR | (mode & 0o777),
                    &crate::fs::ramfs::RAMFS_FOP,
                    InodeKind::Dir(BTreeMap::new()),
                );
                vfs.insert_child(ino, comp, child)?;
                ino = child;
            }
        }
    }
    Ok(ino)
}

/// `vfs_create()`: create (or truncate) a regular file and return its inode.
pub fn vfs_create(path: &[u8], mode: u32) -> Result<u64> {
    let mut vfs = VFS.lock();
    let (parent, name) = vfs.parent_and_name(path)?;
    if name.is_empty() {
        return Err(EINVAL);
    }
    if let Ok(existing) = vfs.lookup_one(parent, name) {
        // Truncate an existing regular file.
        match &mut vfs.get_mut(existing)?.kind {
            InodeKind::Reg(data) => {
                data.clear();
                return Ok(existing);
            }
            InodeKind::Dir(_) => return Err(EISDIR),
            _ => return Err(EEXIST),
        }
    }
    let ino = vfs.new_inode(
        parent,
        S_IFREG | (mode & 0o777),
        &crate::fs::ramfs::RAMFS_FOP,
        InodeKind::Reg(Vec::new()),
    );
    vfs.insert_child(parent, name, ino)?;
    Ok(ino)
}

/// `vfs_symlink()`.
pub fn vfs_symlink(path: &[u8], target: &[u8]) -> Result<u64> {
    let mut vfs = VFS.lock();
    let (parent, name) = vfs.parent_and_name(path)?;
    let ino = vfs.new_inode(
        parent,
        S_IFLNK | 0o777,
        &crate::fs::ramfs::RAMFS_FOP,
        InodeKind::Lnk(target.to_vec()),
    );
    vfs.insert_child(parent, name, ino)?;
    Ok(ino)
}

/// Grafts a prebuilt inode (e.g. a procfs file) into directory `dirpath`.
pub fn graft(
    dirpath: &[u8],
    name: &[u8],
    mode: u32,
    fop: &'static dyn FileOperations,
    kind: InodeKind,
) -> Result<u64> {
    let mut vfs = VFS.lock();
    let dir = vfs.path_lookup(dirpath)?;
    let ino = vfs.new_inode(dir, mode, fop, kind);
    vfs.insert_child(dir, name, ino)?;
    Ok(ino)
}

/// An open file (`struct file`).
pub struct File {
    pub ino: u64,
    pub pos: u64,
    pub flags: u32,
}

/// `filp_open()`: open `path`, creating it if `O_CREAT` and absent.
pub fn filp_open(path: &[u8], flags: u32, mode: u32) -> Result<File> {
    let ino = match kern_path(path) {
        Ok(ino) => {
            if flags & O_TRUNC != 0 {
                let mut vfs = VFS.lock();
                if let InodeKind::Reg(data) = &mut vfs.get_mut(ino)?.kind {
                    data.clear();
                }
            }
            ino
        }
        Err(_) if flags & O_CREAT != 0 => vfs_create(path, mode)?,
        Err(e) => return Err(e),
    };
    Ok(File { ino, pos: 0, flags })
}

/// `kern_path()`: resolve a path to an inode number.
pub fn kern_path(path: &[u8]) -> Result<u64> {
    VFS.lock().path_lookup(path)
}

/// `vfs_read()`: read from an open file through its `file_operations`.
pub fn vfs_read(file: &mut File, buf: &mut [u8]) -> Result<usize> {
    let (fop, is_dir) = {
        let vfs = VFS.lock();
        let inode = vfs.get(file.ino)?;
        (inode.fop, s_isdir(inode.mode))
    };
    if is_dir {
        return Err(EISDIR);
    }
    let n = fop.read(file.ino, file.pos, buf)?;
    file.pos += n as u64;
    Ok(n)
}

/// `vfs_write()`: write to an open file through its `file_operations`.
pub fn vfs_write(file: &mut File, data: &[u8]) -> Result<usize> {
    let fop = VFS.lock().get(file.ino)?.fop;
    let pos = if file.flags & O_APPEND != 0 {
        VFS.lock().get(file.ino)?.size()
    } else {
        file.pos
    };
    let n = fop.write(file.ino, pos, data)?;
    file.pos = pos + n as u64;
    Ok(n)
}

/// `vfs_llseek()`.
pub fn vfs_llseek(file: &mut File, offset: i64, whence: u32) -> Result<u64> {
    let size = VFS.lock().get(file.ino)?.size();
    let base = match whence {
        SEEK_SET => 0,
        SEEK_CUR => file.pos as i64,
        SEEK_END => size as i64,
        _ => return Err(EINVAL),
    };
    let pos = base.checked_add(offset).filter(|&p| p >= 0).ok_or(EINVAL)?;
    file.pos = pos as u64;
    Ok(file.pos)
}

/// `iterate_dir()`: the entries of directory `path`.
pub fn vfs_readdir(path: &[u8]) -> Result<Vec<DirEntry>> {
    let dir = VFS.lock().path_lookup(path)?;
    vfs_readdir_ino(dir)
}

/// `iterate_dir()` for an already-resolved directory inode (used by
/// `getdents` on an open descriptor).
pub fn vfs_readdir_ino(dir: u64) -> Result<Vec<DirEntry>> {
    let vfs = VFS.lock();
    match &vfs.get(dir)?.kind {
        InodeKind::Dir(children) => {
            let mut out = Vec::new();
            for (name, &ino) in children {
                let d_type = vfs.get(ino).map(|i| i.d_type()).unwrap_or(DT_UNKNOWN);
                out.push(DirEntry {
                    name: name.clone(),
                    ino,
                    d_type,
                });
            }
            Ok(out)
        }
        _ => Err(ENOTDIR),
    }
}

/// `vfs_stat()`: (mode, size) of a path.
pub fn vfs_stat(path: &[u8]) -> Result<(u32, u64)> {
    let vfs = VFS.lock();
    let inode = vfs.get(vfs.path_lookup(path)?)?;
    Ok((inode.mode, inode.size()))
}

/// Appends `data` to the regular file `ino` at `pos` (ramfs write core).
pub fn ramfs_write_at(ino: u64, pos: u64, data: &[u8]) -> Result<usize> {
    let mut vfs = VFS.lock();
    match &mut vfs.get_mut(ino)?.kind {
        InodeKind::Reg(buf) => {
            let end = pos as usize + data.len();
            if buf.len() < end {
                buf.resize(end, 0);
            }
            buf[pos as usize..end].copy_from_slice(data);
            Ok(data.len())
        }
        _ => Err(EINVAL),
    }
}

/// Reads from the regular file `ino` at `pos` (ramfs read core).
pub fn ramfs_read_at(ino: u64, pos: u64, buf: &mut [u8]) -> Result<usize> {
    let vfs = VFS.lock();
    match &vfs.get(ino)?.kind {
        InodeKind::Reg(data) | InodeKind::Lnk(data) => Ok(copy_from(data, pos, buf)),
        _ => Err(EINVAL),
    }
}

/// Runs a procfs generator and copies from its output at `pos`.
pub fn proc_read_at(ino: u64, pos: u64, buf: &mut [u8]) -> Result<usize> {
    let generate = {
        let vfs = VFS.lock();
        match &vfs.get(ino)?.kind {
            InodeKind::Proc(generate) => *generate,
            _ => return Err(EINVAL),
        }
    };
    // Generate outside the lock; procfs content may itself touch the VFS.
    let content = generate();
    Ok(copy_from(&content, pos, buf))
}

/// Copies `buf.len()` bytes of `data` starting at `pos`, clamped to the end.
fn copy_from(data: &[u8], pos: u64, buf: &mut [u8]) -> usize {
    let pos = pos as usize;
    if pos >= data.len() {
        return 0;
    }
    let n = buf.len().min(data.len() - pos);
    buf[..n].copy_from_slice(&data[pos..pos + n]);
    n
}

/// Reads an entire file into a `Vec` (`kernel_read_file`-style helper).
pub fn read_file(path: &[u8]) -> Result<Vec<u8>> {
    let mut file = filp_open(path, O_RDONLY, 0)?;
    let mut out = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        let n = vfs_read(&mut file, &mut chunk)?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&chunk[..n]);
    }
    Ok(out)
}

/// Total number of inodes, for reporting.
pub fn nr_inodes() -> usize {
    VFS.lock().inodes.iter().filter(|i| i.is_some()).count()
}
