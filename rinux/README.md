# rinux

rinux is a Rust port of the Linux kernel in `../linux`, done one step at a
time. Each step converts a set of C files, converts the kernel's own tests
for them, and records the result in [PORTING.md](PORTING.md).

The full kernel is about 65,900 C, header and assembly files, half of them
device drivers. The near-term goal is a Rust kernel that boots on x86-64 and
runs a BusyBox userland; it already runs a static `/init` in ring 3. The
roadmap in PORTING.md lists the steps toward it.

The kernel already boots: GRUB loads it under BIOS or UEFI, it switches to
64-bit mode and prints the Linux-style banner and memory map on the serial
and VGA consoles. It sets up its GDT, TSS and IDT, handles exceptions with
Linux-style oopses, takes PIT timer interrupts through the 8259, reads
keystrokes through an IRQ-1 PS/2 keyboard driver, calibrates the TSC for its
timestamps, reads the wall-clock time from the CMOS RTC, and checks its
ported SHA-256 against the FIPS 180-4 vectors. It then brings up memory management
— memblock, a direct-mapped page table (with page 0 unmapped so NULL
faults), a buddy page allocator, `kmalloc` and `vmalloc` — and links Rust's
`alloc`, so `Box` and `Vec` work. It starts the scheduler: the boot path
becomes the idle task `swapper/0`, and kernel threads run round-robin with
real context switches and timer-driven preemption. Finally it mounts a
ramfs root, brings up a small procfs, registers the `/dev/null`, `/dev/zero`
and `/dev/full` character devices, and unpacks a cpio initramfs that
GRUB loads as a module — so `/proc/version` reads back and files from the
initramfs appear under `/`. It enumerates the PCI bus over type-1 config
access, listing each function's vendor, class, BARs and IRQ, and binds a
virtio driver: it brings up a virtio-rng device through the legacy
virtio-pci transport and fetches random bytes over a real split virtqueue,
sends a line to a virtio-console port after the multiport control handshake,
and it reads a sector from a virtio-blk disk through a minimal block layer,
then mounts a real ext2 filesystem on another virtio-blk disk and reads a
file out of it, mounts an ext4 image on a third disk, reading a file mapped through the
extent tree, and mounts a FAT32 image on a fourth, reading a file through
its cluster chain. It brings up a virtio-net interface, resolves the gateway's MAC with an ARP
exchange over real transmit and receive queues, pings the gateway with an
ICMP echo request, leases an address over a full DHCP exchange (UDP), and
opens a TCP connection to a host server, completing the handshake and
fetching an HTTP response, and exchanges a datagram through a UDP socket.
It then enters user space: it programs the SYSCALL path, gives a task its
own page tables, loads `/init` from the initramfs with a ported ELF loader,
and runs it in ring 3, where it writes to the console, reads the system
identity with `uname`, opens and reads a file through the VFS, lists a
directory, reads the wall clock with `clock_gettime`, sleeps for a fixed
interval with `nanosleep` (measuring it with `gettimeofday`), draws random
bytes with `getrandom`, reports the uptime and memory with `sysinfo`, reads
its capability set with `capget`, its root identity with `getuid`/`getgid` and
its working directory with `getcwd`, makes the calls a libc makes at startup
(`getppid`, `set_tid_address`, `set_robust_list`, and `prlimit64` against a
resource-limit table that `open` enforces), waits on and wakes a `futex`, maps,
re-protects and unmaps anonymous memory with `mmap`/`mprotect`/`munmap`, sets up
thread-local storage through the FS base
with `arch_prctl`, installs a `SIGUSR1` handler with
`rt_sigaction` and raises it at itself — the kernel builds a signal frame on
the user stack, runs the handler, and `rt_sigreturn` resumes it — and exits
through the syscall layer. It idles where the ported code ends.

## Layout

```text
rinux/
├── Cargo.toml         workspace
├── PORTING.md         roadmap, conventions, and the per-file tracker
├── lib/               rinux-lib: Linux lib/ (no_std, no allocation)
│   ├── src/           one module per C file or group, naming its source
│   └── tests/         Rust versions of the kernel's KUnit and self tests
├── kernel/            the bootable kernel (own workspace, x86_64-unknown-none)
│   ├── src/           arch/, init/, kernel/, mm/, fs/, drivers/ as in the Linux tree
│   └── linker.ld      image layout, loaded at 1 MiB
└── scripts/
    ├── gen_tables.py  extracts data tables from the C sources
    ├── gen_consts.py  turns header #defines into Rust constants
    ├── build-iso.sh   builds ../rinux-boot.iso with GRUB
    ├── boot-test.sh   boots the ISO in QEMU and checks the serial log
    └── progress.py    counts converted files against ../linux
```

## Build and test

```bash
cd rinux
cargo build
cargo test
cargo clippy --all-targets
```

On this Windows machine an Application Control policy blocks freshly built
test executables, so `cargo test` fails with "os error 4551" even though
everything compiles. Run the tests under WSL instead, with a separate target
directory so the two toolchains don't share build output:

```bash
wsl.exe -d Ubuntu -- bash -lc 'cd /mnt/c/Users/bahda/code/inux/rinux && CARGO_TARGET_DIR=$HOME/.cache/rinux-target ~/.cargo/bin/cargo test'
```

## Boot the kernel

In WSL (Ubuntu with `grub-pc-bin grub-efi-amd64-bin xorriso mtools qemu-system-x86 ovmf`
and the Rust target `x86_64-unknown-none`), from the repository root:

```bash
bash rinux/scripts/build-iso.sh          # writes rinux-boot.iso
bash rinux/scripts/boot-test.sh          # BIOS boot in QEMU
bash rinux/scripts/boot-test.sh --uefi   # UEFI boot in QEMU
```

To watch it boot in a QEMU window instead:

```bash
qemu-system-x86_64 -m 256M -cdrom rinux-boot.iso -serial stdio
```

## Progress

```bash
python rinux/scripts/progress.py
```

prints how many of the kernel's C, header and assembly files have been
converted, in total and per top-level directory.

## Regenerating tables

Errno numbers and several lookup tables are lifted from the C sources
rather than retyped. After updating the linux submodule:

```bash
python rinux/scripts/gen_tables.py
python rinux/scripts/gen_consts.py
```

## License

GPL-2.0-only, like the kernel. Each file keeps the SPDX identifier of the
C file it was ported from.
