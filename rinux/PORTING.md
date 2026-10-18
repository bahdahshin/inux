# Porting Linux to Rust

This file tracks the conversion of the kernel in `../linux` (Linux 7.3-rc5,
commit `6addb4f38557`) to Rust in this directory. It is the source of truth
for progress: `scripts/progress.py` counts the file table below against the
kernel tree.

```bash
python rinux/scripts/progress.py
```

## Scale

The kernel tree has 65,899 C files, headers and assembly files
(`*.c`, `*.h`, `*.S`). Most of it is hardware support that a Rust x86-64
kernel will only need a sliver of:

| Directory | Files | What it is |
|---|---:|---|
| `drivers/` | 34,273 | Device drivers |
| `arch/` | 10,370 | 20+ CPU architectures (rinux needs `arch/x86`) |
| `include/` | 6,724 | Headers |
| `tools/` | 5,156 | User-space tools, perf, selftests |
| `sound/` | 2,721 | ALSA |
| `fs/` | 2,174 | VFS and filesystems |
| `net/` | 1,742 | Networking |
| `lib/` | 830 | Library routines |
| `kernel/` | 663 | Core: scheduler, signals, time, ... |
| `security/` | 269 | LSMs |
| `mm/` | 195 | Memory management |
| other | 782 | `crypto/`, `block/`, `io_uring/`, `init/`, `ipc/`, ... |

## Roadmap

Work goes one step at a time. Each step ports a coherent set of files, ports
their KUnit/self tests to Rust tests, and updates the table below.

| Step | Scope | Status |
|---|---|---|
| 1 | `lib/` foundations: errno, ctype, string, kstrtox, sort, bsearch, math, CRC, hexdump, glob, prandom | done |
| 2 | `lib/` for boot: vsprintf, cmdline/parser, bitmap/find_bit, bitops, hash, base64, uuid, siphash | done |
| 3 | Boot: GRUB multiboot2 entry, early serial and VGA console, printk, panic; `rinux-boot.iso` | done |
| 4 | CPU setup: GDT/TSS/IDT, exceptions and oopses, 8259 PIC, PIT tick, TSC calibration and `sched_clock`, BogoMIPS (local APIC, IO-APIC and HPET later) | done |
| 5 | `mm/`: memblock from the e820 map, kernel page tables with a direct map (page 0 unmapped), buddy page allocator, `kmalloc`/slab wired to Rust's global allocator, `vmalloc`; the intrusive `list.h` and `rbtree` they are built on | done |
| 6 | `kernel/`: `task_struct`, the x86-64 context switch, a round-robin runqueue with `schedule()`, timer-driven preemption, kernel threads, and the boot path as the idle task `swapper/0` (CFS, wait queues and user tasks later) | done |
| 7 | `fs/`: a VFS core (inode/file/dentry split, `file_operations` vtable, path lookup, `vfs_*` helpers), ramfs as the root, a `newc` cpio initramfs unpacker, and a small procfs; the dcache, mount tree and RCU path walk are reduced to one inode arena under a lock | done |
| 8 | User space: SYSCALL/SYSRET entry, per-task user page tables, an ELF64 loader, `execve`, `brk`, a console tty, and running `/init` from the initramfs in ring 3. **Milestone: a static `/init` runs in user mode (write/getpid/exit); BusyBox (dynamic loader, more syscalls, a keyboard tty) is future work** | done |
| 9 | `drivers/pci/`: type-1 (CF8/CFC) config-space access and a PCI bus enumeration from bus 0, recursing through bridges, reading each function's identity, BARs and IRQ | done |
| 10 | `drivers/virtio/`: the legacy virtio-pci transport and a split virtqueue, with a `virtio-rng` driver that runs a real DMA transaction (negotiate, set up the ring, kick, poll completion) | done |
| 11 | `block/` and `drivers/block/virtio_blk.c`: a minimal block-device interface and a virtio-blk driver that reads sectors over the request queue | done |
| 12 | `fs/ext2/`: a read-only ext2 filesystem -- superblock, inodes, direct and single-indirect block mapping, and directory lookup -- mounted on a virtio-blk disk | done |
| 13 | `drivers/net/virtio_net.c` and `net/`: a virtio-net driver with Ethernet framing and ARP, resolving the gateway's MAC over real transmit/receive queues | done |
| 14 | `net/ipv4/`: the Internet checksum, an IPv4 header and ICMP echo, so rinux pings the gateway over virtio-net | done |
| 15 | UDP (`net/ipv4/udp.c`) and the kernel DHCP client (`net/ipv4/ipconfig.c`): a full DHCP exchange that leases an address from the network | done |
| 16 | TCP (`net/ipv4/tcp_output.c`/`tcp_input.c`): an active-open client that completes the three-way handshake and fetches an HTTP response over the link | done |
| 17 | a datagram socket layer (`net/socket.c`, `net/ipv4/af_inet.c`): a `UdpSocket` (bind/send_to/recv_from) over the stack, echoing off a host server | done |
| 18 | `drivers/rtc/`: the PC CMOS real-time clock -- read the wall-clock time through the CMOS ports, decoding BCD/12-hour form | done |
| 19 | `crypto/`: SHA-256 in rinux-lib, validated against the FIPS 180-4 vectors (host tests) and a boot-time known-answer check | done |
| 20 | `fs/ext4/`: a read-only ext4 filesystem with extent-tree block mapping, mounted on a virtio-blk disk | done |
| 21 | `lib/`: `memweight` (count set bits in a byte region) and `union_find` (disjoint sets with path halving and union by rank over an index-linked node slice), each with host tests; the rest of `lib/` that the kernel needs stays for later | done |
| 22 | `lib/`: `packing` (bitfield pack/unpack with the endian and bit-order quirks) with the 34-case `packing_test.c` table ported in full; the struct field helpers wait for raw struct access | done |
| 23 | `lib/linear_ranges.c`: the selector/value tables used by regulator drivers, with the KUnit test ported in full; the `EXPORT`/module boilerplate is dropped | done |
| 24 | `lib/oid_registry.c`: `look_up_OID()`, `parse_OID()` and `sprint_oid()`, with the 106-entry table generated from `oid_registry.h` by `scripts/gen_oid_registry.py` (Linux generates it at build time) | done |
| 25 | `lib/errseq.c`: error sequence numbers (`errseq_set()`, `errseq_sample()`, `errseq_check()`, `errseq_check_and_advance()`) over an `AtomicU32`, with host tests for the sample/set/check protocol; Linux has no KUnit file for it | done |
| 26 | `lib/argv_split.c`: whitespace word splitting as an allocation-free iterator of index ranges over the caller's bytes (`argv_split()`, `count_argc()`), with host tests; `argv_free()` and the `kmalloc`ed array are left to the caller | done |
| 27 | `lib/errname.c`: `errname()` (the symbolic name of an errno, with the sign kept), built on the crate's errno table instead of a second name table; host tests walk every entry | done |
| 28 | `lib/math/polynomial.c`: integer polynomial evaluation with factor redistribution (`polynomial_calc()`), with the temperature conversion from the C comment checked against the real formula; a tolerance covers the truncation at each power | done |
| 29 | `lib/clz_ctz.c`: the `__clzsi2`, `__ctzsi2`, `__clzdi2` and `__ctzdi2` builtins, with host tests; zero gives the bit width, where C's `__ffs(0)` is undefined | done |
| 30 | `lib/seq_buf.c`: the bounded text buffer with its overflow marker, plus its formatted writes and hex dump; the seq_file, printk and user-copy parts wait for those subsystems | partial |
| 31 | `lib/iommu-helper.c`: the free-area allocator that keeps allocations from crossing a power-of-two boundary, with the header's span and device-index helpers; host tests cover allocation, boundary skipping and exhaustion | done |
| 32 | `lib/check_signature.c`: compare a BIOS-style signature with bytes read through a caller-supplied reader, with host tests | done |
| 33 | `lib/libgcc` helpers: the 64-bit shifts and compares GCC calls on 32-bit targets (`ashldi3`, `ashrdi3`, `lshrdi3`, `cmpdi2`, `ucmpdi2`), as the same halves algorithm, with host tests against the native operators | done |
| 34 | `lib/net_utils.c` and `lib/clz_tab.c`: MAC address parsing, and the byte bit-length table, each with host tests | done |
| 35 | `lib/memcat_p.c`: concatenate two pointer arrays into a caller buffer, with host tests | done |
| 36 | `lib/bust_spinlocks.c`: the nested oops counter, with the console and klogd hooks passed in, and a host test | done |
| 37 | `lib/muldi3.c`: the 64-bit multiply from 32-bit halves, with host tests against the native operators | done |
| 38 | `lib/min_heap.c` and `include/linux/min_heap.h`: a generic binary min-heap over a caller slice, with the header's algorithms and host tests for order, pop-push, delete and the empty and full cases | done |
| 39 | `lib/memregion.c` and the `ida_*` part of `lib/idr.c`: lowest-free-id allocation over a fixed bitmap, with host tests | partial |
| 40 | `lib/crypto/arc4.c` (core) and `include/crypto/arc4.h`: the ARC4 stream cipher, with published vectors and split-call tests | partial |
| 41 | `lib/crypto/chacha*` and `include/crypto/chacha.h`: the RFC 7539 ChaCha block, stream and HChaCha functions, checked against the RFC vectors and split calls | done |
| 42 | `lib/crypto/mpi/` limb helpers: `mpihelp_cmp()`, `mpihelp_add_n()` and `mpihelp_sub_n()`, checked against 128-bit arithmetic | done |
| 43 | `lib/crypto/mpi/` shifts: `mpihelp_lshift()` and `mpihelp_rshift()`, checked against 128-bit shifts | done |
| 44 | `lib/crypto/mpi/` multiply by one limb: `mpihelp_mul_1()`, `mpihelp_addmul_1()` and `mpihelp_submul_1()`, checked against limb-by-limb 128-bit arithmetic | done |
| 45 | `lib/crypto/mpi/mpi-cmp.c` and the `Mpi` object: signed comparison over a borrowed limb view, with normalisation and host tests following each branch | partial |
| 46 | `lib/crypto/hash_info.c` and `include/uapi/linux/hash_info.h`: the hash algorithm enum, names and digest sizes, with a test pinning each to the C table | done |
| 47 | `lib/crypto/mpi/mpi-sub-ui.c`: `w = u - v` for an MPI and a limb, with fixed storage and a signed-arithmetic test over edge values | done |
| 48 | `lib/crypto/mpi/mpi-add.c`: signed `mpi_add()` and `mpi_sub()` over fixed storage, checked against signed 128-bit arithmetic over every one-limb sign and value combination | done |
| 49 | `lib/crypto/mpi/mpi-bit.c`: bit count, bit test, bit set and right shift over fixed MPI storage, with u128 tests | partial |
| 50 | `lib/crypto/mpi/mpih-mul.c` (basecase): `mul_n_basecase()` and `mpih_sqr_n_basecase()`, checked against a u128 column reference for sizes 1 to 6 | partial |
| 51 | `include/linux/percpu-defs.h`: a per-CPU slot array (`PerCpu<T, N>`) with the this_cpu access rule, with host tests for independence and bounds | partial |
| 52 | `lib/irq_regs.c` and `include/asm-generic/irq_regs.h`: the per-CPU interrupt frame, using `PerCpu` from rinux-lib; the kernel builds for x86_64-unknown-none | done |
| 53 | `lib/lockref.c` and `include/linux/lockref.h`: reference counting with the lock and count in one word, the cmpxchg fast path and the locked fallback, with fast-path, dead-object and multi-thread tests | done |
| 54 | `lib/lwq.c` and `include/linux/lwq.h`: a lock-free producer queue with a spinlocked consumer side, tested with four producers and a consumer on threads | partial |
| 55 | `lib/dec_and_lock.c`: `atomic_dec_and_lock()`, dropping without the lock unless the count may reach zero, with a thread test that exactly one drop wins | partial |
| 56 | `lib/crypto/utils.c` and `lib/crypto/memneq.c`: byte-wise XOR helpers and the constant-time inequality test, with tests at every length and position | done |
| 57 | `lib/crypto/blake2s.c` and `include/crypto/blake2s.h`: unkeyed BLAKE2s, with the RFC 7693 empty and abc vectors and block-edge split updates | partial |
| 58 | `lib/crypto/blake2b.c` and `include/crypto/blake2b.h`: unkeyed BLAKE2b-512, with the RFC 7693 vectors and block-edge split updates | partial |
| 60 | `lib/crypto/poly1305.c` and `poly1305-donna64.c`: the Poly1305 authenticator with its 44-bit limb core, checked against the RFC 8439 vector and the empty message | done |
| 61 | `lib/crypto/nh.c` and `include/crypto/nh.h`: the NH almost-universal hash over 16-byte units, checked against hand-computed single-unit and second-unit cases, plus length rules | done |
| 62 | `lib/crypto/sm3.c`: the SM3 hash, checked against the two GB/T 32905 examples and split updates | done |
| 63 | `crypto/sm4.c` (core) and `include/crypto/sm4.h`: the SM4 block cipher with its key schedule, checked against the GB/T 32907 example and a decryption round trip | partial |
| 64 | `lib/math/prime_numbers.c`: the prime sieve over a caller bitmap, checked against trial division inside its range and walked for next_prime; growth and RCU wait | partial |
| 65 | `lib/earlycpio.c`: `find_cpio_data()` for newc archives in memory, with tests over archives built in the test (lookup, next offset, missing, bad magic, directory) | done |
| 66 | `lib/cmpxchg-emu.c`: the one-byte compare-and-swap built on a 32-bit word, with tests for position, mismatch and concurrent neighbouring bytes | done |
| 67 | `lib/bucket_locks.c`: the hashed spinlock array with its sizing rule and mask, with tests for sizing, mask selection and mutual exclusion in one bucket | done |
| 68 | `include/linux/rbtree_augmented.h` (kernel side of augmentation): the `RbAugment` hooks, called from insertion rotations and erase exactly where C calls them; host test keeps a subtree maximum through random inserts and erases and checks it after every operation | done |
| 69 | `lib/interval_tree.c` and `include/linux/interval_tree_generic.h`: interval trees over the augmented rbtree, checked against brute-force overlap over random inserts, removals and queries; the span iterator is not ported | partial |
| 70 | `lib/strncpy_from_user.c`: copy a string from user memory bounded by a count, over a reader that reports faults, with tests for the NUL, count, empty and fault cases | partial |
| 71 | `lib/strnlen_user.c`: the bounded user-string length with its three return values, with tests for length, too-long and fault cases | partial |
| 72 | `lib/usercopy.c`: `check_zeroed_user()` with the early exit and the fault result, with tests; the copy wrappers wait for the user-access layer | partial |
| 73 | `lib/stmp_device.c` and `include/linux/stmp_device.h`: the i.MX/STMP block reset sequence over a register trait, with a simulated block that gates its clock on soft reset, and a stuck-reset timeout test | done |
| 74 | `lib/iomap_copy.c`: MMIO block copies of 32- and 64-bit words through register closures, with tests for order, read-back and empty copies | done |
| 75 | `lib/iomem_copy.c`: I/O memory fill and copies as byte operations over closures, with tests for fill, round trip and empty ranges | done |
| 76 | `lib/extable.c`: the exception table sort and binary search over absolute addresses, with tests for lookup hits and misses and a shuffled 1000-entry table | partial |
| 77 | `lib/decompress.c`: the compression magic table and `decompress_method()`, with tests for each magic and short or unknown input; the decompressors wait | partial |
| 78 | `kernel/sched/fair.c` (weights and vruntime delta) and the CFS weight tables from `kernel/sched/core.c`: exact-arithmetic tests for the tables, the inverse, scaling and the nice-0 identity; the runqueue logic waits for the scheduler | partial |
| 79 | `kernel/sched/fair.c` (EEVDF accounting): `avg_vruntime()`, `vruntime_eligible()` and `update_deadline()` over the weighted accumulators, checked against exact rational arithmetic over random entities; the zero_vruntime update and the runqueue wait for the scheduler | partial |
| 80 | `kernel/sched/sched.h` (`rt_prio_array`): the real-time priority array, with tests for pick order, FIFO within a priority, middle removal and the bitmap | partial |
| 81 | `kernel/sched/wait.c`: wait queue placement, removal and the wake walk with exclusive counting, with tests for head and tail order, the count limit, a stop, and removal; the locking and sleep paths wait | partial |
| 82 | `kernel/sched/pelt.c` and `kernel/sched/sched-pelt.h`: PELT decay and segment accumulation with the copied table, checked against the real exponential, the half-life per period, and the cutoff; the per-entity averages wait | partial |
| 83 | `kernel/sched/loadavg.c` and `include/linux/sched/loadavg.h`: the load-average fixed-point decay, the power and the avenrun update, with tests against the real exponential, convergence, clamping and the fraction format | partial |
| 84 | `kernel/sched/cputime.c` (`cputime_adjust`): the monotonic split of runtime into user and system time, with tests that the parts sum to the total, follow the tick ratio, never go backwards over 500 samples, and ignore a non-monotonic total | partial |
| 85 | `kernel/sched/cpudeadline.c`: the per-CPU max-heap of deadlines, set, clear, root and find, checked against a brute-force maximum over 2000 random updates | partial |
| 86 | `kernel/sched/deadline.c` (CBS rules): budget replenishment by whole periods, the new-period reset, and the bandwidth overflow test, with tests for fresh, overrun, lagging and over- and under-bandwidth cases | partial |
| 87 | `kernel/sched/sched.h` (uclamp buckets): the clamp-value to bucket mapping with its rounded width, tests for the edges, the top bucket and monotonicity | done |
| 88 | `include/linux/sched/prio.h` and `kernel/sched/syscalls.c` (`__normal_prio`): the priority ranges, nice conversions and the normal priority from policy, with tests for ranges, nice, real-time and deadline ordering | done |
| 89 | `kernel/sched/completion.c` (non-blocking part): counting completions, complete_all staying in force, and try-wait, with tests including concurrent completers; blocking waits wait for the scheduler | partial |
| 90 | `kernel/sched/cpupri.c` and `cpupri.h`: the level conversion, per-CPU level set, and the find over lower levels without a fitness function, with tests for the special levels, reversal, monotonicity, moving CPUs between levels, and the allowed-CPU choice | partial |
| 92 | `lib/ucs2_string.c`: UCS-2 length, sized size, `strscpy`, `strncmp` and the UTF-8 size and conversion over `u16` slices, with host tests for each rule in the C comments (Linux has no KUnit file for it) | done |
| 93 | `lib/ts_fsm.c`: the naive finite-state text search over a token chain (literal and ctype classes; single, optional, zero-or-more and one-or-more repeats; strict and head-ignoring starts), with the `fsm_init()` checks; the `textsearch` framework and its fragmented-buffer callback are dropped, so the search takes one `&[u8]`, with host tests for each repeat kind | done |
| 94 | `lib/bitmap-str.c` (list format): `bitmap_parselist()` with its region parser, the `all` and `N` tokens, the strided `range:used/group` form, the hex chunk parser `bitmap_parse()`, and the C error codes, with host tests on the C comments' examples; the hex and list printers `bitmap_print_bitmask_to_buf()` and `bitmap_print_list_to_buf()` (with the same offset and count window); the user-copy wrappers are not ported yet | partial |
| 95 | `lib/string_helpers.c`: `string_get_size()` (Napier reduction and the SI and binary units), `string_unescape()` and `string_escape_mem()` with the C class flags and rule order, with host tests for each; `strim()`, `sysfs_streq()`, `match_string()`, `sysfs_match_string()` and `strreplace()` (`strim()` returns the trimmed view rather than writing a NUL); the allocating helpers, `parse_int_array()` and the fortify hooks are not ported yet | partial |
| 96 | `lib/textsearch.c`: the algorithm lookup by name (`kmp`, `bm`, `fsm`) and the prepare and find front end, with `TS_IGNORECASE` and the FSM token path. The runtime registry, its lock and RCU walk, and the module refcounts are replaced by a static table, since the algorithms are part of this crate; the `get_next_block` callback is replaced by a single slice | partial |
| 97 | `lib/dynamic_queue_limits.c` and the inline helpers in `include/linux/dynamic_queue_limits.h`: the byte-queue limit (`queued`, `avail`, `completed`, `reset`, `new`), with the starved and busy paths, the bounds, the object limit, and the BUG_ON; time is passed in as `now`. The stall detector is not ported yet (`stall_thrs` stays 0) | partial |
| 98 | `lib/klist.c` and `include/linux/klist.h`: the klist list of reference-counted nodes, with add head/tail/before/behind, `klist_del` that unlinks only at zero references, and the iterator that holds a reference on its current node; nodes are index-linked in a caller slice; the blocking `klist_remove()` and the `get`/`put` callbacks are not ported | partial |
| 99 | `lib/test_ida.c` (the parts that fit a fixed bitmap): the straightforward alloc checks, leaf fill-and-free, exceptional-entry conversion, bad frees, and the existence checks; `ida_check_max()` (ids near 2^31), `ida_destroy()`, `ida_is_empty()` and the tree-based `find_first_range()` wait for an IDA that grows | partial |
| 100 | `lib/test_memcat_p.c`: the KUnit-style check of `memcat_p()` ported in full: 127-element arrays, the total, the magic, the length, and the order; the arrays are slices, so no NULL terminator | done |
| 101 | `lib/test_bitops.c`: the `get_count_order()` and `get_count_order_long()` tables and the set/clear round trip checked with `find_first_bit`; the `fns()` timing loop is a benchmark and is not ported | done |
| 102 | `lib/is_single_threaded.c`: `current_is_single_threaded()` over a slice of task records in list order, with the thread-group walk, the mm-sharing check and the kernel-thread skip; the RCU walk and `task_struct` are modelled by the caller | partial |
| 103 | `lib/math/test_div64.c`: the 12 x 12 table of 64-by-32 quotients and remainders, run through `do_div()` and the generic `__div64_32()`; the constant-divisor path and the 1024-iteration timing loop are not ported | done |
| 104 | `include/linux/bitfield.h`: the `FIELD_PREP`/`FIELD_GET`/`FIELD_MAX`/`FIELD_FIT`/`FIELD_MODIFY`/`FIELD_GET_SIGNED` helpers over a `Field` trait for `u8`..`u64`, `field_multiplier()`/`field_mask()`, and the generated `u8_encode_bits()` .. `be64_get_bits()` family (the `le`/`be` forms work on the stored representation); the compile-time mask and overflow checks become a bad-mask panic and run-time masking. `lib/tests/bitfield_kunit.c` ported in full | done |
| 105 | `include/linux/compiler.h` (`is_signed_type()`) as the `IsSignedType` trait: the integer types, `bool`, `char` and raw pointers are implemented here, and user enums implement it themselves; `lib/tests/is_signed_type_kunit.c` ported in full | done |
| 106 | `include/linux/util_macros.h` (`find_closest()`, `find_closest_descending()`) as slice functions that take the array and its length in one value, with the midpoint tie rule kept; `lib/tests/util_macros_kunit.c` ported in full, every range of both tables | done |
| 107 | `lib/list_debug.c`: the `list_add` and `list_del` consistency checks, each returning the first failed check as a `ListCorruption` (in C order) with the poison values; the printk and stack dump are left to the caller; host tests cover each check and the valid cases | done |
| 108 | `lib/debug_locks.c`: the global `debug_locks` and `debug_locks_silent` switches as a `DebugLocks` value (with `DEBUG_LOCKS` as the kernel instance); `debug_locks_off()` returns true only for the first switch-off that is not silent, runs the console action passed in, and a silent switch-off still turns debugging off; host tests cover each case | done |
| 109 | `lib/atomic64.c`: the 64-bit atomics (`read`, `set`, the add/sub/and/or/xor forms in fetch and return variants, `dec_if_positive`, `cmpxchg`, `xchg`, `fetch_add_unless`) over `AtomicI64` with no lock table, since Rust atomics need no spinlock fallback; host tests cover each op and a 4-thread counter | done |
| 110 | `lib/dim/dim.c`, `lib/dim/rdma_dim.c` and the DIM helpers of `include/linux/dim.h`: the sample arithmetic (`dim_calc_stats()`, `BIT_GAP`, `IS_SIGNIFICANT_DIFF`), the park/turn walk, and the RDMA moderation state machine (`rdma_dim()`), with hand-computed windows; the consumer's `schedule_work()` is the returned flag, and `lib/dim/net_dim.c` waits | partial |
| 111 | `lib/zstd/common/error_private.c` and the enum in `include/linux/zstd_errors.h`: the error code list, `ZSTD_isError()`, `ZSTD_getErrorCode()` and the one message table, with host tests for each message, the default and the error range; `lib/tests/test_bits.c`: the `BIT_*()` and `GENMASK*()` checks, with the typed and 128-bit cases; the compile-fail cases and `GENMASK_INPUT_CHECK()` are not ported | done |
| 112 | `lib/raid/raid6/mktables.c`, `lib/raid/raid6/int.uc` and `lib/raid/raid6/recov.c`: the GF(2^8) tables as const items, the P/Q syndrome and its partial update, and the two-disk and one-disk recovery (byte-wise, not word-wise); `lib/raid/xor/xor-8regs.c`: the 2- to 5-source XOR over eight-word groups. The SIMD variants, `algos.c` and `xor-8regs-prefetch.c` are not ported | done |
| 113 | `lib/fw_table.c` and `include/linux/fw_table.h`: the generic subtable walker over a byte slice (`acpi_parse_entries_array()`) for the common ACPI subtable header and the CDAT layout, and `cdat_table_parse()`, with host tests for the CDAT and ACPI-style walks, the entry, handler and length rules; the HMAT, PRMT and CEDT layouts are not ported and return `EOPNOTSUPP`, and the handler gets the entry bytes rather than the table end | partial |
| 114 | `lib/rcuref.c` and `include/linux/rcuref.h`: the RCU-safe reference count with its unsigned zones (ONEREF, SATURATED, RELEASED, DEAD, NOREF), the get and put fast paths and their slow paths, with the zone transitions checked one by one and a threaded balance test; the preemption guard in `rcuref_put()` and the `WARN_ONCE()` calls are not reproduced | done |
| 115 | the allocating helpers behind an `alloc` cargo feature (`lib/src/alloc_helpers.rs`): `kasprintf()`, `match_strdup()` and `bitmap_alloc()`/`bitmap_zalloc()`, with the lib crate still allocation-free by default; this unblocks the allocator-blocked rows, and `devm_*()` and the user-copy wrappers remain | done |
| 116 | `lib/src/printk.rs`: the log levels and a `Sink` that lib functions print through, one line at a time, so the lib needs no global state; the kernel passes a sink that calls its `printk()`. `print_hex_dump()` uses it, with the address, offset and none prefixes and the line limit; host tests check the rows, prefixes, levels and truncation | done |
| 117 | `lib/src/uaccess.rs`: the `UserMemory` trait (`copy_from_user()` and `copy_to_user()` returning the bytes not copied), `FlatUser` as a flat user address space, and the byte-wise `strncpy_from_user_mem()`, `strnlen_user_mem()` and `check_zeroed_user_mem()`; `kstrto*_from_user()` for the 64- and 32-bit integers and the boolean; the kernel implements `UserMemory` over the task's address space with the fault fixup | done |
| 118 | `lib/kstrtox.c`: the 16- and 8-bit `kstrtou16_from_user()`, `kstrtos16_from_user()`, `kstrtou8_from_user()` and `kstrtos8_from_user()`, with the copy sized as in C and tests for the range and fault rules | done |
| 119 | `lib/bitmap-str.c`: `bitmap_parse_user()` and `bitmap_parselist_user()` over `UserMemory`, with the `memdup_user_nul()` copy in `alloc_helpers` behind the `alloc` feature; host tests cover the hex and list formats, the copy and fault rules, and a bad string | done |
| 120 | `lib/src/random.rs`: the `RandomSource` trait with `get_random_u32()`, `get_random_bytes()` and `get_random_u32_below()` as in C (the multiply, the high word, and the rejection threshold), implemented for the seeded `RndState`; `find_random_bit()` and the random UUID and GUID generators draw from it | done |
| 121 | `lib/src/task_block.rs`: the `TaskBlocker` trait (`block()` and `wake()` on task ids, as `schedule()` and `try_to_wake_up()`), with the sleep and wake paths of `wait.c` (`wait_event()`, `wake_up()`) and of `completion.c` (`wait_for_completion()`, `complete_wake()`) over it; host tests use a double that completes while the task sleeps | done |
| 122 | `lib/src/printk.rs` and `lib/src/ratelimit.rs`: the ratelimit "N callbacks suppressed" line through the sink, at `LOGLEVEL_WARNING` when a window that suppressed events ends, and `seq_buf_do_printk()` as a sink call per line; host tests cover the report timing, the empty case and the line split | done |
| 123 | `lib/string_helpers.c`: `parse_int_array()` over `get_options()`, returning the count and the integers as C's array does, behind the `alloc` feature; host tests cover the comma list, a negative value and the no-integer `ENOENT` case. The row for this file was stale: `string_get_size()` and the rest were already ported | done |
| 124 | `lib/string_helpers.c`: `kstrdup_quotable()` over `string_escape_mem()` with `ESCAPE_HEX` and C's escape set, behind `alloc`; the escaped bytes are sized in a first pass, as `kasprintf()` does; host tests cover plain text, the hex-escaped set, bytes outside the set, the NUL end and the missing source | done |
| 125 | `lib/genalloc.c`: `gen_pool_has_addr()` on a single-chunk pool, with the inclusive last-byte check; host tests cover ranges inside, leaving either end, and empty ranges and pools | done |
| 126 | `lib/genalloc.c`: `gen_pool_virt_to_phys()` on the single-chunk pool, with the physical base set by `with_phys_addr()`; `None` where C returns `-1`; host tests cover the offset, addresses outside the chunk and the zero default | done |
| 127 | `lib/genalloc.c`: `GenPoolList`, the multi-chunk pool with `gen_pool_add()`, newest chunk first for alloc, free to the chunk that holds the range, and `avail`, `size`, `has_addr` and `virt_to_phys` across chunks; chunk slots are fixed at compile time so `add` past N is `ENOSPC`; host tests cover the search order, the free target, per-chunk physical addresses and the slot limit | done |
| 128 | `lib/genalloc.c`: `gen_pool_best_fit()` as `GenAlgo::BestFit` with `alloc_with()`, which takes the smallest free run that fits and gives `None` when none does; first-fit stays the plain `alloc()`; host tests build a fragmented pool where the two policies pick different regions | done |
| 129 | `lib/genalloc.c`: `gen_pool_first_fit_align()` as `GenAlgo::FirstFitAlign`, aligning the absolute address through the chunk's start, with a non-power-of-two alignment refused; host tests cover an aligned start after a used byte and an unaligned pool start | done |
| 130 | `lib/genalloc.c`: `gen_pool_fixed_alloc()` as `GenAlgo::Fixed`, which takes the region only at its offset, refusing an offset that is not a whole unit, one past the chunk, or one that is already taken; host tests cover each | done |
| 131 | `lib/genalloc.c`: `gen_pool_first_fit_order_align()` as `GenAlgo::OrderAlign`, which aligns the unit index to the request rounded up to a power of two; host tests cover the three-unit case, a one-unit request and an aligned run that does not fit | done |
| 132 | `lib/genalloc.c`: the growable pool, `GenPoolVec` behind `alloc`, where `gen_pool_add()` never runs out of chunks and allocation, free, `avail`, `size`, `has_addr` and `virt_to_phys` work across them newest first; host tests add more chunks than the fixed pool holds, and check allocation order, free and the policies across chunks | done |
| 133 | kernel wiring of the lib hooks: `PoolRandom` in `drivers/char/random.rs` implements `RandomSource` over the locked pool, and `lib_line()` in `kernel/printk.rs` is the line sink for the lib's printing functions, emitting through `vprintk_emit()` with the newline added; the kernel crate builds for `x86_64-unknown-none`. Nothing in the kernel calls these yet, and the kernel's test harness is off, so this is verified by the build only | done |
| 134 | kernel wiring of `UserMemory`: `UserSpace` in `arch/x86/mm/user.rs` copies to and from a task's user pages by walking its page tables one page at a time. Each page is checked for present, user and, for writes, writable bits before the frame is touched, so a bad user pointer gives a short copy and never a kernel page fault; the lower-half limit is enforced. Nothing calls it yet, and the kernel's test harness is off, so this is verified by the build only | done |
| 135 | kernel wiring of `TaskBlocker`: `RunqueueBlocker` in `kernel/sched/core.rs` puts the running task to sleep (`block()` sets it interruptible, dequeues it and schedules) and makes a sleeper runnable again (`wake()` enqueues it). A wake that arrives before the sleep is kept as `wakeup_pending` on the task, so the block that follows returns at once, which closes the lost-wakeup window. The task's id is its `TaskStruct` address. Nothing calls it yet, and the kernel's test harness is off, so this is verified by the build only | done |
| 136 | kernel `nanosleep` now sleeps the task: `sleep_until()` puts it in a sleep table and blocks it with `RunqueueBlocker`, and the tick (`wake_sleepers()`) wakes each task whose deadline has passed, at most 32 per tick. `wake()` also sets `need_resched`, so a woken task runs at once. Verified in QEMU: the init program's 150 ms nanosleep measures 150000 µs with the other self-tests passing | done |
| 137 | the lib hooks wired into the kernel and checked in QEMU: `PoolRandom` draws a random UUID at boot (`check_lib_hooks`), the printk sink carries a `print_hex_dump()` line to the log, `UserSpace` replaces the raw timespec read of `nanosleep` (a bad pointer is `EFAULT`, checked by init), and `nanosleep` sleeps through `RunqueueBlocker` | done |
| 138 | libfdt, read-only half: `scripts/dtc/libfdt/fdt.c` (header checks, the structure-block walk, `fdt_next_node`, the subnode and string helpers, `fdt_move`), `fdt_ro.c` (properties, names, paths, aliases, phandles, memory reservations, string lists, `get_path`, depth lookups, compatible search) and `fdt_strerror.c`, in `lib/src/libfdt.rs`; host tests build a version 17 blob in `lib/tests/libfdt_test.rs`. The write side (`fdt_rw.c`, `fdt_sw.c`, `fdt_wip.c`, `fdt_addresses.c`, `fdt_empty_tree.c`) and the `lib/fdt*.c` wrappers are not ported yet | done |
| 139 | libfdt, write side: `scripts/dtc/libfdt/fdt_rw.c` (the in-place editor `FdtMut`: reservations, names, properties, nodes, `nop_*`, `pack`, and `open_into`), `fdt_sw.c` (the sequential builder `FdtSw` and `sw_resize`), `fdt_wip.c` (in-place value edits and NOPs), `fdt_addresses.c` (`#address-cells`/`#size-cells` and `appendprop_addrrange`) and `fdt_empty_tree.c` (`create_empty_tree`), in `lib/src/libfdt.rs`; host tests in `lib/tests/libfdt_rw_test.rs`. The C in-place forms (`open_into` and `sw_resize` into the same buffer) are split into separate source and destination buffers, and a freshly built tree is never repacked | done |
| 140 | `lib/crypto/poly1305-donna32.c`: the 26-bit limb Poly1305 core (`poly1305_core_setkey()`, `poly1305_core_blocks()`, `poly1305_core_emit()`) with the one-shot tag built from them, checked against the RFC 8439 vector, the empty message, and the 44-bit core for every length from 0 to 300 bytes | done |
| 141 | `kernel/time/` pure conversions: `timeconv.c` (`time64_to_tm()`, the Neri and Schneider calendar algorithm) and `timecounter.c` with `cyclecounter_cyc2ns()` (the cycle-to-nanosecond scale with its fractional carry and the masked wrap), checked against known dates, a round trip through `mktime64()` from 1901 to 2096, and a counter that wraps; the rest of `kernel/time/` (timers, clock events, tick, NTP, posix clocks) depends on the scheduler and interrupts and waits | done |
| 142 | `kernel/time/sleep_timeout.c`: `schedule_timeout()` over the step 136 sleep table (`sleep_until()`), with the `MAX_SCHEDULE_TIMEOUT` and negative-timeout cases and the remaining-jiffies return, plus `msleep()` with `msecs_to_jiffies()`; checked in QEMU by a kernel thread that asserts a 20-jiffy sleep and a 30 ms `msleep` each run at least their length. The `schedule_timeout_interruptible`, `_killable` and `_idle` variants and the hrtimer-range forms wait for signals and hrtimers | done |
| 143 | `kernel/time/hrtimer.c`, the timer queue core, in `lib/src/hrtimer.rs`: `hrtimer_forward()` (the overrun count and the next expiry for a periodic timer, with the clamp to resolution and the queued check), and the queue operations of `enqueue_hrtimer()`, `remove_hrtimer()`, `__hrtimer_start_range_ns()` and `__run_hrtimer()` over one base: a fixed-capacity expiry-ordered queue with restart, cancel, next expiry and pop-expired, with host tests for ordering, ties, restart, cancel and capacity. The per-CPU bases, clock-event reprogramming, the softirq and SMP paths, and wiring the queue into the tick are not ported yet | partial |
| 144 | the hrtimer queue wired into the kernel: `kernel/src/kernel/time/hrtimer.rs` keeps the sleepers in a `TimerQueue`, `sys_nanosleep` arms its deadline in nanoseconds of `local_clock()` (no more rounding up to a jiffy), and `scheduler_tick()` wakes each due sleeper in deadline order (`run_expired`). Checked in QEMU: a 2.5 ms high-resolution sleep returns in 2.9 ms, and the user-space nanosleep test still passes. Periodic timers, `hrtimer_forward` in the kernel, and per-CPU bases are not wired yet | done |
| 145 | `kernel/time/time_test.c`: the KUnit date-range test ported in full, `time64_to_tm()` checked against a day-by-day calendar over 80000 years (`lib/tests/time_test.rs`) | done |
| 146 | `kernel/time/timer.c`, the wheel's bucket arithmetic in `lib/src/timer_wheel.rs`: the `LVL_*` and `WHEEL_*` constants for `HZ` 1000 (9 levels, 64 buckets), `calc_index()` and `calc_wheel_index()`, with host tests that a bucket never fires before its timer, that short timers land in level 0 and long ones in the outer levels, and that obscene timeouts are clamped. The wheel's queues, the per-CPU bases, the sysctl, the NO_HZ migration and the expiry processing of `timer.c` are not ported | partial |
| 147 | `kernel/time/ntp.c`, the frequency discipline in `lib/src/ntp.rs`: `ntp_update_frequency()`, `ntp_update_offset()` and `ntp_update_offset_fll()` over `NtpData`, with the PLL and FLL update, the clamps on phase, interval and frequency, the FREQHOLD and nanosecond cases, and the FLL mode switch, with host tests. The per-timekeeper array, `second_overflow()`, the drain and skew paths, the RTC sync, the PPS discipline and `adjtimex(2)` are not ported | partial |
| 148 | `kernel/time/namespace.c` and the offset helpers of `include/linux/time_namespace.h`, in `lib/src/time_namespace.rs`: `timespec64_add()`, `timespec64_to_ktime()`, `timens_add_monotonic()`, `timens_add_boottime()` and `do_timens_ktime_to_host()`, with host tests for the carry, the offset shift, the already-passed case and the unshifted clocks. The namespace objects, refcounts, `/proc` offsets and the vDSO page are not ported | partial |
| 149 | `kernel/time/clockevents.c`, the programming core in `lib/src/clockevents.rs`: `cev_delta2ns()` and `clockevent_delta2ns()`, the minimum-delta retry with its growth to `MIN_DELTA_LIMIT` (`clockevents_program_min_delta()`, the MIN_ADJUST variant that x86 selects), and `clockevents_program_event()` for a device not coupled to a clocksource, over a `SetNextEvent` trait for the hardware. Host tests cover the conversion and its clamps, the past and future expiries, forced minimum programming, the retry growth and its limit, and shutdown. Not yet wired to a real device: the PIT and TSC drivers still use periodic mode, so the boot path is unchanged. The registry, sysfs, state machine beyond shutdown and the coupled-device paths are not ported | partial |
| 150 | the PIT as a clock event device: `pit_set_oneshot()`, `pit_read_count()` and `PitOneshot`, which implements the clockevents `SetNextEvent` trait, in `kernel/src/drivers/clocksource/i8253.rs`. The kernel's clockevents core programs a 10 ms expiry through it, and the QEMU selftest reads the latched count back as counting down from the armed value (armed 11931 ticks, read back 11814). The periodic tick is restored straight after, so the boot path is unchanged. The tick itself is not yet driven by the clock event device | partial |
| 151 | the tick driven by the clock event device: `arch/x86/kernel/time.rs` arms the PIT in one-shot mode through `ClockEventDevice::program_event`, and each IRQ0 re-arms the next tick. The count read after the IRQ gives the time since the tick fired, so the next one-shot is shortened by that amount and the ticks stay on schedule with no drift. A late tick is clamped to half a period so the device never takes its forced path, which stalled the QEMU boot in the unclamped version. QEMU checks: 100 ticks in about 100 ms, the sleep and nanosleep checks, the user-space test, and the boot-test passes on repeated runs. The periodic `pit_set_periodic()` is no longer used | done |
| 152 | the one-shot tick's stall, root cause: a forced (minimum-delta) event that has fired leaves `next_event_forced` set, so the next `program_event()` with a near deadline returns early and arms nothing. `ClockEventDevice::event_fired()` clears the flag, and the tick handler calls it before re-arming. The clamp from step 151 is removed. Host tests: a PIT model with late handlers (20,000 steps) never runs out of pending events, and a test pins the stale-flag failure. QEMU: the boot passes three runs in a row with the unclamped tick | done |
| 138+ | user-space socket syscalls, more `drivers/` (AHCI, input, console), and `ipc/`, the rest of `crypto/`, `security/` | |

## How a step is done

1. Read the C sources and their tests in `../linux`.
2. Port each file to the matching place in `rinux/` and name the C source in
   the module docs. Keep the SPDX license of the original.
3. Port the kernel's tests for those files into `lib/tests/` (named after
   the C test), keeping every table entry. Add tests for code Linux leaves
   untested.
4. Run `cargo test`, `cargo clippy --all-targets` and `cargo fmt --check`.
5. Add rows to the table below and run `scripts/progress.py`.

Data that lives in C as literal tables (errno numbers, CRC tables, ...) is
extracted by `scripts/gen_tables.py` instead of being retyped.

## Conventions

- C strings are byte slices ending at the first NUL or the end of the slice.
- `-Exxx` returns become `Result<T, Errno>`.
- Pointers into a caller's buffer become indices.
- C `unsigned long`/`long` are `u64`/`i64`: rinux targets 64-bit machines.
- Where C has undefined behaviour (shift by 64, `int` overflow), the Rust
  code picks the x86 result or the intended one and says which.
- Performance-only code paths (word-at-a-time string loops) are not ported
  until profiling asks for them; results are identical.
- The kernel (`kernel/`) is booted by GRUB's `multiboot2` command rather
  than the Linux x86 boot protocol, so one image boots under BIOS and UEFI
  without a real-mode setup stub. The multiboot2 boot information stands in
  for `boot_params`.
- The kernel's module paths follow the Linux tree: `arch::x86::kernel::setup`
  is `arch/x86/kernel/setup.c`.

From step 5 the kernel links `alloc`, so `Box`, `Vec` and `String` work;
they allocate through `kmalloc` via the global allocator in
`kernel/src/mm/rust_alloc.rs`.

## Building and booting the kernel

The kernel is its own Cargo workspace in `kernel/`, built for
`x86_64-unknown-none` in WSL. From the repository root:

```bash
bash rinux/scripts/build-iso.sh
bash rinux/scripts/boot-test.sh
bash rinux/scripts/boot-test.sh --uefi
```

`build-iso.sh` writes `rinux-boot.iso` next to `linux-boot.iso`;
`CMDLINE=...` overrides the kernel command line. It also builds a small
`newc` cpio initramfs (with `cpio`) and loads it as a GRUB `module2`, which
the kernel unpacks into the rootfs at boot; `INITRAMFS=path.cpio` supplies
your own instead. (In step 8 this becomes the BusyBox userland.) `boot-test.sh` boots the
ISO in QEMU without a display, prints the serial log, saves a screenshot
of the VGA screen, and passes once the kernel reaches the end of its
ported boot path (`EXPECT=...` looks for other text).

The boot self-tests (`kernel/src/rinux/selftest.rs`) exercise the timer,
the memory allocators and, from step 6, the scheduler: three kernel threads
that yield cooperatively, and two that never yield and so must be
interleaved by timer preemption.

The crash paths are tested with the rinux-only `rinux.crashtest=` boot
parameter (`divide_error`, `invalid_op`, `page_fault`,
`general_protection`, `int3`, `bug`, `panic`), standing in for LKDTM
until there is a debugfs:

```bash
CMDLINE="earlyprintk=ttyS0 rinux.crashtest=page_fault" OUT=/tmp/crash.iso bash rinux/scripts/build-iso.sh
ISO=/tmp/crash.iso EXPECT="end Kernel panic" bash rinux/scripts/boot-test.sh
```

Under QEMU without KVM the emulated PIT is noisy, and both TSC
calibrations sometimes fail; rinux then uses jiffies for `sched_clock()`
and calibrates the delay loop by timing it, as Linux does without an HPET
or PM timer to cross-check against.

Header constants (`processor-flags.h`, `msr-index.h`, `serial_reg.h`,
`segment.h`, `trapnr.h`, `irq_vectors.h`) are generated by
`scripts/gen_consts.py`, which lists what it can't convert at the top of
each output file.

The kernel parses its command line with rinux-lib's ports of
`lib/cmdline.c` (`next_arg()`, `get_option()`) and `lib/vsprintf.c`
(`simple_strtoull()`), as Linux does.

## Files

Status is `done` (everything ported), `partial` (what is missing is listed)
or `test` (a C test file ported to Rust tests).

| Linux file | Rust | Status | Notes |
|---|---|---|---|
| `include/uapi/asm-generic/errno-base.h` | `lib/src/errno.rs` | done | generated table |
| `include/uapi/asm-generic/errno.h` | `lib/src/errno.rs` | done | generated table |
| `include/linux/errno.h` | `lib/src/errno.rs` | done | generated table |
| `include/linux/err.h` | `lib/src/errno.rs` | partial | `MAX_ERRNO`, `IS_ERR_VALUE`; `ERR_PTR` family is replaced by `Result` |
| `include/asm-generic/bug.h` | `lib/src/bug.rs` | partial | `WARN_ON`, `WARN_ON_ONCE`, `BUG_ON`; backtraces wait for printk |
| `lib/ctype.c` | `lib/src/ctype.rs` | done | |
| `include/linux/ctype.h` | `lib/src/ctype.rs` | done | |
| `lib/string.c` | `lib/src/string.rs` | done | `memcpy`/`memmove`/`memset` come from `core` |
| `include/linux/string.h` | `lib/src/string.rs` | partial | inline helpers; declarations of `string_helpers.c` functions pending |
| `lib/string_helpers.c` | `lib/src/string_helpers.rs` | done | `string_get_size()`, `string_unescape()`, `string_escape_mem()`, `strim()`, `sysfs_streq()`, `match_string()`, `sysfs_match_string()`, `strreplace()`, `parse_int_array()` and `kstrdup_quotable()` (the last two behind `alloc`); `memcpy_and_pad()`'s fortify hooks are not needed; `kstrdup_quotable_cmdline()` and `kstrdup_quotable_file()` need the task and file layers |
| `include/linux/string_helpers.h` | `lib/src/string.rs` | partial | the `UNESCAPE_*` and `ESCAPE_*` flags |
| string escape/unescape tests | `lib/tests/string_escape_kunit.rs` | test | per-class checks, `only`/append, truncation, and an all-bytes escape/unescape round-trip |
| `lib/kstrtox.c` | `lib/src/kstrtox.rs` | done | every `kstrto*()` and `kstrto*_from_user()` over `UserMemory`, including the 16- and 8-bit forms and `kstrtobool` |
| `lib/kstrtox.h` | `lib/src/kstrtox.rs` | done | |
| `include/linux/kstrtox.h` | `lib/src/kstrtox.rs` | done | the `kstrto*()` and `kstrto*_from_user()` helpers as for `lib/kstrtox.c`; `simple_strto*()` are in `lib/src/vsprintf.rs` |
| `lib/sort.c` | `lib/src/sort.rs` | partial | `*_nonatomic()` wait for `cond_resched()` |
| `include/linux/sort.h` | `lib/src/sort.rs` | partial | as `lib/sort.c` |
| `lib/bsearch.c` | `lib/src/bsearch.rs` | done | |
| `lib/ts_kmp.c` | `lib/src/ts_kmp.rs` | partial | Knuth-Morris-Pratt over a linear buffer; the `textsearch` framework's `get_next_block` fragmentation and `ts_config` allocation are not ported |
| KMP search tests | `lib/tests/ts_kmp_kunit.rs` | test | specific cases, ignore-case, and a brute-force cross-check |
| `lib/ts_bm.c` | `lib/src/ts_bm.rs` | partial | Boyer-Moore (bad-character + good-suffix tables) over a linear buffer; the `textsearch` framework and `ts_config` allocation are not ported |
| Boyer-Moore search tests | `lib/tests/ts_bm_kunit.rs` | test | specific cases, ignore-case, and a cross-check against both brute force and KMP |
| `include/linux/bsearch.h` | `lib/src/bsearch.rs` | done | |
| `lib/bcd.c` | `lib/src/bcd.rs` | done | |
| `include/linux/bcd.h` | `lib/src/bcd.rs` | done | |
| `lib/hweight.c` | `lib/src/hweight.rs` | done | |
| `lib/bitrev.c` | `lib/src/bitrev.rs` | done | generated table |
| `include/linux/bitrev.h` | `lib/src/bitrev.rs` | done | |
| `include/asm-generic/bitops/__bitrev.h` | `lib/src/bitrev.rs` | done | |
| `lib/hexdump.c` | `lib/src/hexdump.rs` | done | `print_hex_dump()` over a line sink in `lib/src/printk.rs`; `DUMP_PREFIX_ADDRESS` shows the address as `0x` and 16 digits, without `%p` hashing |
| `include/linux/hex.h` | `lib/src/hexdump.rs` | done | |
| `lib/glob.c` | `lib/src/glob.rs` | done | |
| `include/linux/glob.h` | `lib/src/glob.rs` | done | |
| `lib/random32.c` | `lib/src/random32.rs` | partial | `prandom_seed_full_state()` waits for per-CPU data and the entropy pool |
| `include/linux/prandom.h` | `lib/src/random32.rs` | partial | as `lib/random32.c` |
| `lib/ratelimit.c` | `lib/src/ratelimit.rs` | done | `___ratelimit()` with the time passed in, and the "callbacks suppressed" report through a sink (`ratelimit_report()`); the SMP lock and atomics are the caller's, since the state is `&mut` |
| `include/linux/ratelimit_types.h` | `lib/src/ratelimit.rs` | partial | the `ratelimit_state` fields |
| ratelimit tests | `lib/tests/ratelimit_kunit.rs` | test | burst/suppress/reset, never-limit, always-limit and one-per-interval |
| `lib/refcount.c` | `lib/src/refcount.rs` | partial | `refcount_t` add/inc/sub_and_test/dec with saturation at `REFCOUNT_SATURATED`; the `WARN` splats become a silent clamp |
| `include/linux/refcount.h` | `lib/src/refcount.rs` | partial | the `refcount_t` ops and the `REFCOUNT_MAX`/`REFCOUNT_SATURATED` constants |
| refcount tests | `lib/tests/refcount_kunit.rs` | test | inc/dec, inc_not_zero, and the overflow/underflow/use-after-free/leak saturation paths |
| `lib/math/gcd.c` | `lib/src/math/gcd.rs` | done | |
| `include/linux/gcd.h` | `lib/src/math/gcd.rs` | done | |
| `lib/math/lcm.c` | `lib/src/math/lcm.rs` | done | |
| `include/linux/lcm.h` | `lib/src/math/lcm.rs` | done | |
| `lib/math/int_sqrt.c` | `lib/src/math/int_sqrt.rs` | done | |
| `lib/math/cordic.c` | `lib/src/math/cordic.rs` | done | fixed-point sine/cosine (`cordic_calc_iq`) |
| `include/linux/cordic.h` | `lib/src/math/cordic.rs` | done | the CORDIC constants and `cordic_iq` |
| cordic tests | `lib/tests/cordic_kunit.rs` | test | the full circle checked against `f64` sin/cos within the 18-iteration precision |
| `lib/math/int_pow.c` | `lib/src/math/int_pow.rs` | done | |
| `lib/math/int_log.c` | `lib/src/math/int_log.rs` | done | generated table |
| `include/linux/int_log.h` | `lib/src/math/int_log.rs` | done | |
| `lib/math/reciprocal_div.c` | `lib/src/math/reciprocal_div.rs` | done | |
| `include/linux/reciprocal_div.h` | `lib/src/math/reciprocal_div.rs` | done | |
| `lib/math/polynomial.c` | `lib/src/math/polynomial.rs` | done | `polynomial_calc()` over slices of `PolynomialTerm`; the wrapping arithmetic matches the kernel's x86-64 build |
| `include/linux/polynomial.h` | `lib/src/math/polynomial.rs` | done | `struct polynomial` and `struct polynomial_term` as `Polynomial` and `PolynomialTerm` |
| `lib/math/prime_numbers.c` | `lib/src/math/prime_numbers.rs` | partial | `Primes`: the Eratosthenes sieve over a caller bitmap, `is_prime()` and `next_prime()`, with the C trial-division fallbacks; the on-demand growth, RCU and mutex wait |
| `lib/math/rational.c` | `lib/src/math/rational.rs` | done | |
| `include/linux/rational.h` | `lib/src/math/rational.rs` | done | |
| `lib/math/div64.c` | `lib/src/math/div64.rs` | done | generic `mul_u64_add_u64_div_u64()` with both digit sizes |
| `include/linux/math64.h` | `lib/src/math/div64.rs` | partial | 64-bit half; the 32-bit `#else` branch is covered by the generic code |
| `include/vdso/math64.h` | `lib/src/math/div64.rs` | partial | `__iter_div_u64_rem()` |
| `lib/crc/crc32-main.c` | `lib/src/crc/mod.rs` | done | generic code; arch acceleration later |
| `lib/crc/gen_crc32table.c` | `lib/src/crc/mod.rs` | done | tables built by `const fn` |
| `include/linux/crc32.h` | `lib/src/crc/mod.rs` | done | |
| `include/linux/crc32c.h` | `lib/src/crc/mod.rs` | done | |
| `include/linux/crc32poly.h` | `lib/src/crc/mod.rs` | done | |
| `lib/crc/crc64-main.c` | `lib/src/crc/mod.rs` | done | |
| `lib/crc/gen_crc64table.c` | `lib/src/crc/mod.rs` | done | tables built by `const fn` |
| `include/linux/crc64.h` | `lib/src/crc/mod.rs` | done | |
| `lib/crc/crc16.c` | `lib/src/crc/mod.rs` | done | generated table |
| `include/linux/crc16.h` | `lib/src/crc/mod.rs` | done | |
| `lib/crc/crc-ccitt.c` | `lib/src/crc/mod.rs` | done | generated table |
| `include/linux/crc-ccitt.h` | `lib/src/crc/mod.rs` | done | |
| `lib/crc/crc-itu-t.c` | `lib/src/crc/mod.rs` | done | generated table |
| `include/linux/crc-itu-t.h` | `lib/src/crc/mod.rs` | done | |
| `lib/crc/crc-t10dif-main.c` | `lib/src/crc/mod.rs` | done | generated table |
| `include/linux/crc-t10dif.h` | `lib/src/crc/mod.rs` | done | |
| `lib/crc/crc7.c` | `lib/src/crc/mod.rs` | done | generated table |
| `include/linux/crc7.h` | `lib/src/crc/mod.rs` | done | |
| `lib/crc/crc8.c` | `lib/src/crc/mod.rs` | done | |
| `include/linux/crc8.h` | `lib/src/crc/mod.rs` | done | |
| `lib/crc/crc4.c` | `lib/src/crc/mod.rs` | done | generated table |
| `include/linux/crc4.h` | `lib/src/crc/mod.rs` | done | |
| `lib/tests/string_kunit.c` | `lib/tests/string_kunit.rs` | test | benchmarks not ported |
| `lib/test-kstrtox.c` | `lib/tests/test_kstrtox.rs` | test | all 591 cases |
| `lib/tests/test_sort.c` | `lib/tests/test_sort.rs` | test | |
| `lib/tests/glob_kunit.c` | `lib/tests/glob_kunit.rs` | test | |
| `lib/tests/random32_kunit.c` | `lib/tests/random32_kunit.rs` | test | |
| `lib/test_hexdump.c` | `lib/tests/test_hexdump.rs` | test | exhaustive instead of random |
| `lib/crc/tests/crc_kunit.c` | `lib/tests/crc_kunit.rs` | test | IRQ-context and benchmark cases need a kernel |
| `lib/math/tests/gcd_kunit.c` | `lib/tests/math_kunit.rs` | test | |
| `lib/math/tests/int_log_kunit.c` | `lib/tests/math_kunit.rs` | test | |
| `lib/math/tests/int_pow_kunit.c` | `lib/tests/math_kunit.rs` | test | |
| `lib/math/tests/int_sqrt_kunit.c` | `lib/tests/math_kunit.rs` | test | |
| `lib/math/tests/rational_kunit.c` | `lib/tests/math_kunit.rs` | test | |
| `lib/math/test_mul_u64_u64_div_u64.c` | `lib/src/math/div64.rs` | test | unit test, needs private variants |
| `lib/math/test_div64.c` | `lib/tests/test_div64.rs` | test | the full 12 x 12 table through `do_div()` and `__div64_32()` |
| `include/linux/bitfield.h` | `lib/src/bitfield.rs` | done | `FIELD_*` as generic functions over `Field`; compile-time checks become a panic on a bad mask |
| `lib/tests/bitfield_kunit.c` | `lib/tests/bitfield_kunit.rs` | test | all `CHECK_ENC_GET` and `CHECK` cases, plus `FIELD_*`, `replace_bits` and the bad-mask panic |
| `include/linux/compiler.h` | `lib/src/is_signed_type.rs` | done | ``is_signed_type``: trait over the integer, bool, char and raw-pointer types|
| `lib/tests/is_signed_type_kunit.c` | `lib/tests/is_signed_type_kunit.rs` | test | the full C type table; the enums are modelled with matching `repr` |
| `include/linux/util_macros.h` | `lib/src/util_macros.rs` | done | ``find_closest``: `find_closest()` and `find_closest_descending()` over slices|
| `lib/tests/util_macros_kunit.c` | `lib/tests/util_macros_kunit.rs` | test | every range of the ascending and descending tables |
| `lib/test_ida.c` | `lib/tests/test_ida_test.rs` | test | the parts that fit a fixed bitmap; max-id, destroy and find_first wait for a growing IDA |
| `lib/test_memcat_p.c` | `lib/tests/test_memcat_p_test.rs` | test | |
| `lib/test_bitops.c` | `lib/tests/test_bitops_test.rs` | test | `fns()` timing loop not ported |
| `lib/klist.c` | `lib/src/klist.rs` | partial | index-linked nodes; no blocking `klist_remove()` or get/put callbacks |
| `include/linux/klist.h` | `lib/src/klist.rs` | partial | |
| klist tests | `lib/tests/klist_test.rs` | test | ordering, deletion during iteration, deferred unlink |
| `lib/is_single_threaded.c` | `lib/src/is_single_threaded.rs` | partial | the walk over caller-supplied task records; no RCU or `task_struct` |
| is_single_threaded tests | `lib/tests/is_single_threaded_test.rs` | test | |
| `lib/vsprintf.c` | `lib/src/vsprintf.rs` | partial | arguments are an `Arg` slice; `%p` for resources, ranges, `%pE`, dentries, time, clocks, block devices, flags, firmware nodes and sockaddrs pending; `kasprintf()` is in `alloc_helpers` behind the `alloc` feature; `vbin_printf()`/`bstr_printf()` and the `hash_pointers` boot options pending |
| `include/linux/sprintf.h` | `lib/src/vsprintf.rs` | partial | as `lib/vsprintf.c` |
| `lib/cmdline.c` | `lib/src/cmdline.rs` | done | |
| `lib/parser.c` | `lib/src/parser.rs` | done | `match_strdup()` is in `alloc_helpers` behind the `alloc` feature |
| `include/linux/parser.h` | `lib/src/parser.rs` | done | `substring_t` is a byte slice |
| `lib/bitmap.c` | `lib/src/bitmap.rs` | partial | `bitmap_alloc()` family is in `alloc_helpers` behind the `alloc` feature; `devm_*()` wait for devres |
| `include/linux/bitmap.h` | `lib/src/bitmap.rs` | partial | as `lib/bitmap.c`; compile-time fast paths not needed |
| `lib/bitmap-str.c` | `lib/src/bitmap_str.rs` | partial | `bitmap_parse_user()` and `bitmap_parselist_user()` are in `alloc_helpers` behind the `alloc` feature; the printing `*_user()` side is not ported |
| `include/linux/bitmap-str.h` | `lib/src/bitmap.rs` | partial | as `lib/bitmap-str.c` |
| `lib/find_bit.c` | `lib/src/find_bit.rs` | done | `find_random_bit()` over a `RandomSource`; the rest of the file is ported above |
| `include/linux/find.h` | `lib/src/find_bit.rs` | done | `for_each_*` loops are iterators |
| `lib/genalloc.c` | `lib/src/genalloc.rs` | partial | `gen_pool_alloc`/`free`/`avail`/`size`/`has_addr`/`virt_to_phys` on a single chunk (`GenPool`), across up to N chunks (`GenPoolList`), and across any number of chunks (`GenPoolVec`, behind `alloc`), newest first; first-fit, best-fit, aligned, fixed-offset and order-aligned policies (`GenAlgo`); no locking, NUMA ownership or `gen_pool_dma_*` helpers |
| `include/linux/genalloc.h` | `lib/src/genalloc.rs` | partial | the pool's unit order and bitmap |
| genalloc tests | `lib/tests/genalloc_kunit.rs` | test | alloc/free/reuse, exhaustion, and a no-overlap check |
| `include/linux/bitops.h` | `lib/src/bitops.rs` | partial | atomic bitops, `set_mask_bits()` and `bit_clear_unless()` wait for SMP; `__ptr_*_bit()` not needed |
| `include/linux/bits.h` | `lib/src/bitops.rs` | done | |
| `include/vdso/bits.h` | `lib/src/bitops.rs` | done | |
| `include/asm-generic/bitops/generic-non-atomic.h` | `lib/src/bitops.rs` | done | |
| `include/asm-generic/bitops/__ffs.h` | `lib/src/bitops.rs` | done | |
| `include/asm-generic/bitops/__fls.h` | `lib/src/bitops.rs` | done | |
| `include/asm-generic/bitops/ffs.h` | `lib/src/bitops.rs` | done | |
| `include/asm-generic/bitops/fls.h` | `lib/src/bitops.rs` | done | |
| `include/asm-generic/bitops/fls64.h` | `lib/src/bitops.rs` | done | |
| `include/asm-generic/bitops/ffz.h` | `lib/src/bitops.rs` | done | |
| `include/linux/hash.h` | `lib/src/hash.rs` | done | |
| `lib/siphash.c` | `lib/src/siphash.rs` | done | one function serves aligned and unaligned data |
| `include/linux/siphash.h` | `lib/src/siphash.rs` | done | |
| `lib/xxhash.c` | `lib/src/xxhash.rs` | done | `xxh32`/`xxh64` one-shot and the `xxh64` streaming state (reset/update/digest) |
| `include/linux/xxhash.h` | `lib/src/xxhash.rs` | done | the xxHash API and `struct xxh64_state` |
| xxHash known-answer tests | `lib/tests/xxhash_kunit.rs` | test | canonical + reference vectors and a streaming-vs-one-shot check |
| `lib/win_minmax.c` | `lib/src/win_minmax.rs` | done | Nichols' windowed running min/max (`minmax_running_max`/`_min`) |
| `include/linux/win_minmax.h` | `lib/src/win_minmax.rs` | done | the `minmax`/`minmax_sample` structs and `get`/`reset` |
| win_minmax tests | `lib/tests/win_minmax_kunit.rs` | test | peak/trough ageing out of the window, monotonic streams, and a 500-sample brute-force cross-check |
| `lib/crypto/sha256.c` | `lib/src/sha256.rs` | partial | the generic block transform and the init/update/final streaming API plus a one-shot; no arch-accelerated blocks, SHA-224, HMAC or two-way finup |
| `include/crypto/sha2.h` | `lib/src/sha256.rs` | partial | the SHA-256 digest/block sizes and initial constants |
| `lib/crypto/sha1.c` | `lib/src/sha1.rs` | partial | the block transform and init/update/final streaming API plus a one-shot; no arch-accelerated blocks or HMAC |
| `lib/crypto/md5.c` | `lib/src/md5.rs` | partial | the `md5_transform` block function and init/update/final streaming API plus a one-shot; no arch-accelerated blocks |
| `include/crypto/md5.h` | `lib/src/md5.rs` | partial | the MD5 digest/block sizes and initial constants |
| MD5 known-answer tests | `lib/tests/md5_kunit.rs` | test | the RFC 1321 test suite and a streaming-vs-one-shot check |
| SHA-1 known-answer tests | `lib/tests/sha1_kunit.rs` | test | the RFC 3174 vectors (empty, abc, two-block, million-a) and a streaming-vs-one-shot check |
| crypto known-answer tests | `lib/tests/sha256_kunit.rs` | test | FIPS 180-4 vectors (empty, abc, two-block, million-a) and a streaming-vs-one-shot check |
| `lib/kfifo.c` | `lib/src/kfifo.rs` | partial | the byte FIFO core (`__kfifo_in`/`__kfifo_out`, copy-in/out with wraparound) as a fixed-capacity `Kfifo<N>`; no `kfifo_alloc`, record variants or DMA/scatterlist helpers |
| `include/linux/kfifo.h` | `lib/src/kfifo.rs` | partial | the byte `Kfifo` API (put/get/peek/len/reset); the typed macro wrappers are replaced by const generics |
| kfifo tests | `lib/tests/kfifo_kunit.rs` | test | put/get, fill-and-reject, buffer wraparound, non-consuming peek, and a long streaming run |
| `lib/uuid.c` | `lib/src/uuid.rs` | done | the random generators draw from a `RandomSource` (`generate_random_uuid_from()`, `generate_random_guid_from()`); the closure forms stay for callers that have one |
| `include/linux/uuid.h` | `lib/src/uuid.rs` | done | |
| `lib/base64.c` | `lib/src/base64.rs` | done | |
| `include/linux/base64.h` | `lib/src/base64.rs` | done | |
| `lib/tests/printf_kunit.c` | `lib/tests/printf_kunit.rs` | test | every buffer size instead of a random one; cases for pending `%p` extensions and `kvasprintf()` not run |
| `lib/tests/scanf_kunit.c` | `lib/tests/scanf_kunit.rs` | test | |
| `lib/tests/cmdline_kunit.c` | `lib/tests/cmdline_kunit.rs` | test | all 256 `u8` values instead of a random one |
| `lib/test_bitmap.c` | `lib/tests/test_bitmap.rs` | test | timing-only perf cases not ported; large strings generated |
| `lib/tests/base64_kunit.c` | `lib/tests/base64_kunit.rs` | test | timing not ported |
| `lib/tests/uuid_kunit.c` | `lib/tests/uuid_kunit.rs` | test | |
| `lib/tests/siphash_kunit.c` | `lib/tests/siphash_kunit.rs` | test | generated vectors |
| `arch/x86/boot/compressed/head_64.S` | `kernel/src/arch/x86/boot/head_64.rs` | partial | `startup_32` to 64-bit mode; relocation, decompression, EFI and SEV not needed |
| `arch/x86/kernel/verify_cpu.S` | `kernel/src/arch/x86/boot/head_64.rs` | done | |
| `arch/x86/include/asm/segment.h` | `kernel/src/arch/x86/include/segment.rs` | partial | generated; `loadsegment()` and other function-like macros pending |
| `arch/x86/kernel/head64.c` | `kernel/src/arch/x86/kernel/head64.rs` | partial | `x86_64_start_kernel()`: switch to `gdt_page`, `copy_bootdata()` |
| `arch/x86/kernel/setup.c` | `kernel/src/arch/x86/kernel/setup.rs` | partial | `setup_arch()`: CPU identification, early traps, memory map, command line, early params, early TSC |
| `arch/x86/include/asm/setup.h` | `kernel/src/init/main.rs` | partial | `COMMAND_LINE_SIZE` |
| `arch/x86/kernel/e820.c` | `kernel/src/arch/x86/kernel/e820.rs` | partial | table from the boot loader and `e820__print_table()`; no sanitizing or memblock |
| `arch/x86/include/asm/e820/types.h` | `kernel/src/arch/x86/kernel/e820.rs` | done | |
| `arch/x86/kernel/early_printk.c` | `kernel/src/arch/x86/kernel/early_printk.rs` | partial | `serial`, `ttyS` and `vga`; `mmio32`, `pciserial`, `dbgp`, `xen`, `xdbc` pending |
| `arch/x86/include/asm/shared/io.h` | `kernel/src/arch/x86/include/io.rs` | done | |
| `arch/x86/include/asm/irqflags.h` | `kernel/src/arch/x86/include/irqflags.rs` | partial | `cli`/`sti`, save/restore, `hlt`, safe halt |
| `arch/x86/include/uapi/asm/processor-flags.h` | `kernel/src/arch/x86/include/processor_flags.rs` | partial | generated; 1 macro not convertible |
| `arch/x86/include/asm/msr-index.h` | `kernel/src/arch/x86/include/msr_index.rs` | partial | generated; 24 function-like or non-numeric macros |
| `include/uapi/linux/serial_reg.h` | `kernel/src/include/uapi/serial_reg.rs` | partial | generated; RSA offsets and `UART_FCR_R_TRIG_BITS()` not converted |
| `init/main.c` | `kernel/src/init/main.rs` | partial | `start_kernel()` through `trap_init()`, `init_IRQ()`, `time_init()`, `calibrate_delay()`; `parse_early_param()`, `debug`/`quiet`/`loglevel`; no `mm_core_init()` or `rest_init()` yet |
| `init/version.c` | `kernel/src/init/version.rs` | partial | `linux_banner` |
| `init/version-timestamp.c` | `kernel/src/init/version.rs` | partial | `linux_banner` text |
| `include/linux/init.h` | `kernel/src/include/linux/init.rs` | partial | `obs_kernel_param`, `early_param()` via the `.init.setup` section |
| `include/linux/kern_levels.h` | `kernel/src/kernel/printk.rs` | done | levels as integers rather than `KERN_SOH` strings |
| `include/linux/printk.h` | `kernel/src/kernel/printk.rs` | partial | `pr_*()` macros, console log levels |
| `include/linux/console.h` | `kernel/src/kernel/printk.rs` | partial | `struct console`, `CON_*` flags |
| `kernel/printk/printk.c` | `kernel/src/kernel/printk.rs` | partial | log buffer, `register_console()` with replay, timestamps, `console_verbose()`; no ringbuffer, kmsg, syslog or `pr_cont()` |
| `kernel/panic.c` | `kernel/src/kernel/panic.rs` | partial | `panic()` message and halt |
| `lib/bug.c` | `kernel/src/kernel/panic.rs` | partial | "kernel BUG at" report for Rust panics |
| `kernel/params.c` | `kernel/src/kernel/params.rs` | partial | `parameq()`, `parameqn()`, `parse_args()` without module parameters |
| `arch/x86/entry/entry_64.S` | `kernel/src/arch/x86/entry/entry_64.rs` | partial | `idtentry` stubs, int3 call-emulation gap; syscall entry, user return and paranoid entry come with user space |
| `arch/x86/include/asm/idtentry.h` | `kernel/src/arch/x86/entry/entry_64.rs` | partial | `irq_entries_start`, `spurious_entries_start` |
| `arch/x86/include/asm/ptrace.h` | `kernel/src/arch/x86/include/ptrace.rs` | partial | `struct pt_regs`, `user_mode()` |
| `arch/x86/include/asm/desc_defs.h` | `kernel/src/arch/x86/include/desc.rs` | partial | descriptors and gates; `AR_*` access rights pending |
| `arch/x86/include/asm/desc.h` | `kernel/src/arch/x86/include/desc.rs` | partial | gate packing, TSS descriptors, `lgdt`/`lidt`/`ltr` |
| `arch/x86/include/asm/processor.h` | `kernel/src/arch/x86/include/processor.rs` | partial | `cpuid()`, `struct x86_hw_tss`, `cpu_relax()` |
| `arch/x86/include/asm/special_insns.h` | `kernel/src/arch/x86/include/processor.rs` | partial | control register reads |
| `arch/x86/include/asm/msr.h` | `kernel/src/arch/x86/include/processor.rs` | partial | `rdmsr`, `wrmsr`, `rdtsc`, `rdtsc_ordered` |
| `arch/x86/include/asm/tsc.h` | `kernel/src/arch/x86/include/processor.rs` | partial | `get_cycles()` |
| `arch/x86/include/asm/io.h` | `kernel/src/arch/x86/include/io.rs` | partial | `outb_p()`, `native_io_delay()` |
| `arch/x86/include/asm/trapnr.h` | `kernel/src/arch/x86/include/trapnr.rs` | done | generated |
| `arch/x86/include/asm/irq_vectors.h` | `kernel/src/arch/x86/include/irq_vectors.rs` | partial | generated; `ISA_IRQ_VECTOR()` is in `kernel/i8259.rs` |
| `arch/x86/include/asm/i8259.h` | `kernel/src/arch/x86/include/i8259.rs` | partial | ports and init words |
| `arch/x86/include/asm/kdebug.h` | `kernel/src/arch/x86/include/kdebug.rs` | partial | `enum die_val` |
| `include/linux/kdebug.h` | `kernel/src/arch/x86/include/kdebug.rs` | partial | `struct die_args` |
| `arch/x86/include/asm/trap_pf.h` | `kernel/src/arch/x86/mm/fault.rs` | done |  |
| `arch/x86/include/asm/page_64_types.h` | `kernel/src/arch/x86/kernel/cpu/common.rs` | partial | IST indices; exception stacks are 16 KiB, not one page |
| `arch/x86/include/asm/cpu_entry_area.h` | `kernel/src/arch/x86/kernel/cpu/common.rs` | partial | exception stack layout, as plain statics |
| `arch/x86/include/asm/text-patching.h` | `kernel/src/arch/x86/kernel/alternative.rs` | partial | `int3_emulate_push/jmp/call()` |
| `arch/x86/kernel/cpu/common.c` | `kernel/src/arch/x86/kernel/cpu/common.rs` | partial | `gdt_page`, TSS and IST, `set_tss_sp0()`, `cpu_init_exception_handling()`, `cpu_detect()`, vendor and model name, `print_cpu_info()` |
| `arch/x86/lib/cpu.c` | `kernel/src/arch/x86/kernel/cpu/common.rs` | done |  |
| `arch/x86/kernel/idt.c` | `kernel/src/arch/x86/kernel/idt.rs` | partial | exception and external interrupt gates; APIC vectors pending |
| `arch/x86/kernel/traps.c` | `kernel/src/arch/x86/kernel/traps.rs` | partial | kernel-mode exception handlers, `trap_init()`; signals, fixups, `WARN()` via `ud2`, #GP address decoding pending |
| `arch/x86/kernel/nmi.c` | `kernel/src/arch/x86/kernel/traps.rs` | partial | unknown NMI report |
| `arch/x86/kernel/dumpstack.c` | `kernel/src/arch/x86/kernel/dumpstack.rs` | partial | `die()`, oops markers, `show_regs()`, `Code:` line, frame-pointer call trace; no kallsyms |
| `lib/dump_stack.c` | `kernel/src/arch/x86/kernel/dumpstack.rs` | partial | `dump_stack_print_info()` |
| `arch/x86/kernel/process_64.c` | `kernel/src/arch/x86/kernel/process_64.rs` | partial | `__show_regs()`; `do_arch_prctl_64` (FS/GS base get/set via the MSRs) is `sys_arch_prctl` in `kernel/src/kernel/syscalls.rs`, with no per-task save/restore of the base |
| `arch/x86/include/uapi/asm/prctl.h` | `kernel/src/kernel/syscalls.rs` | partial | the `ARCH_SET_FS`/`ARCH_GET_FS`/`ARCH_SET_GS`/`ARCH_GET_GS` codes |
| `arch/x86/mm/fault.c` | `kernel/src/arch/x86/mm/fault.rs` | partial | kernel page fault oops, `dump_pagetable()`; a user-mode fault now kills the task with SIGSEGV; demand paging and exception tables pending |
| `arch/x86/kernel/irq.c` | `kernel/src/arch/x86/kernel/irq.rs` | partial | `common_interrupt()`, `vector_irq`, spurious vectors |
| `arch/x86/kernel/irqinit.c` | `kernel/src/arch/x86/kernel/irqinit.rs` | partial | `init_IRQ()` with the legacy PIC and cascade |
| `arch/x86/kernel/i8259.c` | `kernel/src/arch/x86/kernel/i8259.rs` | partial | init, mask/unmask, specific-EOI ack; suspend/resume, ELCR pending |
| `arch/x86/kernel/i8253.c` | `kernel/src/arch/x86/kernel/time.rs` | partial | `setup_pit_timer()` |
| `drivers/clocksource/i8253.c` | `kernel/src/drivers/clocksource/i8253.rs` | partial | periodic mode; PIT clocksource and one-shot pending |
| `include/linux/i8253.h` | `kernel/src/drivers/clocksource/i8253.rs` | done |  |
| `arch/x86/kernel/time.c` | `kernel/src/arch/x86/kernel/time.rs` | partial | timer IRQ, `time_init()`, late timer setup |
| `arch/x86/kernel/tsc.c` | `kernel/src/arch/x86/kernel/tsc.rs` | partial | CPUID, fast and slow PIT calibration, cyc2ns, `native_sched_clock()`; HPET/PM timer references, clocksource, sync checks pending |
| `arch/x86/lib/delay.c` | `kernel/src/arch/x86/lib/delay.rs` | partial | loop and TSC delays, `udelay()` |
| `arch/x86/kernel/alternative.c` | `kernel/src/arch/x86/kernel/alternative.rs` | partial | `int3_selftest()`; code patching pending |
| `init/calibrate.c` | `kernel/src/init/calibrate.rs` | partial | `calibrate_delay()` with timer-frequency and converge paths; `lpj=` and direct calibration pending |
| `kernel/notifier.c` | `kernel/src/kernel/notifier.rs` | partial | atomic notifier chains, die chain |
| `kernel/softirq.c` | `kernel/src/kernel/softirq.rs` | partial | `irq_enter()`/`irq_exit()` accounting |
| `kernel/exit.c` | `kernel/src/kernel/exit.rs` | partial | `make_task_dead()` for the idle task |
| `kernel/irq/irqdesc.c` | `kernel/src/kernel/irq/mod.rs` | partial | fixed table of 16 legacy IRQs, counts |
| `kernel/irq/chip.c` | `kernel/src/kernel/irq/mod.rs` | partial | `handle_level_irq()` |
| `kernel/irq/manage.c` | `kernel/src/kernel/irq/mod.rs` | partial | `request_irq()` |
| `include/linux/irqreturn.h` | `kernel/src/kernel/irq/mod.rs` | partial | `IRQ_NONE`, `IRQ_HANDLED` |
| `kernel/time/clocksource.c` | `kernel/src/kernel/time/clocksource.rs` | partial | `clocks_calc_mult_shift()` |
| `kernel/time/tick-common.c` | `kernel/src/kernel/time/tick_common.rs` | partial | periodic tick handler |
| `kernel/time/jiffies.c` | `kernel/src/kernel/time/jiffies.rs` | partial | `jiffies_64` |
| `include/linux/jiffies.h` | `kernel/src/kernel/time/jiffies.rs` | partial | `INITIAL_JIFFIES`, `HZ` |
| `kernel/time/time.c` | `lib/src/mktime.rs` | partial | `mktime64()` (host-tested); `sys_gettimeofday` (`struct timeval` from `ktime_get_real_ts64`) is in `kernel/src/kernel/syscalls.rs` |
| `kernel/time/timekeeping.c` | `kernel/src/kernel/time/timekeeping.rs` | partial | boot wall-clock anchor from the RTC; `timekeeping_init()`, `ktime_get_real_ts64()` over jiffies |
| `kernel/time/timer.c` | `lib/src/timer_wheel.rs` | partial | `wheel index`: `calc_wheel_index()` and `calc_index()`; the wheel queues and expiry are not ported|
| `kernel/time/ntp.c` | `lib/src/ntp.rs` | partial | `frequency discipline`: `ntp_update_frequency()`, `ntp_update_offset()`, `ntp_update_offset_fll()`; `second_overflow()`, drain/skew, RTC sync, PPS and `adjtimex` are not ported|
| `kernel/time/namespace.c` | `lib/src/time_namespace.rs` | partial | `offset math`: `do_timens_ktime_to_host()` and the monotonic and boottime offset helpers; the namespace objects and `/proc` are not ported|
| `kernel/time/clockevents.c` | `lib/src/clockevents.rs` | partial | `programming core`: `cev_delta2ns()`, the min-delta retry, `program_event()` and `event_fired()` over `SetNextEvent`; the PIT is the tick's clock event device (steps 150 to 152); the registry, sysfs and the coupled paths are not ported|
| `kernel/time/timeconv.c` | `lib/src/timeconv.rs` | done | `time64_to_tm()` with the `Tm` struct, C's widths and wrapping kept |
| `kernel/time/sleep_timeout.c` | `kernel/src/kernel/time/sleep_timeout.rs` | partial | `schedule_timeout()`, `schedule_timeout_uninterruptible()`, `msleep()`, `msecs_to_jiffies()`; the interruptible, killable, idle and hrtimer-range forms are not ported |
| `kernel/time/timecounter.c` | `lib/src/timecounter.rs` | done | `Timecounter` over a `Cyclecounter` whose `read` is a closure; `cyclecounter_cyc2ns()` from `include/linux/timecounter.h`; `timecounter_cyc2time()` not ported |
| `kernel/time/time_test.c` | `lib/tests/time_test.rs` | test | the 80000-year day-by-day `time64_to_tm()` range test |
| `kernel/time/posix-timers.c` | `kernel/src/kernel/syscalls.rs` | partial | `sys_clock_gettime` for `CLOCK_REALTIME`/`CLOCK_MONOTONIC` |
| `kernel/time/hrtimer.c` | `kernel/src/kernel/syscalls.rs` | partial | `sys_nanosleep`: round up to whole jiffies, then sleep the task (`sleep_until()`), which the tick wakes at its deadline so other tasks run meanwhile; no `hrtimer` arming, so the wake is tick-granular |
| `kernel/time/hrtimer.c` | `lib/src/hrtimer.rs` | partial | `queue core`: `hrtimer_forward()` and an expiry-ordered `TimerQueue` (start, cancel, next expiry, pop expired); not yet used by the kernel's nanosleep, which still uses `sleep_until()`|
| `include/uapi/linux/time.h` | `kernel/src/kernel/syscalls.rs` | partial | `CLOCK_REALTIME`, `CLOCK_MONOTONIC`, `struct timespec`/`timeval` layout |
| `kernel/sched/clock.c` | `kernel/src/kernel/sched/clock.rs` | partial | `local_clock()` for one CPU with a stable clock |
| `include/linux/list.h` | `lib/src/list.rs` | partial | intrusive list: add/del, del_init, iteration; hlist and RCU variants pending |
| `lib/rbtree.c` | `lib/src/rbtree.rs` | partial | insert/erase/first/next; `insert_color_augmented()` and `erase_augmented()` take an `RbAugment`; `rb_prev` pending |
| `include/linux/rbtree.h` | `lib/src/rbtree.rs` | partial | nodes, roots, link_node |
| `include/linux/rbtree_augmented.h` | `lib/src/rbtree.rs` | done | colour/parent helpers; the `rb_augment_callbacks` as `RbAugment` (`propagate`, `copy`, `rotate`) called at the same points as C |
| `lib/timerqueue.c` | `lib/src/timerqueue.rs` | partial | `timerqueue_add`/`del`/`getnext`/`iterate_next` over the rbtree with the earliest node cached; no linked variant (the caller serializes, as in the C) |
| `include/linux/timerqueue.h` | `lib/src/timerqueue.rs` | partial | the `timerqueue_node`/`timerqueue_head` structs |
| timerqueue tests | `lib/tests/timerqueue_kunit.rs` | test | ordering, the cached earliest, deletion advancing the cache, and a sorted cross-check |
| `lib/tests/list-test.c` | `lib/tests/list_test.rs` | test |  |
| `lib/list_sort.c` | `lib/src/list.rs` | partial | `list_sort`: the bottom-up stable merge sort (merge/merge_final), with a closure comparator |
| `include/linux/llist.h` | `lib/src/llist.rs` | partial | the lock-less list API (add/add_batch/del_first/del_all/reverse/iter) via `AtomicPtr`; the typed for_each_entry macros use the offset instead |
| `lib/llist.c` | `lib/src/llist.rs` | partial | `llist_del_first`, `llist_reverse_order` and the cmpxchg add loop; the `_this`/`on_list` helpers pending |
| llist tests | `lib/tests/llist_kunit.rs` | test | LIFO add/del_all, empty-flag return, del_first, reverse_order, and a batch push |
| list_sort tests | `lib/tests/list_sort_kunit.rs` | test | ordering, already-sorted/reversed, empty/single, stability for equal keys, and a 1000-element random sort |
| `include/linux/plist_types.h` | `lib/src/plist.rs` | done | the `plist_head`/`plist_node` layout |
| `include/linux/plist.h` | `lib/src/plist.rs` | partial | the plist API (init/first/iter); the typed `for_each_entry` macros use the offset instead |
| `lib/plist.c` | `lib/src/plist.rs` | partial | `plist_add`/`plist_del` with the deduplicated prio_list; the debug `plist_check_*` are dropped |
| plist tests | `lib/tests/plist_kunit.rs` | test | priority-ordered inserts, stability, the prio_list hand-off on delete, and a 300-op randomized run |
| `lib/rbtree_test.c` | `lib/tests/rbtree_test.rs` | test | random insert/erase with invariant checks |
| `include/vdso/page.h` | `kernel/src/arch/x86/include/page.rs` | partial | PAGE_SHIFT/SIZE/MASK, alignment |
| `arch/x86/include/asm/page_types.h` | `kernel/src/arch/x86/include/page.rs` | partial | PFN helpers |
| `arch/x86/include/asm/page_64.h` | `kernel/src/arch/x86/include/page.rs` | partial | __pa/__va (identity for now) |
| `arch/x86/include/asm/pgtable_types.h` | `kernel/src/arch/x86/include/pgtable.rs` | partial | PTE flags, PAGE_KERNEL* |
| `arch/x86/include/asm/pgtable_64_types.h` | `kernel/src/arch/x86/include/pgtable.rs` | partial | 4-level shifts and fan-out, index helpers |
| `mm/memblock.c` | `kernel/src/mm/memblock.rs` | partial | region arrays, add/reserve/remove, free-range iteration, top-down alloc; fixed-size arrays, no NUMA or bottom-up |
| `arch/x86/mm/init.c` | `kernel/src/arch/x86/mm/init.rs` | partial | init_mem_mapping(): direct map, page 0 unmapped, on-demand kernel page map/unmap |
| `arch/x86/mm/init_64.c` | `kernel/src/arch/x86/mm/init.rs` | partial | 4-level page-table population |
| `mm/mm_init.c` | `kernel/src/mm/mm_init.rs` | partial | mm_core_init(): mem_map allocation and memblock_free_all(); no sparsemem or zones |
| `mm/page_alloc.c` | `kernel/src/mm/page_alloc.rs` | partial | buddy __rmqueue_smallest/expand/__free_one_page, boot free; one zone/migratetype, no pcp, watermarks, reclaim or compaction |
| `mm/slub.c` | `kernel/src/mm/slub.rs` | partial | kmalloc size-class slab caches, kmalloc_large, kfree, ksize; no per-CPU slabs, merging or page reclaim |
| `mm/slab_common.c` | `kernel/src/mm/slub.rs` | partial | kmalloc size classes |
| `mm/vmalloc.c` | `kernel/src/mm/vmalloc.rs` | partial | vmalloc/vfree with an address-ordered rbtree of areas, first-fit; no vmap(), huge pages or per-CPU caches |
| `include/linux/sched.h` | `kernel/src/include/linux/sched.rs` | partial | task_struct and thread_struct fields the scheduler needs, task states, current(), `kstack_top` and the user `mm_pgd`; files, signals, creds pending |
| `arch/x86/include/asm/processor.h` | `kernel/src/include/linux/sched.rs` | partial | thread.sp |
| `arch/x86/entry/entry_64.S` | `kernel/src/arch/x86/kernel/process.rs` | partial | __switch_to_asm and the new-thread trampoline (ret_from_fork_asm); also the idtentry stubs in entry/ |
| `arch/x86/include/asm/switch_to.h` | `kernel/src/arch/x86/kernel/process.rs` | partial | inactive_task_frame layout, kthread_frame_init |
| `arch/x86/kernel/process.c` | `kernel/src/arch/x86/kernel/process.rs` | partial | copy_thread for kernel threads and `start_thread()` (ring-3 entry via iretq); FPU, FS/GS and TLS pending |
| `kernel/sched/fair.c` | `lib/src/sched_fair.rs` | partial | `weights and delta`: the CFS load-weight tables (`sched_prio_to_weight`, `sched_prio_to_wmult`, copied by script), `scale_load`/`scale_load_down`, `__update_inv_weight()`, `__calc_delta()` and `calc_delta_fair()`; the runqueue, pick and enqueue logic waits for the kernel's scheduler|
| `kernel/sched/core.c` | `lib/src/sched_fair.rs` | done | `weight tables`: `sched_prio_to_weight` and `sched_prio_to_wmult`, extracted from the C source|
| `kernel/sched/core.c` | `kernel/src/kernel/sched/core.rs` | partial | runqueue, __schedule/context_switch/finish_task_switch, schedule(), wake_up_new_task, sched_tick, do_exit, kernel_thread, cpu idle loop, sched_init; round-robin only, one CPU, no CFS/EEVDF, load balancing or mm switch |
| `kernel/sched/sched.h` | `lib/src/sched_rt_prio.rs` | partial | ``rt_prio_array``: the RT priority array: one FIFO per priority over entity ids, a bitmap with the delimiter bit, enqueue, dequeue and the highest-priority pick; the rt_rq and bandwidth logic wait|
| `kernel/sched/cputime.c` | `lib/src/cputime.rs` | partial | ``cputime_adjust``: the monotonic split of precise runtime into user and system time, with the sample and previous state explicit; the per-task lock and the tick accounting wait for the scheduler|
| `kernel/sched/deadline.c` | `lib/src/sched_dl_cbs.rs` | partial | `CBS rules`: `replenish_dl_new_period()`, `replenish_dl_entity()` and `dl_entity_overflow()` over explicit parameters; the deferred-server, yield and tracing paths and the rq clock wait for the scheduler|
| `kernel/sched/sched.h` | `lib/src/sched_dl_cbs.rs` | done | ``DL_SCALE``: the overflow scale|
| `kernel/sched/cpupri.c` | `lib/src/cpupri.rs` | partial | `convert_prio()`, `cpupri_set()` and `cpupri_find()` without a fitness function, over per-level membership; the cpumask-based fitness search and the atomics wait |
| `kernel/sched/cpupri.h` | `lib/src/cpupri.rs` | done | the level constants |
| `kernel/sched/completion.c` | `lib/src/completion.rs` | done | `complete()`, `complete_all()`, `try_wait_for_completion()`, `completion_done()` over an atomic count with C's UINT_MAX meaning, and the blocking wait and wake over a `TaskBlocker` (`wait_for_completion()`, `complete_wake()`); the spinlock is the caller's, as C's lock is released inside `schedule()` |
| `include/linux/completion.h` | `lib/src/completion.rs` | partial | the count and its states; the wait queue member waits |
| `include/linux/sched/prio.h` | `lib/src/sched_prio.rs` | done | the nice and priority constants, the conversions, and the policy numbers used by `normal_prio()` |
| `kernel/sched/syscalls.c` | `lib/src/sched_prio.rs` | done | ``__normal_prio``: `normal_prio()`: deadline, then real-time by rt priority, then nice|
| `kernel/sched/sched.h` | `lib/src/sched_uclamp.rs` | done | ``uclamp_bucket_id``: `UCLAMP_BUCKET_DELTA`, `uclamp_bucket_id()` and `DIV_ROUND_CLOSEST`, with the default five-bucket count from `init/Kconfig`|
| `kernel/sched/cpudeadline.c` | `lib/src/cpudl.rs` | partial | `cpudl_set()`, `cpudl_clear()`, `cpudl_maximum()` and the heapify functions over CPU ids, with `find()` without a mask; the cpumask search with capacity fitting and the lock wait |
| `kernel/sched/loadavg.c` | `lib/src/loadavg.rs` | partial | `calc_load()`, `fixed_power_int()`, `calc_load_n()`, the `avenrun[]` update and `LOAD_INT`/`LOAD_FRAC`; the NO_HZ folding and the per-CPU active counts wait for the scheduler |
| `include/linux/sched/loadavg.h` | `lib/src/loadavg.rs` | done | `FSHIFT`, `FIXED_1`, `EXP_1`, `EXP_5`, `EXP_15` and the load macros |
| `kernel/sched/pelt.c` | `lib/src/sched_pelt.rs` | partial | `decay_load()` and `__accumulate_pelt_segments()` with the copied inverse-decay table; the per-entity update and the runnable and util averages wait for the scheduler |
| `kernel/sched/sched-pelt.h` | `lib/src/sched_pelt.rs` | done | `LOAD_AVG_PERIOD`, `LOAD_AVG_MAX` and `runnable_avg_yN_inv[]`, the table copied by script |
| `kernel/sched/wait.c` | `lib/src/wait.rs` | done | `add_wait_queue()`, `add_wait_queue_exclusive()`, `remove_wait_queue()`, `__wake_up_common()`, and the sleep and wake paths over a `TaskBlocker` (`wait_event()`, `wake_up()`); the spinlock is the caller's, as in C |
| `kernel/sched/rt.c` | `kernel/src/kernel/sched/core.rs` | partial | RR_TIMESLICE round-robin rotation |
| `kernel/kthread.c` | `kernel/src/kernel/kthread.rs` | partial | kthread_run; kthread_stop, parking and kthreadd pending |
| `include/linux/preempt.h` | `kernel/src/kernel/softirq.rs` | partial | preempt_count, preempt_disable/enable (spinlocks disable preemption) |
| `kernel/sched/idle.c` | `kernel/src/kernel/sched/core.rs` | partial | cpu_idle_loop (do_idle) |
| `include/uapi/linux/stat.h` | `kernel/src/include/linux/fs.rs` | partial | S_IF* mode bits |
| `include/uapi/asm-generic/fcntl.h` | `kernel/src/include/linux/fs.rs` | partial | O_* open flags |
| `include/linux/fs.h` | `kernel/src/include/linux/fs.rs` | partial | file_operations trait, DirEntry; the full inode/file/super_block structs pending |
| `fs/namei.c` | `kernel/src/fs/vfs.rs` | partial | path_lookup, parent/name split; no dcache, symlinks-in-path, permissions or RCU walk |
| `fs/open.c` | `kernel/src/fs/vfs.rs` | partial | filp_open, vfs_create, O_TRUNC |
| `fs/read_write.c` | `kernel/src/fs/vfs.rs` | partial | vfs_read, vfs_write, vfs_llseek |
| `fs/readdir.c` | `kernel/src/fs/vfs.rs` | partial | `vfs_readdir`/`vfs_readdir_ino`, and `sys_getdents64` (in `kernel/syscalls.rs`) fills the user buffer from them |
| `include/linux/dirent.h` | `kernel/src/kernel/syscalls.rs` | partial | the `linux_dirent64` record layout `getdents64` writes |
| `fs/inode.c` | `kernel/src/fs/vfs.rs` | partial | new_inode, the inode arena |
| `fs/libfs.c` | `kernel/src/fs/ramfs.rs` | partial | simple_* read/write via the ramfs cores |
| `fs/ramfs/inode.c` | `kernel/src/fs/ramfs.rs` | partial | ramfs file_operations and mount_rootfs; data stored in the inode, not the page cache |
| `init/initramfs.c` | `kernel/src/init/initramfs.rs` | partial | newc cpio header parsing and unpack of dirs/files/symlinks; no decompression, hard links or device nodes |
| `fs/proc/root.c` | `kernel/src/fs/proc/mod.rs` | partial | proc_root_init, /proc directory |
| `fs/proc/generic.c` | `kernel/src/fs/proc/mod.rs` | partial | proc files whose contents a function generates (version, meminfo, uptime, cmdline); no seq_file or per-pid |
| `arch/x86/kernel/cpu/proc.c` | `kernel/src/fs/proc/mod.rs` | partial | `/proc/cpuinfo` `show_cpuinfo` for the boot CPU (processor/vendor/family/model/stepping); the feature flags line omitted |
| `fs/proc/stat.c` | `kernel/src/fs/proc/mod.rs` | partial | `/proc/stat`: a reduced `show_stat` (uptime as idle, ctxt, running tasks); no per-mode CPU accounting |
| `fs/proc/loadavg.c` | `kernel/src/fs/proc/mod.rs` | partial | `/proc/loadavg`: running/total tasks; the load averages are not computed (read as zero) |
| `fs/filesystems.c` | `kernel/src/fs/proc/mod.rs` | partial | `/proc/filesystems`: the registered filesystem types |
| `kernel/ksysfs.c` | `kernel/src/fs/sysfs.rs` | partial | the `/sys/kernel` attributes `cpu_byteorder`, `address_bits` and `uevent_seqnum`; no kobject/kset model, device tree or writable attributes |
| `include/uapi/linux/elf.h` | `kernel/src/include/uapi/elf.rs` | partial | ELF64 `Ehdr`/`Phdr` and the `ET_`/`PT_`/`PF_`/`AT_` constants the loader needs; dynamic, section and note types pending |
| `fs/binfmt_elf.c` | `kernel/src/fs/binfmt_elf.rs` | partial | `load_elf_binary()` for static non-PIE `ET_EXEC`: maps `PT_LOAD`, builds the argv/envp/auxv stack; no `PT_INTERP`/ld.so, PIE, core dumps or randomization |
| `fs/exec.c` | `kernel/src/fs/exec.rs` | partial | `kernel_execve()`: read the file, load the ELF, install the mm and enter user mode; `do_brk()`; no argv from the caller, no `mm_struct`/VMA tree |
| `arch/x86/mm/pgtable.c` | `kernel/src/arch/x86/mm/user.rs` | partial | per-task user PGD (`pgd_alloc`: copy the kernel entries), map/copy user pages in a high PGD slot; no PCID, no `mm_struct` |
| `arch/x86/entry/entry_64.S` | `kernel/src/arch/x86/kernel/syscall_64.rs` | partial | the `entry_SYSCALL_64` stub: save the user frame, switch to the kernel stack, call the dispatcher, `sysretq`; no paranoid/NMI paths, no FRED |
| `arch/x86/entry/common.c` | `kernel/src/arch/x86/kernel/syscall_64.rs` | partial | `do_syscall_64()` dispatch; no seccomp, ptrace or audit hooks |
| `arch/x86/kernel/cpu/common.c` | `kernel/src/arch/x86/kernel/syscall_64.rs` | partial | `syscall_init()`: program STAR/LSTAR/SFMASK and `EFER.SCE` |
| `kernel/sys.c` | `kernel/src/kernel/syscalls.rs` | partial | `sys_getpid()`/`sys_getppid()`, `sys_newuname()`, `sys_sysinfo()`, the `getuid`/`geteuid`/`getgid`/`getegid` getters, and `sys_prlimit64()` (`do_prlimit`: get/set a limit, rejecting soft > hard); the rest of `sys.c` pending |
| `include/asm-generic/resource.h` | `kernel/src/kernel/resource.rs` | partial | the `INIT_RLIMITS` table as the single task's limits |
| `include/uapi/asm-generic/resource.h` | `kernel/src/kernel/resource.rs` | partial | the `RLIMIT_*` numbers and `RLIM_INFINITY` |
| `include/uapi/linux/resource.h` | `kernel/src/kernel/resource.rs` | partial | `struct rlimit64`, `_STK_LIM`, `MLOCK_LIMIT` |
| `kernel/fork.c` | `kernel/src/kernel/syscalls.rs` | partial | `sys_set_tid_address()` records `clear_child_tid`, and the exit path zeroes it and futex-wakes a waiter as `mm_release()` does; `fork`/`clone` themselves pending |
| `kernel/futex/syscalls.c` | `kernel/src/kernel/syscalls.rs` | partial | `sys_set_robust_list()`, and `sys_futex()` as `do_futex`: `FUTEX_WAIT` (with the relative timeout converted to jiffies) and `FUTEX_WAKE`, flags accepted; PI, requeue and `FUTEX_WAKE_OP` pending |
| `kernel/futex/core.c` | `kernel/src/kernel/futex.rs` | partial | the waiter queue (one list standing in for the hash buckets) and `exit_robust_list`/`handle_futex_death`; no `futex_key`, hashing or PI state |
| `kernel/futex/waitwake.c` | `kernel/src/kernel/futex.rs` | partial | `futex_wait` (value check, queue, timed sleep with interrupts on) and `futex_wake`; no bitsets |
| `include/uapi/linux/futex.h` | `kernel/src/kernel/futex.rs` | partial | the ops and flags, `FUTEX_WAITERS`/`OWNER_DIED`/`TID_MASK`, and the `robust_list_head` layout |
| `kernel/cred.c` | `kernel/src/kernel/cred.rs` | partial | `struct cred`, a root `INIT_CRED` and `current_cred()`; one immutable cred, no refcounting, namespaces or supplementary groups |
| `kernel/signal.c` | `kernel/src/kernel/syscalls.rs` | partial | `sys_rt_sigaction`/`sys_rt_sigprocmask`, `sys_kill` (raise a pending signal) and `do_signal` (via `kernel/signal.rs`); no process groups, `siginfo` payloads, default actions or job control |
| `arch/x86/kernel/signal_64.c` | `kernel/src/arch/x86/kernel/signal.rs` | partial | `x64_setup_rt_frame` and `rt_sigreturn`: build the `rt_sigframe` on the user stack (filling the `siginfo` for `SA_SIGINFO` handlers) and restore from it; no FPU/xstate or shadow stack |
| `include/uapi/asm-generic/siginfo.h` | `kernel/src/arch/x86/kernel/signal.rs` | partial | the `siginfo` field offsets and `SI_USER` filled for `SA_SIGINFO` delivery |
| `arch/x86/kernel/signal.c` | `kernel/src/arch/x86/kernel/signal.rs` | partial | `get_sigframe` (red zone, 16-byte alignment) and `do_signal` on the syscall return path |
| `arch/x86/include/uapi/asm/sigcontext.h` | `kernel/src/arch/x86/kernel/signal.rs` | partial | the `struct sigcontext_64` register offsets |
| `include/uapi/asm-generic/ucontext.h` | `kernel/src/arch/x86/kernel/signal.rs` | partial | the `ucontext` layout around `uc_mcontext` |
| `include/linux/signal_types.h` | `kernel/src/kernel/signal.rs` | partial | the `k_sigaction` handler table and blocked mask |
| `include/uapi/asm-generic/signal.h` | `kernel/src/kernel/signal.rs` | partial | `_NSIG`, `SIG_BLOCK`/`SIG_UNBLOCK`/`SIG_SETMASK`, `SA_RESTORER`, and the unblockable `SIGKILL`/`SIGSTOP` |
| `include/linux/cred.h` | `kernel/src/kernel/cred.rs` | partial | the `cred` fields (uids/gids and capability sets) |
| `include/uapi/linux/sysinfo.h` | `kernel/src/kernel/syscalls.rs` | partial | the `struct sysinfo` field layout filled by `sys_sysinfo` |
| `kernel/capability.c` | `kernel/src/kernel/syscalls.rs` | partial | `sys_capget` with `cap_validate_magic` (v1/v2/v3 ABI); the single root task reports the full set, so `capset` and per-task lookup are not ported |
| `include/uapi/linux/capability.h` | `kernel/src/kernel/syscalls.rs` | partial | the capability ABI versions, `CAP_LAST_CAP` and the full-set value |
| `include/uapi/linux/utsname.h` | `kernel/src/kernel/syscalls.rs` | partial | the `new_utsname` layout (six 65-byte fields) that `uname(2)` fills |
| `mm/mmap.c` | `kernel/src/mm/mmap.rs` | partial | `do_mmap` (anonymous only, pages populated eagerly), top-down `get_unmapped_area` below the stack gap and `do_munmap` with `split_vma`, over a sorted VMA `Vec`; `sys_brk()` (via `fs/exec.rs`) grows the heap by mapping pages; no file backing, maple tree, hole reuse or `MAP_FIXED` replacement |
| `fs/read_write.c` | `kernel/src/kernel/syscalls.rs` | partial | `sys_write`/`sys_writev`/`sys_read` and `sys_lseek` on open fds; the vfs helpers are in `fs/vfs.rs` |
| `fs/file.c` | `kernel/src/kernel/syscalls.rs` | partial | a tiny open-file table and `sys_open`/`sys_close`, refusing a descriptor at or beyond `RLIMIT_NOFILE` with `EMFILE` as `alloc_fd` does (one global table, no per-task `files_struct`, dup or O_CLOEXEC) |
| `fs/d_path.c` | `kernel/src/kernel/syscalls.rs` | partial | `sys_getcwd` returning the task's cwd string; no dentry-to-path walk |
| `fs/fs_struct.c` | `kernel/src/fs/fs_struct.rs` | partial | `struct fs_struct`, a root `INIT_FS` and `current_fs()`; one immutable context, no refcounting, `chroot` or `chdir` |
| `include/linux/fs_struct.h` | `kernel/src/fs/fs_struct.rs` | partial | the root and pwd fields |
| `mm/mprotect.c` | `kernel/src/mm/mmap.rs` | partial | `do_mprotect_pkey`: coverage check, VMA split, PTE re-flag; no pkeys or accounting |
| `include/uapi/asm-generic/mman-common.h` | `kernel/src/mm/mmap.rs` | partial | `PROT_*` and the `MAP_SHARED`/`PRIVATE`/`FIXED`/`ANONYMOUS` flags |
| `fs/stat.c` | `kernel/src/kernel/syscalls.rs` | partial | `sys_newstat`: fill the user `struct stat` (mode, size, nlink, blksize) from `vfs_stat`; timestamps and the rest are zero |
| `arch/x86/include/uapi/asm/stat.h` | `kernel/src/kernel/syscalls.rs` | partial | the x86-64 `struct stat` field offsets `uname`/`stat` write |
| `include/uapi/linux/pci_regs.h` | `kernel/src/include/uapi/pci_regs.rs` | partial | the header-0/header-1 identity, BAR, bus-number and class fields the scan reads; capabilities and PCIe extended config pending |
| `arch/x86/pci/direct.c` | `kernel/src/arch/x86/pci/direct.rs` | partial | type-1 config access (`pci_conf1_read`/`write` via CF8/CFC under a lock); type-2 and MMCONFIG pending |
| `include/linux/pci.h` | `kernel/src/drivers/pci/mod.rs` | partial | `PCI_DEVFN`/`PCI_SLOT`/`PCI_FUNC` and the config accessors; `struct pci_dev` reduced to the identity header and raw BARs |
| `drivers/pci/probe.c` | `kernel/src/drivers/pci/mod.rs` | partial | the bus scan (`pci_scan_slot`/`pci_scan_child_bus`): find every function and descend through bridges; no resource sizing/assignment, driver binding, MSI or hotplug |
| `include/uapi/linux/virtio_config.h` | `kernel/src/include/uapi/virtio_config.rs` | partial | the `VIRTIO_CONFIG_S_*` status bits; feature-bit definitions pending |
| `include/uapi/linux/virtio_ids.h` | `kernel/src/include/uapi/virtio_ids.rs` | partial | net/block/console/rng device-type IDs |
| `include/uapi/linux/virtio_pci.h` | `kernel/src/include/uapi/virtio_pci.rs` | partial | the legacy (I/O-BAR) register offsets; the modern capability layout and MSI-X offsets pending |
| `include/uapi/linux/virtio_ring.h` | `kernel/src/include/uapi/virtio_ring.rs` | partial | split-ring descriptor flags and `vring_size()`; packed ring and event index pending |
| `drivers/virtio/virtio_ring.c` | `kernel/src/drivers/virtio/virtio_ring.rs` | partial | split-ring setup, `publish`/used-ring polling; one buffer in flight, no indirect descriptors, event index or DMA-API bounce |
| `drivers/virtio/virtio_pci_legacy.c` | `kernel/src/drivers/virtio/virtio_pci_legacy.rs` | partial | find a virtio function, take BAR0 I/O, status handshake, feature negotiation, `setup_vq`/notify; modern transport and MSI-X pending |
| `drivers/char/hw_random/virtio-rng.c` | `kernel/src/drivers/char/hw_random/virtio_rng.rs` | partial | probe and a synchronous polled `read`; no `hwrng` registration or completion interrupt |
| `drivers/char/mem.c` | `kernel/src/drivers/char/mem.rs` | partial | the `/dev/null`, `/dev/zero` and `/dev/full` devices (read/write `file_operations`), grafted into `/dev`; no `/dev/mem`, `/dev/port` or mmap |
| `drivers/char/random.c` | `kernel/src/drivers/char/random.rs` | partial | `/dev/random`, `/dev/urandom` and `get_random_bytes()` backed by the ported `prandom` seeded from TSC+RTC; the `getrandom(2)` syscall is in `kernel/src/kernel/syscalls.rs`; NOT a CSPRNG and never blocks (the real CRNG is future work) |
| `include/uapi/linux/random.h` | `kernel/src/kernel/syscalls.rs` | partial | the `GRND_NONBLOCK`/`GRND_RANDOM`/`GRND_INSECURE` flags for `getrandom(2)` |
| `drivers/char/virtio_console.c` | `kernel/src/drivers/char/virtio_console.rs` | partial | bind a virtio-console, run the multiport control handshake (`DEVICE_READY`/`PORT_ADD`/`PORT_READY`/`PORT_OPEN`), and transmit and receive on port 0's queues; no hvc/tty layer or interrupts |
| `include/uapi/linux/virtio_console.h` | `kernel/src/include/uapi/virtio_console.rs` | partial | feature bits, config field offsets and the control-message events/struct size |
| `drivers/input/serio/i8042.c` | `kernel/src/drivers/input/serio/i8042.rs` | partial | the command/status polling, the controller/keyboard self-tests, and the data-port read/write the keyboard driver uses; no serio bus or AUX/MUX |
| `drivers/input/keyboard/atkbd.c` | `kernel/src/drivers/input/keyboard/atkbd.rs` | partial | IRQ 1 handler, i8042 config-byte setup (interrupt/port/translation) and a scancode-set-1 US keymap; no `input`/`serio` subsystem, modifiers, repeat or LEDs |
| `drivers/tty/serial/8250/8250_port.c` | `kernel/src/drivers/tty/serial/serial8250.rs` | partial | the autoconfig scratch-register test and the internal-loopback check; no tty/uart layer, interrupts or FIFO sizing |
| `drivers/input/serio/i8042-io.h` | `kernel/src/drivers/input/serio/i8042.rs` | partial | the 0x60/0x64 port access |
| `include/linux/i8042.h` | `kernel/src/drivers/input/serio/i8042.rs` | partial | the command/status-bit and self-test constants |
| `include/uapi/linux/virtio_blk.h` | `kernel/src/include/uapi/virtio_blk.rs` | partial | the request header, IN/OUT types, status bytes and the capacity config field; feature bits, discard/zeroes and zoned requests pending |
| `include/linux/blkdev.h` | `kernel/src/block/mod.rs` | partial | a `BlockDevice` trait (the role of `block_device_operations`): synchronous sector reads; no `bio`/`request_queue`, gendisk, scheduler or writes |
| `block/partitions/msdos.c` | `kernel/src/block/partitions/msdos.rs` | partial | parse the MBR signature and the four primary partition entries; no extended/logical partitions, subformat probes or GPT |
| `block/partitions/efi.c` | `kernel/src/block/partitions/efi.rs` | partial | parse the primary GPT header (CRC-32 validated) and the partition-entry array; no backup header, repair or type-GUID handling |
| `block/partitions/efi.h` | `kernel/src/block/partitions/efi.rs` | partial | the GPT header and partition-entry field offsets |
| `drivers/block/virtio_blk.c` | `kernel/src/drivers/block/virtio_blk.rs` | partial | `virtblk_probe` and sector reads via the three-descriptor request chain, polled; reads only, no blk-mq, interrupts or partitions |
| `fs/ext2/ext2.h` | `kernel/src/fs/ext2/layout.rs` | partial | on-disk superblock, group descriptor, inode and dir entry, plus the constants; feature-flag and ext3/4 fields omitted |
| `fs/ext2/super.c` | `kernel/src/fs/ext2/mod.rs` | partial | `ext2_fill_super`: read and validate the superblock, compute the geometry; no feature checks, remount or writeback |
| `fs/ext2/inode.c` | `kernel/src/fs/ext2/mod.rs` | partial | `ext2_iget` (on-disk read) and `ext2_get_block` for direct and single-indirect blocks; no page cache, double/triple indirect or writes |
| `fs/ext2/dir.c` | `kernel/src/fs/ext2/mod.rs` | partial | `ext2_readdir`/`ext2_lookup`: walk directory blocks and resolve a name; no htree, no creation |
| `fs/ext4/ext4_extents.h` | `kernel/src/fs/ext4/ext4_extents.rs` | partial | the extent-tree header, leaf extent and index entry, the magic and `EXT4_EXTENTS_FL` |
| `fs/ext4/super.c` | `kernel/src/fs/ext4/mod.rs` | partial | read and validate the superblock (shares ext2's layout); no feature/journal/csum checks |
| `fs/ext4/inode.c` | `kernel/src/fs/ext4/mod.rs` | partial | read an inode and `ext4_map_blocks` dispatch (extents vs the classic indirect map); read-only |
| `fs/ext4/extents.c` | `kernel/src/fs/ext4/mod.rs` | partial | `ext4_ext_map_blocks`: walk the extent tree (leaf and index nodes) to map a file block; no allocation, splitting or uninitialized-extent conversion |
| `fs/ext4/dir.c` | `kernel/src/fs/ext4/mod.rs` | partial | `ext4_readdir`/lookup over directory blocks; no htree |
| `include/uapi/linux/msdos_fs.h` | `kernel/src/fs/fat/mod.rs` | partial | the BPB and directory-entry offsets and the attribute bits the FAT32 reader needs |
| `fs/fat/inode.c` | `kernel/src/fs/fat/mod.rs` | partial | `fat_fill_super`: parse and validate the FAT32 boot sector; FAT32 only, read-only |
| `fs/fat/fatent.c` | `kernel/src/fs/fat/mod.rs` | partial | `fat_ent_read`: follow a cluster chain through the FAT; no FAT cache or allocation |
| `fs/fat/dir.c` | `kernel/src/fs/fat/mod.rs` | partial | walk 32-byte directory entries matching short 8.3 names; VFAT long names are skipped |
| `include/uapi/linux/virtio_net.h` | `kernel/src/include/uapi/virtio_net.rs` | partial | the 10-byte packet header, the MAC feature bit and config offset; offload, mergeable buffers, control queue and multiqueue pending |
| `include/uapi/linux/if_ether.h` | `kernel/src/include/uapi/if_ether.rs` | partial | address/header lengths and the EtherTypes the stack uses |
| `include/uapi/linux/if_arp.h` | `kernel/src/include/uapi/if_arp.rs` | partial | the Ethernet hardware type and request/reply opcodes |
| `drivers/net/virtio_net.c` | `kernel/src/drivers/net/virtio_net.rs` | partial | probe, read the MAC, and send/receive frames over the rx/tx queues, polled; reset the device on teardown (as the remove path does) so a late packet cannot DMA into freed rings; no NAPI/interrupts, offload, mergeable buffers or `net_device` registration |
| `net/ethernet/eth.c` | `kernel/src/net/ethernet.rs` | partial | build an Ethernet header and read the EtherType/payload; no `eth_type_trans` classification or VLAN |
| `net/ipv4/arp.c` | `kernel/src/net/arp.rs` | partial | build an ARP request and parse a reply (Ethernet/IPv4); no ARP cache, gratuitous ARP or proxy |
| `include/uapi/linux/in.h` | `kernel/src/include/uapi/in.rs` | partial | the `IPPROTO_*` numbers; the sockaddr_in and socket option definitions pending |
| `include/uapi/linux/ip.h` | `kernel/src/include/uapi/ip.rs` | partial | IPv4 header field offsets and constants (the C struct uses bitfields, so the header is built field by field) |
| `include/uapi/linux/icmp.h` | `kernel/src/include/uapi/icmp.rs` | partial | echo request/reply types and the 8-byte header offsets |
| `lib/checksum.c` | `kernel/src/net/checksum.rs` | partial | the portable Internet checksum (`csum_partial`/`csum_fold`/`ip_compute_csum`); the per-arch assembly is not ported |
| `net/ipv4/ip_output.c` | `kernel/src/net/ipv4/mod.rs` | partial | build a fixed 20-byte IPv4 header and `ip_send_check`; no options, routing or fragmentation |
| `net/ipv4/ip_input.c` | `kernel/src/net/ipv4/mod.rs` | partial | `ip_rcv_core`: validate version/IHL and the header checksum, return the header; no options or reassembly |
| `net/ipv4/icmp.c` | `kernel/src/net/ipv4/icmp.rs` | partial | build an echo request and parse an echo reply; no error messages or rate limiting |
| `include/uapi/linux/udp.h` | `kernel/src/include/uapi/udp.rs` | partial | the 8-byte UDP header offsets |
| `net/ipv4/udp.c` | `kernel/src/net/ipv4/udp.rs` | partial | build a UDP header with the IPv4 pseudo-header checksum and read the ports; no socket layer, ports table or GRO |
| `net/ipv4/ipconfig.c` | `kernel/src/net/ipv4/ipconfig.rs` | partial | the DHCP client: build DISCOVER/REQUEST and parse OFFER/ACK options (your-IP, server, subnet, router, DNS); no BOOTP/RARP fallback or retry machine |
| `include/uapi/linux/tcp.h` | `kernel/src/include/uapi/tcp.rs` | partial | the 20-byte header field offsets and the `TCPHDR_*` control-flag bits |
| `net/ipv4/tcp_output.c` | `kernel/src/net/ipv4/tcp.rs` | partial | build a segment with the pseudo-header checksum (`tcp_v4_send_check`); no options, windows, congestion control or retransmission |
| `net/ipv4/tcp_input.c` | `kernel/src/net/ipv4/tcp.rs` | partial | parse a received segment's seq/ack/flags/payload; the state machine (`tcp_rcv_state_process`) is driven lock-step by the caller, not ported |
| `include/linux/socket.h` | `kernel/src/include/uapi/socket.rs` | partial | the `AF_INET` family and `struct sockaddr_in`; the other families and ancillary data pending |
| `include/linux/net.h` | `kernel/src/include/uapi/socket.rs` | partial | the `SOCK_STREAM`/`SOCK_DGRAM` types; the `struct socket` state machine pending |
| `net/socket.c` | `kernel/src/net/socket.rs` | partial | a datagram `UdpSocket` (bind/send_to/recv_from), the role of `sock_create`/`sock_sendmsg`/`sock_recvmsg`; no fd table, poll or the generic sock split |
| `net/ipv4/af_inet.c` | `kernel/src/net/socket.rs` | partial | the inet datagram path (`inet_bind`/`udp_sendmsg`/`udp_recvmsg`) for one socket; no port table, routing or connected sockets |
| `include/linux/mc146818rtc.h` | `kernel/src/include/linux/mc146818rtc.rs` | partial | the CMOS index/data ports, clock register numbers and the UIP/BCD/24H control bits |
| `drivers/rtc/rtc-cmos.c` | `kernel/src/drivers/rtc/rtc_cmos.rs` | partial | read the clock through the CMOS ports; no alarms, periodic/update interrupts or NVRAM, and read-only |
| `drivers/rtc/rtc-mc146818-lib.c` | `kernel/src/drivers/rtc/rtc_cmos.rs` | partial | `mc146818_get_time`: wait out the update cycle and convert from BCD / 12-hour form; no century register |
| `lib/memweight.c` | `lib/src/memweight.rs` | done | byte-wise count; the word-at-a-time `bitmap_weight()` path is not ported |
| `include/linux/union_find.h` | `lib/src/union_find.rs` | done | `UfNode` with index parents in place of pointers |
| `lib/union_find.c` | `lib/src/union_find.rs` | done | `uf_find` (path halving) and `uf_union` (by rank); host tests cover singletons, ranks, compression and a random cross-check against a naive partition |
| `lib/packing.c` | `lib/src/packing.rs` | partial | `pack`/`unpack`/`packing` with `QUIRK_*`; the `pack_fields_*`/`unpack_fields_*` struct helpers need raw struct pointers and are not ported; the overflow WARN is dropped |
| `include/linux/packing.h` | `lib/src/packing.rs` | partial | the quirk bits and `enum packing_op`; the `PACKED_FIELD` macros go with the struct helpers |
| `lib/packing_test.c` | `lib/tests/packing_kunit.rs` | test | the 34-case pack and unpack table, every entry kept; the field-helper cases pending |
| `lib/linear_ranges.c` | `lib/src/linear_ranges.rs` | done | the selector/value lookups; the `*found` and `*selector` outputs are `&mut` parameters |
| `include/linux/linear_range.h` | `lib/src/linear_ranges.rs` | done | `LinearRange` and `LINEAR_RANGE()` as `LinearRange::new` |
| `lib/tests/test_linear_ranges.c` | `lib/tests/linear_ranges_kunit.rs` | test | the two-range fixture and the four KUnit cases, values and selectors as in the C |
| `lib/clz_ctz.c` | `lib/src/clz_ctz.rs` | done | the four GCC builtins; not called by kernel C, since Rust's bit intrinsics compile to the instruction |
| `lib/seq_buf.c` | `lib/src/seq_buf.rs` | partial | `SeqBuf`: puts, putc, putmem, putmem_hex, printf, hex_dump and `do_printk()` through a sink, with the overflow marker; seq_file, binary printf, paths and user copies wait for those subsystems |
| `include/linux/seq_buf.h` | `lib/src/seq_buf.rs` | partial | the buffer, used/left and overflow helpers; the `DECLARE_SEQ_BUF` macro is a constructor |
| `lib/iommu-helper.c` | `lib/src/iommu_helper.rs` | done | `iommu_area_alloc()` returns `Option<usize>` for the C `-1`; `iommu_is_span_boundary()` and `iommu_device_max_index()` from the header |
| `lib/check_signature.c` | `lib/src/check_signature.rs` | done | the byte comparison over a caller-supplied `read(offset)`; the `ioremap` and `readb()` access stays with the caller |
| `lib/ashldi3.c` | `lib/src/libgcc.rs` | done | `ashldi3()`; the 32-bit halves algorithm as in C, shift counts masked to 6 bits |
| `lib/ashrdi3.c` | `lib/src/libgcc.rs` | done | `ashrdi3()`, arithmetic shift |
| `lib/lshrdi3.c` | `lib/src/libgcc.rs` | done | `lshrdi3()`, logical shift |
| `lib/cmpdi2.c` | `lib/src/libgcc.rs` | done | `cmpdi2()`, signed compare returning 0/1/2 |
| `lib/ucmpdi2.c` | `lib/src/libgcc.rs` | done | `ucmpdi2()`, unsigned compare returning 0/1/2 |
| `include/linux/libgcc.h` | `lib/src/libgcc.rs` | done | `DWunion` as split halves |
| `lib/net_utils.c` | `lib/src/net_utils.rs` | done | `mac_pton()` returns `Option<[u8; 6]>`, so a bad string writes nothing |
| `lib/clz_tab.c` | `lib/src/clz_tab.rs` | done | the 256-byte table built at compile time from its rule (bit length of the index) |
| `lib/memcat_p.c` | `lib/src/memcat_p.rs` | done | `memcat_p()` over slices into a caller buffer; the NULL terminators become slice ends |
| `include/linux/string.h` | `lib/src/memcat_p.rs` | done | ``memcat_p``: the macro is the function|
| `lib/bust_spinlocks.c` | `lib/src/bust_spinlocks.rs` | done | the oops counter; the console unblank and klogd wake are passed in as closures until those subsystems exist |
| `lib/muldi3.c` | `lib/src/muldi3.rs` | done | `umul_ppmm()`, `umulsidi3()` and `muldi3()` as the same 16-bit-piece algorithm; the cross-term sum wraps as the C does |
| `lib/min_heap.c` | `lib/src/min_heap.rs` | done | `MinHeap<T>` over a caller slice with a `less` closure; the header's sift, heapify, pop, pop-push, push and delete algorithms, step for step |
| `include/linux/min_heap.h` | `lib/src/min_heap.rs` | done | the byte-offset and `min_heap_callbacks` machinery becomes the generic type and the closure |
| `lib/memregion.c` | `lib/src/memregion.rs` | done | `memregion_alloc()` and `memregion_free()` take the allocator as a parameter instead of a static |
| `lib/idr.c` | `lib/src/ida.rs` | partial | ``ida_*``: `Ida<N>`: `ida_alloc()`, `ida_alloc_range()` and `ida_free()` over a fixed bitmap; the radix tree and the IDR pointer mapping wait|
| `lib/crypto/arc4.c` | `lib/src/crypto/arc4.rs` | partial | `Arc4Ctx::new()` (`arc4_setkey()`) and `crypt()` (`arc4_crypt()`), checked against published vectors; the `ecb(arc4)` lskcipher registration in `crypto/arc4.c` waits for the crypto API |
| `include/crypto/arc4.h` | `lib/src/crypto/arc4.rs` | done | the key-size limits and block size as constants |
| `lib/crypto/chacha.c` | `lib/src/crypto/chacha.rs` | done | `ChachaState::new()`, `block()`, `crypt()` and `hchacha_block()` as the generic code; the SIMD versions compute the same bytes and are not ported |
| `lib/crypto/chacha-block-generic.c` | `lib/src/crypto/chacha.rs` | done | `chacha_permute()` and the block and HChaCha functions |
| `include/crypto/chacha.h` | `lib/src/crypto/chacha.rs` | done | the sizes and the constants; the `chacha_init()` helper is `ChachaState::new()` |
| `lib/crypto/mpi/mpih-cmp.c` | `lib/src/crypto/mpi.rs` | done | `mpihelp_cmp()`, most significant limb first |
| `lib/crypto/mpi/generic_mpih-add1.c` | `lib/src/crypto/mpi.rs` | done | `mpihelp_add_n()` with the two-step carry, returning the carry out |
| `lib/crypto/mpi/generic_mpih-sub1.c` | `lib/src/crypto/mpi.rs` | done | `mpihelp_sub_n()` with the two-step borrow, returning the borrow out |
| `lib/crypto/mpi/generic_mpih-lshift.c` | `lib/src/crypto/mpi_shift.rs` | done | `mpihelp_lshift()`, returning the bits shifted out of the top limb |
| `lib/crypto/mpi/generic_mpih-rshift.c` | `lib/src/crypto/mpi_shift.rs` | done | `mpihelp_rshift()`, returning the bits shifted out of the bottom limb |
| `lib/crypto/mpi/generic_mpih-mul1.c` | `lib/src/crypto/mpi_mul.rs` | done | `mpihelp_mul_1()`, storing the product and returning the carry limb |
| `lib/crypto/mpi/generic_mpih-mul2.c` | `lib/src/crypto/mpi_mul.rs` | done | `mpihelp_addmul_1()`, adding the product into `res` |
| `lib/crypto/mpi/generic_mpih-mul3.c` | `lib/src/crypto/mpi_mul.rs` | done | `mpihelp_submul_1()`, subtracting the product from `res` |
| `lib/crypto/mpi/mpi-cmp.c` | `lib/src/crypto/mpi_obj.rs` | done | `mpi_cmp()` and `mpi_cmp_ui()` on the `Mpi` view |
| `include/linux/mpi.h` | `lib/src/crypto/mpi_obj.rs` | partial | ``struct gcry_mpi``: `Mpi` borrows its limbs; `alloced`, flags and the allocating constructors wait for an allocator|
| `lib/crypto/mpi/mpi-bit.c` | `lib/src/crypto/mpi_obj.rs` | partial | ``mpi_normalize``: `Mpi::normalize()`; the bit-count and shift functions of mpi-bit.c are not ported|
| `lib/crypto/hash_info.c` | `lib/src/crypto/hash_info.rs` | done | `HashAlgo::name()` and `HashAlgo::digest_size()` as matches on the enum, in the C enum order |
| `include/uapi/linux/hash_info.h` | `lib/src/crypto/hash_info.rs` | done | `enum hash_algo` as `HashAlgo` |
| `lib/crypto/mpi/mpi-sub-ui.c` | `lib/src/crypto/mpi_sub_ui.rs` | done | `mpi_sub_ui()` with fixed storage: `mpi_resize()` becomes an `ENOMEM` check; the limb add and subtract loops are inline until `mpih-add1`/`sub1` helpers land |
| `lib/crypto/mpi/mpi-add.c` | `lib/src/crypto/mpi_add.rs` | done | `mpi_add()` and `mpi_sub()` with fixed storage: `RESIZE_IF_NEEDED` becomes an `ENOMEM` check, and `mpi_sub` flips the sign of `v` instead of copying it; the general limb add and sub are inline until `mpih-add`/`sub` land. `mpi_addm`/`mpi_subm` wait for `mpi_mod` |
| `lib/crypto/mpi/mpi-bit.c` | `lib/src/crypto/mpi_bit.rs` | partial | `get_nbits()`, `test_bit()`, `set_bit()` (ENOMEM beyond storage) and `mpi_rshift()`, with the in-place mode left to the caller; host tests against u128 bit arithmetic |
| `lib/crypto/mpi/mpih-mul.c` | `lib/src/crypto/mpi_sqr.rs` | partial | ``mul_n_basecase`, `mpih_sqr_n_basecase``: the schoolbook product and its squaring; the Karatsuba `mul_n`, `mpih_sqr_n` and `mpih_mul` wait|
| `include/linux/percpu-defs.h` | `lib/src/percpu.rs` | partial | ``DEFINE_PER_CPU`, `per_cpu_ptr``: `PerCpu<T, N>`: one slot per CPU with `get_mut()` (safe) and `with_cpu()` (unsafe, with the this_cpu rule); the kernel does not use it yet, as rinux runs one CPU and the per-CPU offsets wait|
| `lib/irq_regs.c` | `kernel/src/kernel/irq/regs.rs` | done | `get_irq_regs()` and `set_irq_regs()` over a `PerCpu` slot holding the frame address; the `DEFINE_PER_CPU` storage is the slot |
| `include/asm-generic/irq_regs.h` | `kernel/src/kernel/irq/regs.rs` | done | the accessors; the `Option` return stands in for the null pointer |
| `lib/lockref.c` | `lib/src/lockref.rs` | done | `Lockref` with lock and count in one `AtomicU64`; the `CMPXCHG_LOOP` with 100 retries and the lock fallback; `put_return()` and `put_or_lock()` keep C's return conventions |
| `include/linux/lockref.h` | `lib/src/lockref.rs` | done | `LOCKREF_DEAD_VAL` and the `mark_dead()` helper |
| `lib/lwq.c` | `lib/src/lwq.rs` | partial | `enqueue()` is a lock-free `cmpxchg` push onto the producer stack; `dequeue()` and `dequeue_all()` take a spinlock and reverse the stack into arrival order, with the `TRANSFER` marker as C's `(void *)1`; the `lwq_for_each_safe` macro is not ported |
| `include/linux/lwq.h` | `lib/src/lwq.rs` | partial | the `lwq_empty`, `lwq_enqueue` and `lwq_dequeue` operations; the atomics are not modelled |
| `lib/dec_and_lock.c` | `lib/src/dec_and_lock.rs` | partial | `atomic_dec_and_lock()` over an `AtomicI32` and an `AtomicBool` spinlock; the `irqsave` and `raw` variants wait for the kernel's interrupt and raw-lock wrappers |
| `lib/crypto/utils.c` | `lib/src/crypto/utils.rs` | done | `crypto_xor_cpy()` and `crypto_xor()`; the word-stride paths are speed-only and are not ported |
| `lib/crypto/memneq.c` | `lib/src/crypto/memneq.rs` | done | `crypto_memneq()` generic loop, OR-accumulating the XOR of each byte pair, with `black_box` in place of `OPTIMIZER_HIDE_VAR`; the arch versions compute the same result |
| `lib/crypto/blake2s.c` | `lib/src/crypto/blake2s.rs` | partial | the generic compression, `update()` and `final()` for unkeyed BLAKE2s-256 and shorter outputs, checked against the RFC 7693 vectors; the keyed init and the arch versions wait |
| `include/crypto/blake2s.h` | `lib/src/crypto/blake2s.rs` | partial | `blake2s_init()` (unkeyed); `blake2s_init_key()` waits |
| `lib/crypto/blake2b.c` | `lib/src/crypto/blake2b.rs` | partial | the generic 64-bit compression, `update()` and `finalize()` for unkeyed BLAKE2b, checked against the RFC 7693 vectors; the keyed init and arch versions wait |
| `include/crypto/blake2b.h` | `lib/src/crypto/blake2b.rs` | partial | `Blake2bCtx::new()` (unkeyed); the keyed init waits |
| `lib/crypto/poly1305-donna64.c` | `lib/src/crypto/poly1305.rs` | done | the 44-bit limb core: `poly1305_core_setkey()`, `blocks()`, `emit()`, with u128 products as in donna |
| `lib/crypto/poly1305.c` | `lib/src/crypto/poly1305.rs` | done | `poly1305()` one-shot, with the final partial block padded by a 1 bit and no 2^128 bit, as C does |
| `lib/crypto/poly1305-donna32.c` | `lib/src/crypto/poly1305_donna32.rs` | done | the 26-bit limb core, with `poly1305_tag()` as the one-shot composed from it; checked against the 44-bit core |
| `lib/crypto/nh.c` | `lib/src/crypto/nh.rs` | done | `nh()`, the generic loop (the arch hook returns false); output bytes via `nh_bytes()` |
| `include/crypto/nh.h` | `lib/src/crypto/nh.rs` | done | the unit and pass constants |
| `lib/crypto/sm3.c` | `lib/src/crypto/sm3.rs` | done | `Sm3Ctx` init, update and finalize, and `sm3()`; the 64 rounds are a loop with the constant rotated by the index, not the unrolled macros |
| `crypto/sm3.c` | — | not ported | the Crypto API shash wrapper (registration and export/import) waits for the crypto API |
| `crypto/sm4.c` | `lib/src/crypto/sm4.rs` | partial | `sm4_expandkey()` and `sm4_crypt_block()` with the S-box copied from the C table; the Crypto API `sm4_alg` registration waits |
| `include/crypto/sm4.h` | `lib/src/crypto/sm4.rs` | partial | the block and key sizes, and the round-key context |
| `lib/earlycpio.c` | `lib/src/earlycpio.rs` | done | `find_cpio_data()` over byte offsets, parsing the newc hex fields as C does and returning name and data as slices; host tests build archives and check lookup, next offset, a missing name, bad magic and directories |
| `lib/cmpxchg-emu.c` | `lib/src/cmpxchg_emu.rs` | done | `cmpxchg_emu_u8()` as a compare-and-swap of one byte inside an `AtomicU32` word, little-endian as the C union; host tests cover the position, mismatch, word crossing and four threads on neighbouring bytes |
| `lib/bucket_locks.c` | `lib/src/bucket_locks.rs` | done | `bucket_locks_size()` (the sizing rule and mask) and `BucketLocks` over caller-provided spinlock flags, with `lock(hash)`; the kvmalloc and lockdep init stay with the caller |
| `lib/interval_tree.c` | `lib/src/interval_tree.rs` | partial | insert, remove, subtree search, `iter_first()` and `iter_next()` on the augmented rbtree; the span iterator (`CONFIG_INTERVAL_TREE_SPAN_ITER`) waits; the leftmost node is found by walking rather than cached |
| `include/linux/interval_tree_generic.h` | `lib/src/interval_tree.rs` | done | the generic algorithms and the `RB_DECLARE_CALLBACKS_MAX` callbacks for the subtree maximum |
| `lib/strncpy_from_user.c` | `lib/src/strncpy_from_user.rs` | done | `strncpy_from_user()` over `UserMemory` (`strncpy_from_user_mem()`); the word-at-a-time fast path is an optimisation with the same result and is not ported |
| `lib/strnlen_user.c` | `lib/src/strnlen_user.rs` | done | `strnlen_user()` with C's convention (length plus NUL, count plus one when too long, 0 on fault), over `UserMemory` (`strnlen_user_mem()`); the word search is an optimisation and is not ported |
| `lib/usercopy.c` | `lib/src/usercopy.rs` | done | `check_zeroed_user()` with C's early exit and its -EFAULT, over `UserMemory` (`check_zeroed_user_mem()`); the `_copy_from_user()` and `_copy_to_user()` wrappers are the `UserMemory` trait in `lib/src/uaccess.rs` |
| `lib/iomap_copy.c` | `lib/src/iomap_copy.rs` | done | `iowrite32_copy()`, `ioread32_copy()` and `iowrite64_copy()` with the raw register accesses as closures; word-at-a-time, one access per word as C |
| `lib/iomem_copy.c` | `lib/src/iomem_copy.rs` | done | `memset_io()`, `memcpy_fromio()` and `memcpy_toio()` as byte operations over closures; the word-width bus accesses are left to the kernel's accessors |
| `lib/decompress.c` | `lib/src/decompress.rs` | partial | `decompress_method()` as the magic table naming the method (gzip, bzip2, lzma, xz, lzo, lz4, zstd); the decompressors themselves are separate files and wait |
| `lib/extable.c` | `lib/src/extable.rs` | partial | `sort_extable()` as an in-place heapsort and `search_extable()` by binary search, over absolute addresses; C's relative x86 encoding is a build-time layout that the kernel's linker produces |
| `lib/stmp_device.c` | `lib/src/stmp_device.rs` | done | `stmp_reset_block()` over a `StmpRegs` trait for the register access and the 1 µs delay; the SET/CLR offsets and gate bits are the header's; ETIMEDOUT on a stuck poll |
| `include/linux/stmp_device.h` | `lib/src/stmp_device.rs` | done | the offsets and bit masks |
| `lib/errname.c` | `lib/src/errname.rs` | done | `errname()` returns an `ErrName` that writes the sign through `Display`; the MIPS-only `EDQUOT` entry is not ported |
| `lib/argv_split.c` | `lib/src/argv_split.rs` | done | the word ranges as an iterator; the copied, NUL-terminated argv and `argv_free()` stay with the caller |
| `lib/errseq.c` | `lib/src/errseq.rs` | done | `errseq_set()`, `errseq_sample()`, `errseq_check()` and `errseq_check_and_advance()` over an `AtomicU32`; the `WARN()` on a bad error becomes an ignored value |
| `include/linux/errseq.h` | `lib/src/errseq.rs` | done | `Errseq` as `u32`; the shift, `SEEN` and counter macros as constants |
| errseq tests | `lib/tests/errseq_test.rs` | test | host tests for the sample/set/check protocol and the counter rules |
| `lib/oid_registry.c` | `lib/src/oid_registry.rs` | done | `look_up_oid()`, `sprint_oid()` (snprintf truncation as `Snprintf`); `parse_oid()` only checks the TLV, as the C never sets its output |
| `include/linux/oid_registry.h` | `lib/src/oid_registry_data.rs` | done | the `Oid` enum and the names, generated by `scripts/gen_oid_registry.py` |
| `lib/oid_registry_data.c` (generated at build) | `lib/src/oid_registry_data.rs` | done | `generated at build`: the search table, index and data, generated with the same hash and sort order|
| OID registry tests | `lib/tests/oid_registry_test.rs` | test | every entry round-trips through lookup and printing; an RSA OID by hand; truncation, malformed content and TLV checks |
| `lib/list_debug.c` | `lib/src/list_debug.rs` | done | `list_add_valid_or_report()` and `list_del_entry_valid_or_report()` returning the first failed check as `ListCorruption`; the `CHECK_DATA_CORRUPTION` printk and stack dump are left to the caller |
| list debug tests | `lib/tests/list_debug_test.rs` | test | a valid ring passes; each add and del check fires in C order with the poison values |
| `lib/debug_locks.c` | `lib/src/debug_locks.rs` | done | `DebugLocks::off()` in place of `debug_locks_off()`; the console call is a closure, and the `debug_locks` and `debug_locks_silent` globals are fields with the `DEBUG_LOCKS` instance |
| debug locks tests | `lib/tests/debug_locks_test.rs` | test | first and later switch-offs, silent switch-off, the no-op repeat, and the silent setting round trip |
| `lib/atomic64.c` | `lib/src/atomic64.rs` | done | the generic spinlock fallback is replaced by `AtomicI64`; `fetch_*` returns the old value and `*_return` the new one |
| atomic64 tests | `lib/tests/atomic64_test.rs` | test | each operation's result, `dec_if_positive` at zero, `add_unless` and `inc_not_zero`, and a 4-thread count |
| `lib/dim/dim.c` | `lib/src/dim.rs` | done | `dim_calc_stats()`, `dim_on_top()`, `dim_turn()`, `dim_park_on_top()` and `dim_park_tired()`; the `u32` wrap of the event counter and the 32-bit `BIT_GAP` are kept as C computes them |
| `lib/dim/rdma_dim.c` | `lib/src/dim.rs` | done | `Dim::rdma_dim()` and its decision, step and stats-compare helpers, with the same early exits as the C switch |
| `include/linux/dim.h` | `lib/src/dim.rs` | partial | `DIM helpers`: the enums, `struct dim`, the sample helpers and the macros; the net DIM profile API and the irq-moderation calls wait for `net/`|
| DIM tests | `lib/tests/dim_test.rs` | test | the 10% significance rule, the window arithmetic, the turn and park rules, and the RDMA windows: a climb after a 10%+ gain, the edge turn, the start sample kept before 64 events, and the u16 counter wrap |
| `lib/zstd/common/error_private.c` | `lib/src/zstd_error.rs` | done | `err_get_error_string()` with the message for every enum value and the unnamed default; the `ZSTD_STRIP_ERROR_STRINGS` build is not ported |
| `include/linux/zstd_errors.h` | `lib/src/zstd_error.rs` | done | the codes as `u32` constants, `is_error()` and `get_error_code()` on the `size_t` error encoding |
| zstd error tests | `lib/tests/zstd_error_test.rs` | test | every named message, the default for unnamed codes, distinct messages, and the error range |
| `lib/tests/test_bits.c` | `lib/tests/test_bits_test.rs` | test | the `BIT_*()` and `GENMASK*()` cases, with the typed 128-bit ones; the `TEST_GENMASK_FAILURES` compile-fail cases and `GENMASK_INPUT_CHECK()` wait |
| `lib/raid/raid6/mktables.c` | `lib/src/raid6.rs` | done | `gfmul()`, `gfpow()` and the `raid6_gf*` tables as const items; the 256-entry tables are checked against the field axioms |
| int.uc (template, not tracked as C) | `lib/src/raid6.rs` | done | `gen_syndrome()` and `xor_syndrome()` for every unroll width (the widths give the same bytes); byte-wise, not word-wise |
| `lib/raid/raid6/recov.c` | `lib/src/raid6.rs` | done | `recov_2data()` and `recov_datap()` with the same parity equations; the P that `recov_datap()` writes is the XOR of all data, so the C aliasing of `p` is not reproduced |
| `lib/raid/xor/xor-8regs.c` | `lib/src/raid_xor.rs` | done | `xor_2()` to `xor_5()` over eight-word groups; the `DO_XOR_BLOCKS` template is the four functions |
| `lib/fw_table.c` | `lib/src/fw_table.rs` | partial | `acpi_parse_entries_array()` and `cdat_table_parse()` over a byte slice, for the common ACPI and CDAT subtable layouts; the HMAT, PRMT and CEDT layouts are not ported (`EOPNOTSUPP`), and the handler gets the entry bytes, not the table end |
| `scripts/dtc/libfdt/fdt.c` | `lib/src/libfdt.rs` | done | header checks, the structure walk, node and subnode iteration, `find_string_len`, `move_to`; the `can_assume()` fast paths are not ported |
| `scripts/dtc/libfdt/fdt_ro.c` | `lib/src/libfdt.rs` | done | every read-only lookup as a method of `Fdt`; C's negative returns are `Result<_, FdtError>`, and the string-list helpers are free functions |
| `scripts/dtc/libfdt/fdt_strerror.c` | `lib/src/libfdt.rs` | done | `strerror()` and `FdtError::name()`, with the table in code order |
| `scripts/dtc/libfdt/fdt_rw.c` | `lib/src/libfdt.rs` | done | `FdtMut` with the reservation, name, property, node and NOP edits, `pack`, and `open_into` (separate source and destination) |
| `scripts/dtc/libfdt/fdt_sw.c` | `lib/src/libfdt.rs` | done | `FdtSw` for the sequential builder, and `sw_resize` (separate source and destination) |
| `scripts/dtc/libfdt/fdt_wip.c` | `lib/src/libfdt.rs` | done | in-place value edits and NOP tags on `FdtMut` |
| `scripts/dtc/libfdt/fdt_addresses.c` | `lib/src/libfdt.rs` | done | `address_cells`, `size_cells` and `FdtMut::appendprop_addrrange` |
| `scripts/dtc/libfdt/fdt_empty_tree.c` | `lib/src/libfdt.rs` | done | `create_empty_tree` |
| `lib/fdt.c` | `lib/src/libfdt.rs` | done | wrapper that includes `scripts/dtc/libfdt/fdt.c` |
| `lib/fdt_addresses.c` | `lib/src/libfdt.rs` | done | wrapper that includes `scripts/dtc/libfdt/fdt_addresses.c` |
| `lib/fdt_empty_tree.c` | `lib/src/libfdt.rs` | done | wrapper that includes `scripts/dtc/libfdt/fdt_empty_tree.c` |
| `lib/fdt_ro.c` | `lib/src/libfdt.rs` | done | wrapper that includes `scripts/dtc/libfdt/fdt_ro.c` |
| `lib/fdt_rw.c` | `lib/src/libfdt.rs` | done | wrapper that includes `scripts/dtc/libfdt/fdt_rw.c` |
| `lib/fdt_strerror.c` | `lib/src/libfdt.rs` | done | wrapper that includes `scripts/dtc/libfdt/fdt_strerror.c` |
| `lib/fdt_sw.c` | `lib/src/libfdt.rs` | done | wrapper that includes `scripts/dtc/libfdt/fdt_sw.c` |
| `lib/fdt_wip.c` | `lib/src/libfdt.rs` | done | wrapper that includes `scripts/dtc/libfdt/fdt_wip.c` |
| `include/linux/fw_table.h` | `lib/src/fw_table.rs` | partial | `SubtableProc` in place of `struct acpi_subtable_proc`, with the handler as a closure; the `handler_arg` and `arg` pair folds into the closure |
| `lib/rcuref.c` | `lib/src/rcuref.rs` | done | `rcuref_get()`, `rcuref_put()`, `rcuref_put_rcusafe()` and both slow paths; the preemption guard and `WARN_ONCE()` are not reproduced |
| `include/linux/rcuref.h` | `lib/src/rcuref.rs` | done | the `RCUREF_*` constants, `RcuRef` in place of `rcuref_t`, with `new`, `read` and `is_dead` |
| RAID tests | `lib/tests/raid6_test.rs`, `lib/tests/raid_xor_test.rs` | test | table axioms, syndromes against a direct sum, partial updates, every two-disk and one-disk failure, and the XOR counts |
