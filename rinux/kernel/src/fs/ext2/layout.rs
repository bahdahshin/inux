// SPDX-License-Identifier: GPL-2.0
//! The ext2 on-disk layout.
//!
//! A subset of `fs/ext2/ext2.h`: the superblock, block group descriptor,
//! inode and directory entry as they appear on disk, plus the constants the
//! reader needs. All multi-byte fields are little-endian; rinux only runs on
//! little-endian x86, so the structs are read directly. Fields past what the
//! read path uses are omitted.

/// The superblock lives at this byte offset, regardless of block size.
pub const EXT2_SUPERBLOCK_OFFSET: u64 = 1024;
/// `s_magic` value for a valid ext2 filesystem.
pub const EXT2_SUPER_MAGIC: u16 = 0xEF53;
/// The root directory's inode number.
pub const EXT2_ROOT_INO: u32 = 2;
/// Inode size before revision 1 fixed it in the superblock.
pub const EXT2_GOOD_OLD_INODE_SIZE: u16 = 128;

/// Number of direct block pointers in an inode.
pub const EXT2_NDIR_BLOCKS: usize = 12;
/// Index of the single-indirect block pointer.
pub const EXT2_IND_BLOCK: usize = EXT2_NDIR_BLOCKS;
/// Total block pointers (12 direct, single/double/triple indirect).
pub const EXT2_N_BLOCKS: usize = 15;

/// Directory entry file types.
pub const EXT2_FT_REG_FILE: u8 = 1;
pub const EXT2_FT_DIR: u8 = 2;

// Mode bits (`i_mode & EXT2_S_IFMT`).
pub const EXT2_S_IFMT: u16 = 0xF000;
pub const EXT2_S_IFDIR: u16 = 0x4000;
pub const EXT2_S_IFREG: u16 = 0x8000;

/// `struct ext2_super_block`, truncated after the fields the reader uses.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ext2SuperBlock {
    pub s_inodes_count: u32,
    pub s_blocks_count: u32,
    pub s_r_blocks_count: u32,
    pub s_free_blocks_count: u32,
    pub s_free_inodes_count: u32,
    pub s_first_data_block: u32,
    /// Block size is `1024 << s_log_block_size`.
    pub s_log_block_size: u32,
    pub s_log_frag_size: u32,
    pub s_blocks_per_group: u32,
    pub s_frags_per_group: u32,
    pub s_inodes_per_group: u32,
    pub s_mtime: u32,
    pub s_wtime: u32,
    pub s_mnt_count: u16,
    pub s_max_mnt_count: u16,
    pub s_magic: u16,
    pub s_state: u16,
    pub s_errors: u16,
    pub s_minor_rev_level: u16,
    pub s_lastcheck: u32,
    pub s_checkinterval: u32,
    pub s_creator_os: u32,
    pub s_rev_level: u32,
    pub s_def_resuid: u16,
    pub s_def_resgid: u16,
    pub s_first_ino: u32,
    pub s_inode_size: u16,
}

/// `struct ext2_group_desc`: one per block group, 32 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ext2GroupDesc {
    pub bg_block_bitmap: u32,
    pub bg_inode_bitmap: u32,
    /// First block of this group's inode table.
    pub bg_inode_table: u32,
    pub bg_free_blocks_count: u16,
    pub bg_free_inodes_count: u16,
    pub bg_used_dirs_count: u16,
    pub bg_pad: u16,
    pub bg_reserved: [u32; 3],
}

/// `struct ext2_inode`, truncated after the direct/indirect block pointers.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ext2Inode {
    pub i_mode: u16,
    pub i_uid: u16,
    pub i_size: u32,
    pub i_atime: u32,
    pub i_ctime: u32,
    pub i_mtime: u32,
    pub i_dtime: u32,
    pub i_gid: u16,
    pub i_links_count: u16,
    pub i_blocks: u32,
    pub i_flags: u32,
    pub i_osd1: u32,
    /// 12 direct, then single/double/triple indirect block numbers.
    pub i_block: [u32; EXT2_N_BLOCKS],
    pub i_generation: u32,
    pub i_file_acl: u32,
    pub i_dir_acl: u32,
}

impl Ext2Inode {
    /// True if this inode is a directory.
    pub fn is_dir(&self) -> bool {
        self.i_mode & EXT2_S_IFMT == EXT2_S_IFDIR
    }
    /// True if this inode is a regular file.
    pub fn is_reg(&self) -> bool {
        self.i_mode & EXT2_S_IFMT == EXT2_S_IFREG
    }
    /// File size in bytes.
    pub fn size(&self) -> u64 {
        self.i_size as u64
    }
}

/// Reads a `#[repr(C)]` value of type `T` from `buf` at byte offset `off`,
/// tolerating any alignment. Returns `None` if `buf` is too short.
pub fn read_struct<T: Copy>(buf: &[u8], off: usize) -> Option<T> {
    if off + core::mem::size_of::<T>() > buf.len() {
        return None;
    }
    // SAFETY: the bounds are checked; T is Copy and repr(C), and the read is
    // unaligned, so any byte offset is valid.
    Some(unsafe { (buf.as_ptr().add(off) as *const T).read_unaligned() })
}
