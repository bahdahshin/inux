// SPDX-License-Identifier: GPL-2.0
//! rinux-lib: the Linux kernel's `lib/` directory, ported to Rust.
//!
//! Each module names the C files it was ported from. The crate is `no_std`
//! and allocation-free so the kernel can use it from the first instruction.
//!
//! Conventions shared by the modules:
//!
//! - C strings are byte slices that end at the first NUL or at the end of
//!   the slice; see [`string`].
//! - Functions that return `-Exxx` in C return [`errno::Result`].
//! - Functions that return a pointer into a buffer in C return an index.
//! - C `unsigned long` is `u64`; rinux targets 64-bit machines.

#![no_std]

pub mod bucket_locks;
pub mod bug;
pub mod bust_spinlocks;
pub mod check_signature;
pub mod clz_ctz;
pub mod clz_tab;
pub mod cputime;

pub mod argv_split;
pub mod base64;
pub mod bcd;
pub mod bitmap;
pub mod bitops;
pub mod bitrev;
pub mod bsearch;
pub mod cmdline;
pub mod cmpxchg_emu;
pub mod crc;
pub mod crypto;
pub mod ctype;
pub mod dec_and_lock;
pub mod decompress;
pub mod earlycpio;
pub mod errname;
pub mod errno;
pub mod errseq;
pub mod extable;
pub mod find_bit;
pub mod genalloc;
pub mod glob;
pub mod hash;
pub mod hexdump;
pub mod hweight;
pub mod ida;
pub mod interval_tree;
pub mod iomap_copy;
pub mod iomem_copy;
pub mod iommu_helper;
pub mod kfifo;
pub mod kstrtox;
pub mod libgcc;
pub mod linear_ranges;
pub mod list;
pub mod llist;
pub mod loadavg;
pub mod lockref;
pub mod lwq;
pub mod math;
pub mod md5;
pub mod memcat_p;
pub mod memregion;
pub mod memweight;
pub mod min_heap;
pub mod mktime;
pub mod muldi3;
pub mod net_utils;
pub mod oid_registry;
pub mod oid_registry_data;
pub mod packing;
pub mod parser;
pub mod percpu;
pub mod plist;
pub mod random32;
pub mod ratelimit;
pub mod rbtree;
pub mod refcount;
pub mod sched_eevdf;
pub mod sched_fair;
pub mod sched_pelt;
pub mod sched_rt_prio;
pub mod seq_buf;
pub mod sha1;
pub mod sha256;
pub mod siphash;
pub mod sort;
pub mod stmp_device;
pub mod string;
pub mod strncpy_from_user;
pub mod strnlen_user;
pub mod timerqueue;
pub mod ts_bm;
pub mod ts_kmp;
pub mod union_find;
pub mod usercopy;
pub mod uuid;
pub mod vsprintf;
pub mod wait;
pub mod win_minmax;
pub mod xxhash;
