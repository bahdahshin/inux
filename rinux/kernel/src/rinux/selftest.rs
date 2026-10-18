// SPDX-License-Identifier: GPL-2.0
//! Boot-time checks with no Linux counterpart.
//!
//! Linux finds out that its timer works when the scheduler starts running
//! tasks. rinux has no tasks yet, so it checks directly and says so on the
//! console, which `boot-test.sh` looks for.

use crate::kernel::irq::kstat_irqs;
use crate::kernel::sched::clock::local_clock;
use crate::kernel::time::NSEC_PER_MSEC;
use crate::kernel::time::jiffies::{HZ, jiffies};

/// Ticks to wait for: a tenth of a second.
const TICKS: u64 = HZ / 10;

/// Waits for `TICKS` timer ticks and reports how long they took by the
/// scheduler clock.
/// Checks the tick now that it is one-shot: 100 ms of jiffies must come
/// from about 100 one-shot IRQ0s, on schedule. The clock event device rearms
/// each tick, so the count and the elapsed time should both match 100.
pub fn check_oneshot_tick() {
    use crate::kernel::sched::clock::local_clock;
    use crate::kernel::time::NSEC_PER_MSEC;
    use crate::kernel::time::jiffies::jiffies;

    let start_j = jiffies();
    let start_ns = local_clock();
    for _ in 0..(1u64 << 32) {
        if jiffies().wrapping_sub(start_j) >= 100 {
            break;
        }
        core::hint::spin_loop();
    }
    let ticks = jiffies().wrapping_sub(start_j);
    let ms = local_clock().wrapping_sub(start_ns) / NSEC_PER_MSEC;
    if (95..=105).contains(&ticks) && (90..=115).contains(&ms) {
        pr_info!("rinux: tick: one-shot ok ({} ticks in {} ms)", ticks, ms);
    } else {
        pr_err!("rinux: tick: one-shot FAILED ({} ticks in {} ms)", ticks, ms);
    }
}

pub fn check_timer() {
    let start_j = jiffies();
    let start_ns = local_clock();
    // Spin rather than halt, so a dead timer can't hang the check. The
    // bound is a few seconds on anything that can run this kernel.
    for _ in 0..(1u64 << 32) {
        if jiffies().wrapping_sub(start_j) >= TICKS {
            break;
        }
        core::hint::spin_loop();
    }
    let ticks = jiffies().wrapping_sub(start_j);
    let ms = local_clock().wrapping_sub(start_ns) / NSEC_PER_MSEC;
    if ticks == 0 {
        pr_err!("rinux: timer: no ticks; IRQ0 is not arriving");
        return;
    }
    pr_info!(
        "rinux: timer: {} ticks in {} ms (HZ={}, IRQ0 count {})",
        ticks,
        ms,
        HZ,
        kstat_irqs(0)
    );
}

/// Checks the memory allocators: the page allocator, kmalloc, the Rust
/// global allocator (Box/Vec), and vmalloc. Linux exercises these through
/// normal use; rinux checks them directly and reports on the console.
pub fn check_mm() {
    use crate::mm::__GFP_ZERO;
    use crate::mm::page_alloc::{alloc_pages, free_pages, nr_free_pages};
    use crate::mm::slub::{kfree, kmalloc, ksize};
    use crate::mm::vmalloc::{vfree, vmalloc};
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    let free_before = nr_free_pages();

    // Page allocator: order-0 and a higher order; freed pages come back.
    let p0 = alloc_pages(__GFP_ZERO, 0);
    let p3 = alloc_pages(0, 3);
    if p0 == 0 || p3 == 0 {
        pr_err!("rinux: mm: page allocation failed");
        return;
    }
    // The order-0 page is zeroed and writable.
    unsafe { (p0 as *mut u64).write_volatile(0xdead_beef) };
    assert!(unsafe { (p0 as *const u64).read_volatile() } == 0xdead_beef);
    free_pages(p0, 0);
    free_pages(p3, 3);

    // kmalloc across size classes, then kfree.
    let a = kmalloc(24, __GFP_ZERO);
    let b = kmalloc(4000, 0);
    assert!(a != 0 && b != 0 && ksize(a) >= 24 && ksize(b) >= 4000);
    unsafe { core::ptr::write_bytes(b as *mut u8, 0x5a, 4000) };
    kfree(a);
    kfree(b);

    // The Rust global allocator: Box and a growing Vec.
    let boxed = Box::new([7u64; 64]);
    assert_eq!(boxed[63], 7);
    let mut v: Vec<u32> = Vec::new();
    for i in 0..10_000 {
        v.push(i);
    }
    assert_eq!(
        v.iter().map(|&x| x as u64).sum::<u64>(),
        (0..10_000u64).sum()
    );
    drop(boxed);
    drop(v);

    // vmalloc: a multi-page region, written and freed.
    let vm = vmalloc(40 * 1024);
    if vm == 0 {
        pr_err!("rinux: mm: vmalloc failed");
        return;
    }
    unsafe { core::ptr::write_bytes(vm as *mut u8, 0xa5, 40 * 1024) };
    assert!(unsafe { (vm as *const u8).add(39 * 1024).read_volatile() } == 0xa5);
    vfree(vm);

    let free_after = nr_free_pages();
    pr_info!(
        "rinux: mm: ok ({} KiB free; page/kmalloc/Box/Vec/vmalloc tested; leak {} pages)",
        free_after * 4,
        free_before.saturating_sub(free_after)
    );
}

/// Demo kernel threads for [`check_sched`]: each bumps its counter a fixed
/// number of times, yielding with `schedule()` between iterations, then
/// exits.
mod sched_demo {
    use crate::kernel::sched::core::schedule;
    use core::sync::atomic::{AtomicU64, Ordering};

    pub static COUNTERS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
    pub const ITERS: u64 = 20;

    pub extern "C" fn worker(arg: u64) -> i32 {
        for _ in 0..ITERS {
            COUNTERS[arg as usize].fetch_add(1, Ordering::Relaxed);
            schedule();
        }
        0
    }
}

/// Preemption demo: two kernel threads that spin without ever yielding, so
/// they can only interleave if the timer preempts them. Each notes when the
/// running thread changed.
mod preempt_demo {
    use crate::kernel::time::jiffies::jiffies;
    use core::sync::atomic::{AtomicU64, Ordering};

    pub static DONE: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
    pub static LAST_ID: AtomicU64 = AtomicU64::new(u64::MAX);
    pub static TRANSITIONS: AtomicU64 = AtomicU64::new(0);
    pub const ITERS: u64 = 8;

    pub extern "C" fn spinner(id: u64) -> i32 {
        for _ in 0..ITERS {
            // Note if the running thread changed since the last iteration.
            if LAST_ID.swap(id, Ordering::Relaxed) != id {
                TRANSITIONS.fetch_add(1, Ordering::Relaxed);
            }
            // Busy-wait ~15 ms without yielding; only a timer preemption can
            // let the other thread run.
            let until = jiffies() + 15;
            while jiffies() < until {
                core::hint::spin_loop();
            }
        }
        DONE[id as usize].store(1, Ordering::Relaxed);
        0
    }

    pub fn done_all() -> bool {
        DONE.iter().all(|d| d.load(Ordering::Relaxed) != 0)
    }
}

/// Checks timer-driven preemption: two non-yielding kernel threads must
/// still interleave because the tick preempts them at the end of each slice.
/// Set once the sleep_timeout thread has reported.
static SLEEP_TIMEOUT_DONE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// The sleep_timeout check body, run in a kernel thread: a sleep blocks the
/// running task, which must not be the boot task (the idle task).
extern "C" fn sleep_timeout_check(_arg: u64) -> i32 {
    use crate::kernel::time::jiffies::jiffies;
    use crate::kernel::sched::clock::local_clock;
    use crate::kernel::time::hrtimer::sleep_until_ns;
    use crate::kernel::time::sleep_timeout::{msleep, schedule_timeout};
    use core::sync::atomic::Ordering;

    let j0 = jiffies();
    let negative_ret = schedule_timeout(-1);
    let negative_jiffies = jiffies().wrapping_sub(j0);

    let j1 = jiffies();
    let ret = schedule_timeout(20);
    let slept = jiffies().wrapping_sub(j1);

    let j2 = jiffies();
    msleep(30);
    let msleep_slept = jiffies().wrapping_sub(j2);

    // A high-resolution sleep of 2.5 ms wakes at its deadline, not on a whole
    // jiffy, and not early.
    let t0 = local_clock();
    sleep_until_ns(t0 + 2_500_000);
    let hr_ns = local_clock().wrapping_sub(t0);
    let hr_ok = (2_500_000..25_000_000).contains(&hr_ns);

    let ok = negative_ret == 0
        && negative_jiffies <= 1
        && ret == 0
        && slept >= 20
        && msleep_slept >= 30
        && hr_ok;
    if ok {
        pr_info!(
            "rinux: sleep_timeout: schedule_timeout(20) slept {} jiffies, msleep(30) slept {} jiffies, hrtimer 2.5 ms slept {} us",
            slept,
            msleep_slept,
            hr_ns / 1000
        );
    } else {
        pr_err!(
            "rinux: sleep_timeout: FAILED (negative ret {}, slept {}, ret {}, msleep slept {}, hrtimer {} ns)",
            negative_ret,
            slept,
            ret,
            msleep_slept,
            hr_ns
        );
    }
    SLEEP_TIMEOUT_DONE.store(true, Ordering::Release);
    0
}

/// Checks `schedule_timeout()` and `msleep()`: a negative timeout returns at
/// once, a timeout sleeps for at least its jiffies and returns 0, and `msleep`
/// sleeps for at least its milliseconds. The check runs in a kernel thread,
/// and the boot task yields until it reports.
pub fn check_sleep_timeout() {
    use crate::kernel::kthread::kthread_run;
    use crate::kernel::sched::core::schedule;
    use crate::kernel::time::jiffies::{HZ, jiffies};
    use core::sync::atomic::Ordering;

    if kthread_run(sleep_timeout_check, 0, b"sleep_timeout").is_null() {
        pr_err!("rinux: sleep_timeout: failed to start the check thread");
        return;
    }
    let deadline = jiffies() + 5 * HZ;
    while !SLEEP_TIMEOUT_DONE.load(Ordering::Acquire) && jiffies() < deadline {
        schedule();
    }
    if !SLEEP_TIMEOUT_DONE.load(Ordering::Acquire) {
        pr_err!("rinux: sleep_timeout: FAILED (check thread did not finish)");
    }
}

pub fn check_preempt() {
    use crate::kernel::kthread::kthread_run;
    use crate::kernel::sched::core::schedule;
    use crate::kernel::time::jiffies::{HZ, jiffies};
    use core::sync::atomic::Ordering;

    // Short slices make interleaving visible within the test window.
    crate::kernel::sched::core::set_rr_timeslice(20);
    for id in 0..2u64 {
        let name = [b"spin/", &[b'0' + id as u8][..]].concat();
        if kthread_run(preempt_demo::spinner, id, &name).is_null() {
            pr_err!("rinux: preempt: failed to start spinner {id}");
            return;
        }
    }
    let deadline = jiffies() + 5 * HZ;
    let done = || preempt_demo::done_all();
    while !done() && jiffies() < deadline {
        schedule();
    }
    crate::kernel::sched::core::set_rr_timeslice(crate::kernel::sched::core::RR_TIMESLICE);

    let transitions = preempt_demo::TRANSITIONS.load(Ordering::Relaxed);
    // More than one transition means the two threads interleaved, which is
    // only possible by preemption since neither yields.
    if preempt_demo::done_all() && transitions > 2 {
        pr_info!(
            "rinux: preempt: ok (2 non-yielding kthreads interleaved, {} switches)",
            transitions
        );
    } else {
        pr_err!(
            "rinux: preempt: not interleaved ({} transitions)",
            transitions
        );
    }
}

/// Checks the scheduler: spawn three kernel threads, let them run
/// round-robin by yielding from the idle task, and confirm each made
/// progress and that context switches happened.
pub fn check_sched() {
    use crate::kernel::kthread::kthread_run;
    use crate::kernel::sched::core::{nr_context_switches, schedule};
    use crate::kernel::time::jiffies::{HZ, jiffies};
    use core::sync::atomic::Ordering;

    let switches_before = nr_context_switches();
    for i in 0..3u64 {
        let name = [b"worker/", &[b'0' + i as u8][..]].concat();
        if kthread_run(sched_demo::worker, i, &name).is_null() {
            pr_err!("rinux: sched: failed to start worker {i}");
            return;
        }
    }

    // Yield from the idle task until the workers finish, or time out.
    let deadline = jiffies() + 5 * HZ;
    let done = || {
        sched_demo::COUNTERS
            .iter()
            .all(|c| c.load(Ordering::Relaxed) >= sched_demo::ITERS)
    };
    while !done() && jiffies() < deadline {
        schedule();
    }

    let switches = nr_context_switches() - switches_before;
    let counts: [u64; 3] =
        core::array::from_fn(|i| sched_demo::COUNTERS[i].load(Ordering::Relaxed));
    if done() {
        pr_info!(
            "rinux: sched: ok (3 kthreads x{} iters, {} context switches)",
            sched_demo::ITERS,
            switches
        );
    } else {
        pr_err!("rinux: sched: workers did not all finish: {:?}", counts);
    }
}

/// Checks the filesystem: list the root, read a procfs file, and if the
/// initramfs was unpacked, read a file back from it. Linux exercises the
/// VFS through syscalls; rinux calls the `vfs_*` helpers directly.
pub fn check_fs() {
    use crate::fs::vfs::{read_file, vfs_readdir, vfs_stat};
    use crate::include::linux::fs::s_isdir;
    use crate::kernel::printk::{BStr, FmtBuf};
    use core::fmt::Write;

    // List "/".
    match vfs_readdir(b"/") {
        Ok(entries) => {
            let mut line = FmtBuf::<256>::new();
            for e in &entries {
                let slash = if s_isdir(
                    vfs_stat(&[b"/", &e.name[..]].concat())
                        .map(|(m, _)| m)
                        .unwrap_or(0),
                ) {
                    "/"
                } else {
                    ""
                };
                let _ = write!(line, "{}{} ", BStr(&e.name), slash);
            }
            pr_info!("rinux: fs: / contains: {}", line);
        }
        Err(_) => {
            pr_err!("rinux: fs: cannot read root directory");
            return;
        }
    }

    // Read /proc/version through the procfs file_operations.
    match read_file(b"/proc/version") {
        Ok(data) => pr_info!(
            "rinux: fs: /proc/version = {}",
            BStr(data.as_slice().split(|&c| c == b'\n').next().unwrap_or(&[]))
        ),
        Err(_) => pr_err!("rinux: fs: cannot read /proc/version"),
    }

    // Read the "model name" line out of /proc/cpuinfo.
    if let Ok(data) = read_file(b"/proc/cpuinfo") {
        for line in data.split(|&c| c == b'\n') {
            if line.starts_with(b"model name") {
                pr_info!("rinux: fs: /proc/cpuinfo {}", BStr(line));
                break;
            }
        }
    }

    // Read a sysfs attribute (/sys/kernel/address_bits).
    if let Ok(data) = read_file(b"/sys/kernel/address_bits") {
        let first = data.split(|&c| c == b'\n').next().unwrap_or(&[]);
        pr_info!("rinux: fs: /sys/kernel/address_bits = {}", BStr(first));
    }

    // If the initramfs supplied /etc/motd, read it back.
    if let Ok(data) = read_file(b"/etc/motd") {
        let first = data.split(|&c| c == b'\n').next().unwrap_or(&[]);
        pr_info!("rinux: fs: /etc/motd = {}", BStr(first));
    }
    pr_info!("rinux: fs: ok ({} inodes)", crate::fs::vfs::nr_inodes());
}

/// The init process body for [`check_user`]: exec `/init` from the rootfs. On
/// success `kernel_execve` enters ring 3 and never returns here; on failure it
/// logs the error and the thread exits.
extern "C" fn init_process(_arg: u64) -> i32 {
    if let Err(e) = crate::fs::exec::kernel_execve(b"/init") {
        pr_err!("rinux: user: could not exec /init (errno {})", e.to_errno());
        return 1;
    }
    // Unreachable: a successful execve does not return to the kernel thread.
    0
}

/// Checks user space: load `/init` from the initramfs, run it in ring 3, and
/// report the exit code it passes to `exit()`. Linux's `rest_init()` starts
/// the init process this way; rinux does it as a boot-time check and waits for
/// it from the idle task.
pub fn check_user() {
    use crate::kernel::kthread::kthread_run;
    use crate::kernel::sched::core::schedule;
    use crate::kernel::syscalls::USER_EXIT_CODE;
    use crate::kernel::time::jiffies::{HZ, jiffies};
    use core::sync::atomic::Ordering;

    // Only run if the initramfs actually supplied an /init.
    if crate::fs::vfs::vfs_stat(b"/init").is_err() {
        pr_info!("rinux: user: no /init in rootfs; skipping user-space check");
        return;
    }

    USER_EXIT_CODE.store(i64::MIN, Ordering::Release);
    if kthread_run(init_process, 0, b"init").is_null() {
        pr_err!("rinux: user: failed to start init");
        return;
    }

    // Yield from the idle task until init exits, or time out.
    let deadline = jiffies() + 5 * HZ;
    while USER_EXIT_CODE.load(Ordering::Acquire) == i64::MIN && jiffies() < deadline {
        schedule();
    }

    let code = USER_EXIT_CODE.load(Ordering::Acquire);
    if code == i64::MIN {
        pr_err!("rinux: user: init did not exit within the time limit");
    } else {
        pr_info!(
            "rinux: user: ok (init ran in ring 3 and exited with code {})",
            code
        );
    }
}

/// Checks the PCI bus: enumerate the hierarchy from bus 0 and list each
/// function's address, vendor:device, class and IRQ. Linux discovers PCI
/// devices this way during boot and binds drivers; rinux only enumerates,
/// so it reports the scan directly on the console.
pub fn check_pci() {
    use crate::drivers::pci::{pci_func, pci_slot, scan_root};

    let devs = scan_root();
    if devs.is_empty() {
        pr_err!("rinux: pci: no devices found (is this a PCI machine?)");
        return;
    }
    for d in &devs {
        // The low 32 bits of BAR0 locate a header-0 device's registers.
        let (bar0, io) = d.bar_address(0);
        let mut bar = crate::kernel::printk::FmtBuf::<32>::new();
        if bar0 != 0 {
            use core::fmt::Write;
            let _ = write!(bar, " {}={:#x}", if io { "io" } else { "mem" }, bar0);
        }
        pr_info!(
            "rinux: pci: {:02x}:{:02x}.{} {:04x}:{:04x} class {:06x} ({}) irq {}{}",
            d.bus,
            pci_slot(d.devfn),
            pci_func(d.devfn),
            d.vendor,
            d.device,
            d.class,
            d.class_name(),
            d.irq,
            bar.as_str(),
        );
    }
    pr_info!("rinux: pci: ok ({} functions enumerated)", devs.len());
}

/// Checks a virtio driver end to end: bind the virtio-rng device on the PCI
/// bus, run a real virtqueue transaction (hand it a buffer, kick it, wait
/// for completion), and report the random bytes it filled in. Exercises the
/// whole virtio stack: the legacy PCI transport, the split ring, and DMA to
/// an identity-mapped buffer. Skipped if QEMU was started without the device.
pub fn check_virtio() {
    use crate::drivers::char::hw_random::virtio_rng::VirtioRng;
    use crate::kernel::printk::FmtBuf;
    use core::fmt::Write;

    let rng = match VirtioRng::probe() {
        Some(r) => r,
        None => {
            pr_info!("rinux: virtio: no virtio-rng device; skipping (add -device virtio-rng-pci)");
            return;
        }
    };

    let mut buf = [0u8; 16];
    let n = rng.read(&mut buf);
    if n == 0 {
        pr_err!("rinux: virtio: virtio-rng returned no data");
        return;
    }
    let mut hex = FmtBuf::<48>::new();
    for b in &buf[..n] {
        let _ = write!(hex, "{:02x}", b);
    }
    pr_info!(
        "rinux: virtio: ok (virtio-rng gave {} bytes: {})",
        n,
        hex.as_str()
    );
}

/// Checks the virtio-console driver: bind a virtio-console device and send a
/// marker line to its port. The host routes the port's output to its backing
/// character device, so the boot test greps that file for the marker.
/// Skipped if QEMU was started without a virtio-console.
pub fn check_virtio_console() {
    use crate::drivers::char::virtio_console::VirtioConsole;

    let con = match VirtioConsole::probe() {
        Some(c) => c,
        None => {
            pr_info!(
                "rinux: virtio-console: no device; skipping (add -device virtio-serial-pci -device virtconsole,chardev=...)"
            );
            return;
        }
    };

    let msg = b"rinux-virtio-console: hello from the port\n";
    let n = con.write(msg);
    if n == msg.len() {
        pr_info!(
            "rinux: virtio-console: ok (sent {} bytes, device has {} port(s))",
            n,
            con.max_nr_ports
        );
    } else {
        pr_err!(
            "rinux: virtio-console: short write ({} of {})",
            n,
            msg.len()
        );
    }

    // Read whatever the host sent to the port (its chardev `input-path`).
    use crate::kernel::printk::BStr;
    let mut inbuf = [0u8; 128];
    let got = con.read(&mut inbuf);
    if got > 0 {
        pr_info!(
            "rinux: virtio-console: received {} bytes: {}",
            got,
            BStr(&inbuf[..got])
        );
    } else {
        pr_info!("rinux: virtio-console: no host input (set chardev input-path to test)");
    }
}

/// Checks the block layer: bind a virtio-blk disk and read sector 0 through
/// the [`BlockDevice`](crate::block::BlockDevice) trait, reporting its
/// capacity and the first bytes. Linux reads the boot disk this way before
/// mounting a root filesystem; rinux reads one sector as a check. Skipped if
/// QEMU was started without a virtio-blk drive.
pub fn check_block() {
    use crate::block::probe_boot_blockdev;
    use crate::kernel::printk::{BStr, FmtBuf};
    use core::fmt::Write;

    let disk = match probe_boot_blockdev() {
        Some(d) => d,
        None => {
            pr_info!("rinux: block: no block device; skipping (add -device virtio-blk-pci)");
            return;
        }
    };

    let mut sector = [0u8; 512];
    if disk.read(0, &mut sector).is_err() {
        pr_err!("rinux: block: reading sector 0 failed");
        return;
    }

    // Show the first 16 bytes as hex, and any leading printable ASCII (the
    // test disk starts with a signature string).
    let mut hex = FmtBuf::<48>::new();
    for b in &sector[..16] {
        let _ = write!(hex, "{:02x}", b);
    }
    let sig_len = sector
        .iter()
        .take_while(|&&c| (0x20..0x7f).contains(&c))
        .count();
    pr_info!(
        "rinux: block: ok (virtio-blk {} sectors, {} MiB; sector 0: {} \"{}\")",
        disk.nr_sectors(),
        disk.nr_sectors() / 2048,
        hex.as_str(),
        BStr(&sector[..sig_len])
    );
}

/// Checks the ext2 filesystem: find a virtio-blk disk that holds an ext2
/// image, mount it, list the root directory, and read a file back. Linux
/// mounts a real root filesystem this way; rinux reads one as a check.
/// Skipped if no attached disk carries ext2.
pub fn check_ext2() {
    use crate::block::probe_all_blockdevs;
    use crate::fs::ext2::Ext2Fs;
    use crate::kernel::printk::{BStr, FmtBuf};
    use core::fmt::Write;

    // Try to mount ext2 on each disk; the first that validates wins.
    let fs = probe_all_blockdevs()
        .into_iter()
        .find_map(|d| Ext2Fs::mount(d).ok());
    let fs = match fs {
        Some(fs) => fs,
        None => {
            pr_info!("rinux: ext2: no ext2 disk; skipping (attach an ext2 virtio-blk drive)");
            return;
        }
    };

    // List the root directory.
    let root = match fs.read_inode(crate::fs::ext2::layout::EXT2_ROOT_INO) {
        Ok(i) => i,
        Err(_) => {
            pr_err!("rinux: ext2: cannot read the root inode");
            return;
        }
    };
    let mut listing = FmtBuf::<160>::new();
    let _ = fs.read_dir(&root, |name, _ino, ft| {
        let slash = if ft == crate::fs::ext2::layout::EXT2_FT_DIR {
            "/"
        } else {
            ""
        };
        let _ = write!(listing, "{}{} ", BStr(name), slash);
    });
    pr_info!("rinux: ext2: / contains: {}", listing.as_str());

    // Read a known file back.
    match fs.read_path(b"/hello.txt") {
        Ok(data) => {
            let first = data.split(|&c| c == b'\n').next().unwrap_or(&[]);
            pr_info!(
                "rinux: ext2: ok (/hello.txt is {} bytes: \"{}\")",
                data.len(),
                BStr(first)
            );
        }
        Err(_) => pr_err!("rinux: ext2: could not read /hello.txt"),
    }
}

/// Checks GPT partition parsing: find a virtio-blk disk with a GUID partition
/// table, validate its header CRC, and list the partitions (first/last LBA
/// and name). Skipped if no GPT disk is attached.
pub fn check_gpt() {
    use crate::block::partitions::efi::parse_gpt;
    use crate::block::probe_all_blockdevs;
    use crate::kernel::printk::{BStr, FmtBuf};
    use core::fmt::Write;

    for dev in probe_all_blockdevs() {
        let gpt = match parse_gpt(&*dev) {
            Ok(g) if !g.partitions.is_empty() => g,
            _ => continue,
        };
        let mut line = FmtBuf::<200>::new();
        for p in &gpt.partitions {
            let _ = write!(line, " [{}: {}-{}]", BStr(&p.name), p.first_lba, p.last_lba);
        }
        pr_info!(
            "rinux: gpt: ok ({} partitions, header CRC {}:{})",
            gpt.partitions.len(),
            if gpt.header_crc_ok { "ok" } else { "BAD" },
            line.as_str()
        );
        return;
    }
    pr_info!("rinux: gpt: no GPT disk; skipping");
}

/// Checks MBR partition parsing: find a virtio-blk disk with a DOS partition
/// table and list its primary partitions. Linux scans partitions when a disk
/// is added; rinux does it as a check. Skipped if no partitioned disk is
/// attached.
pub fn check_partitions() {
    use crate::block::partitions::msdos::parse_mbr;
    use crate::block::probe_all_blockdevs;
    use crate::kernel::printk::FmtBuf;
    use core::fmt::Write;

    for dev in probe_all_blockdevs() {
        // A real primary partition starts past the MBR, so ignore any entry
        // at LBA 0 (a FAT "superfloppy" boot sector can look like a table).
        let parts: alloc::vec::Vec<_> = match parse_mbr(&*dev) {
            Ok(p) => p
                .into_iter()
                .filter(|p| p.start_lba > 0 && p.nr_sectors > 0)
                .collect(),
            _ => continue,
        };
        if parts.is_empty() {
            continue;
        }
        let mut line = FmtBuf::<160>::new();
        for (i, p) in parts.iter().enumerate() {
            let _ = write!(
                line,
                " p{}: type={:#04x} lba={} sectors={}{}",
                i + 1,
                p.sys_ind,
                p.start_lba,
                p.nr_sectors,
                if p.bootable { " (boot)" } else { "" }
            );
        }
        pr_info!(
            "rinux: partitions: ok ({} MBR partitions:{})",
            parts.len(),
            line.as_str()
        );
        return;
    }
    pr_info!("rinux: partitions: no partitioned disk; skipping");
}

/// Checks the ext4 filesystem: find the virtio-blk disk holding an ext4
/// image, mount it, and read a file whose data is mapped through the extent
/// tree. The test file `/ext4.txt` exists only on the ext4 disk, so this
/// also tells the ext4 image apart from the ext2 one. Skipped if no attached
/// disk carries an ext4 image with that file.
pub fn check_ext4() {
    use crate::block::probe_all_blockdevs;
    use crate::fs::ext4::Ext4Fs;
    use crate::kernel::printk::BStr;

    // Try each disk; the ext4 image is the one that mounts and has /ext4.txt.
    for dev in probe_all_blockdevs() {
        let fs = match Ext4Fs::mount(dev) {
            Ok(fs) => fs,
            Err(_) => continue,
        };
        let data = match fs.read_path(b"/ext4.txt") {
            Ok(d) => d,
            Err(_) => continue,
        };
        // Count entries in the root to show the directory read works too.
        let mut entries = 0u32;
        if let Ok(root) = fs.read_inode(crate::fs::ext2::layout::EXT2_ROOT_INO) {
            let _ = fs.read_dir(&root, |_, _, _| entries += 1);
        }
        let first = data.split(|&c| c == b'\n').next().unwrap_or(&[]);
        pr_info!(
            "rinux: ext4: ok (extent-mapped /ext4.txt is {} bytes: \"{}\"; / has {} entries)",
            data.len(),
            BStr(first),
            entries
        );
        return;
    }
    pr_info!("rinux: ext4: no ext4 disk with /ext4.txt; skipping");
}

/// Checks the FAT32 filesystem: find the virtio-blk disk holding a FAT32
/// image, mount it, and read `/fat.txt` through the FAT cluster chain. The
/// file exists only on the FAT disk. Skipped if no attached disk is FAT32.
pub fn check_fat() {
    use crate::block::probe_all_blockdevs;
    use crate::fs::fat::Fat32Fs;
    use crate::kernel::printk::BStr;

    for dev in probe_all_blockdevs() {
        let fs = match Fat32Fs::mount(dev) {
            Ok(fs) => fs,
            Err(_) => continue,
        };
        let data = match fs.read_root_file(b"fat.txt") {
            Ok(d) => d,
            Err(_) => continue,
        };
        let entries = fs.read_root().map(|v| v.len()).unwrap_or(0);
        let first = data.split(|&c| c == b'\n').next().unwrap_or(&[]);
        pr_info!(
            "rinux: fat: ok (FAT32 /fat.txt is {} bytes: \"{}\"; / has {} entries)",
            data.len(),
            BStr(first),
            entries
        );
        return;
    }
    pr_info!("rinux: fat: no FAT32 disk with /fat.txt; skipping");
}

/// Checks networking end to end: bind a virtio-net device, then transmit a
/// broadcast ARP request for the gateway and wait for its reply, proving the
/// NIC's transmit and receive queues and the Ethernet/ARP code all work.
/// Under QEMU user networking the gateway is 10.0.2.2 and the guest is
/// 10.0.2.15. Skipped if QEMU was started without a virtio-net device.
pub fn check_net() {
    use crate::drivers::net::virtio_net::VirtioNet;
    use crate::include::uapi::if_ether::ETH_FRAME_LEN;
    use crate::kernel::printk::FmtBuf;
    use crate::kernel::time::jiffies::HZ;
    use crate::net::arp;
    use core::fmt::Write;

    let mut net = match VirtioNet::probe() {
        Some(n) => n,
        None => {
            pr_info!("rinux: net: no virtio-net device; skipping (add -device virtio-net-pci)");
            return;
        }
    };

    let mut macbuf = FmtBuf::<24>::new();
    for (i, b) in net.mac.iter().enumerate() {
        let _ = write!(macbuf, "{}{:02x}", if i == 0 { "" } else { ":" }, b);
    }
    pr_info!("rinux: net: virtio-net up, MAC {}", macbuf.as_str());

    // Ask who owns the gateway address.
    let our_ip = [10u8, 0, 2, 15];
    let gw_ip = [10u8, 0, 2, 2];
    let mut frame = [0u8; arp::ARP_FRAME_LEN];
    let len = arp::build_request(&mut frame, &net.mac, &our_ip, &gw_ip);
    if !net.send(&frame[..len]) {
        pr_err!("rinux: net: ARP request transmit timed out");
        return;
    }

    // Collect frames for up to a second, looking for the ARP reply.
    let mut buf = [0u8; ETH_FRAME_LEN];
    for _ in 0..16 {
        let n = match net.recv(&mut buf, HZ) {
            Some(n) => n,
            None => break,
        };
        if let Some(reply) = arp::parse_reply(&buf[..n]) {
            let mut macbuf = FmtBuf::<24>::new();
            for (i, b) in reply.mac.iter().enumerate() {
                let _ = write!(macbuf, "{}{:02x}", if i == 0 { "" } else { ":" }, b);
            }
            pr_info!(
                "rinux: net: ok (ARP: {}.{}.{}.{} is at {})",
                reply.ip[0],
                reply.ip[1],
                reply.ip[2],
                reply.ip[3],
                macbuf.as_str()
            );
            return;
        }
    }
    pr_err!("rinux: net: no ARP reply received");
}

/// Resolves `target_ip`'s MAC with a broadcast ARP request, as the neighbour
/// layer would before sending an IP packet. Returns `None` on timeout.
fn arp_resolve(
    net: &mut crate::drivers::net::virtio_net::VirtioNet,
    our_ip: &[u8; 4],
    target_ip: &[u8; 4],
) -> Option<[u8; 6]> {
    use crate::include::uapi::if_ether::ETH_FRAME_LEN;
    use crate::kernel::time::jiffies::HZ;
    use crate::net::arp;

    let mut frame = [0u8; arp::ARP_FRAME_LEN];
    let len = arp::build_request(&mut frame, &net.mac, our_ip, target_ip);
    if !net.send(&frame[..len]) {
        return None;
    }
    let mut buf = [0u8; ETH_FRAME_LEN];
    for _ in 0..16 {
        let n = net.recv(&mut buf, HZ)?;
        if let Some(reply) = arp::parse_reply(&buf[..n])
            && reply.ip == *target_ip
        {
            return Some(reply.mac);
        }
    }
    None
}

/// Checks the IPv4 + ICMP path: resolve the gateway's MAC, then ping it with
/// an ICMP echo request and wait for the echo reply. This exercises ARP, the
/// Internet checksum, the IPv4 header and ICMP together. Under QEMU user
/// networking the gateway 10.0.2.2 answers pings. Skipped with no NIC.
pub fn check_ping() {
    use crate::drivers::net::virtio_net::VirtioNet;
    use crate::include::uapi::if_ether::{ETH_FRAME_LEN, ETH_HLEN, ETH_P_IP};
    use crate::include::uapi::in_::IPPROTO_ICMP;
    use crate::include::uapi::ip::IP_HLEN;
    use crate::kernel::time::jiffies::HZ;
    use crate::net::ethernet;
    use crate::net::ipv4::{self, icmp};

    let mut net = match VirtioNet::probe() {
        Some(n) => n,
        None => {
            pr_info!("rinux: ping: no virtio-net device; skipping");
            return;
        }
    };

    let our_ip = [10u8, 0, 2, 15];
    let gw_ip = [10u8, 0, 2, 2];
    let gw_mac = match arp_resolve(&mut net, &our_ip, &gw_ip) {
        Some(m) => m,
        None => {
            pr_err!("rinux: ping: could not resolve the gateway MAC");
            return;
        }
    };

    // Assemble an Ethernet + IPv4 + ICMP echo-request frame.
    let id = 0x1234u16;
    let seq = 1u16;
    let payload = b"rinux-icmp-echo!";
    let mut frame = [0u8; ETH_HLEN + IP_HLEN + 64];
    ethernet::eth_header(&mut frame, &gw_mac, &net.mac, ETH_P_IP);
    let icmp_off = ETH_HLEN + IP_HLEN;
    let icmp_len = icmp::build_echo_request(&mut frame[icmp_off..], id, seq, payload);
    ipv4::build_header(
        &mut frame[ETH_HLEN..],
        &our_ip,
        &gw_ip,
        IPPROTO_ICMP,
        icmp_len,
    );
    let total = ETH_HLEN + IP_HLEN + icmp_len;

    if !net.send(&frame[..total]) {
        pr_err!("rinux: ping: echo request transmit timed out");
        return;
    }

    let mut buf = [0u8; ETH_FRAME_LEN];
    for _ in 0..16 {
        let n = match net.recv(&mut buf, HZ) {
            Some(n) => n,
            None => break,
        };
        if ethernet::eth_type(&buf[..n]) != Some(ETH_P_IP) {
            continue;
        }
        let pkt = ethernet::eth_payload(&buf[..n]);
        let iph = match ipv4::parse_header(pkt) {
            Some(h) => h,
            None => continue,
        };
        if iph.protocol != IPPROTO_ICMP || iph.src != gw_ip {
            continue;
        }
        let end = iph.total_len.min(pkt.len());
        if iph.payload_off >= end {
            continue;
        }
        if let Some(rep) = icmp::parse_echo_reply(&pkt[iph.payload_off..end])
            && rep.id == id
        {
            pr_info!(
                "rinux: ping: ok (echo reply from {}.{}.{}.{}, id {:#x} seq {})",
                gw_ip[0],
                gw_ip[1],
                gw_ip[2],
                gw_ip[3],
                rep.id,
                rep.seq
            );
            return;
        }
    }
    pr_err!("rinux: ping: no echo reply from the gateway");
}

/// Sends one DHCP message (Ethernet/IP/UDP broadcast) and returns the first
/// matching reply, as `ic_bootp_send_if`/`ic_bootp_recv` do for one round.
fn dhcp_send_recv(
    net: &mut crate::drivers::net::virtio_net::VirtioNet,
    xid: u32,
    msg_type: u8,
    server_ip: Option<[u8; 4]>,
    req_ip: Option<[u8; 4]>,
) -> Option<crate::net::ipv4::ipconfig::DhcpReply> {
    use crate::include::uapi::if_ether::{ETH_BROADCAST, ETH_FRAME_LEN, ETH_HLEN, ETH_P_IP};
    use crate::include::uapi::in_::IPPROTO_UDP;
    use crate::include::uapi::ip::IP_HLEN;
    use crate::include::uapi::udp::UDP_HLEN;
    use crate::kernel::time::jiffies::HZ;
    use crate::net::ethernet;
    use crate::net::ipv4::{self, ipconfig, udp};

    let src_ip = [0u8, 0, 0, 0];
    let dst_ip = [255u8, 255, 255, 255];
    let udp_off = ETH_HLEN + IP_HLEN;
    let bootp_off = udp_off + UDP_HLEN;

    let mut frame = [0u8; 600];
    let bootp_len = ipconfig::build_message(
        &mut frame[bootp_off..],
        &net.mac,
        xid,
        msg_type,
        server_ip,
        req_ip,
    );
    let udp_total = UDP_HLEN + bootp_len;
    udp::build(
        &mut frame[udp_off..udp_off + udp_total],
        ipconfig::BOOTP_CLIENT_PORT,
        ipconfig::BOOTP_SERVER_PORT,
        &src_ip,
        &dst_ip,
    );
    ipv4::build_header(
        &mut frame[ETH_HLEN..],
        &src_ip,
        &dst_ip,
        IPPROTO_UDP,
        udp_total,
    );
    ethernet::eth_header(&mut frame, &ETH_BROADCAST, &net.mac, ETH_P_IP);

    if !net.send(&frame[..bootp_off + bootp_len]) {
        return None;
    }

    let mut buf = [0u8; ETH_FRAME_LEN];
    for _ in 0..32 {
        let n = net.recv(&mut buf, HZ)?;
        if ethernet::eth_type(&buf[..n]) != Some(ETH_P_IP) {
            continue;
        }
        let pkt = ethernet::eth_payload(&buf[..n]);
        let iph = match ipv4::parse_header(pkt) {
            Some(h) => h,
            None => continue,
        };
        if iph.protocol != IPPROTO_UDP {
            continue;
        }
        let end = iph.total_len.min(pkt.len());
        if iph.payload_off + UDP_HLEN > end {
            continue;
        }
        let seg = &pkt[iph.payload_off..end];
        if udp::ports(seg).map(|(_, d)| d) != Some(ipconfig::BOOTP_CLIENT_PORT) {
            continue;
        }
        if let Some(r) = ipconfig::parse_reply(&seg[UDP_HLEN..], xid) {
            return Some(r);
        }
    }
    None
}

/// Checks UDP and the kernel DHCP client: run a full DHCP exchange
/// (DISCOVER -> OFFER -> REQUEST -> ACK) and report the leased address,
/// router and DNS server. Under QEMU user networking the built-in DHCP
/// server hands out 10.0.2.15. Skipped with no NIC.
pub fn check_dhcp() {
    use crate::drivers::net::virtio_net::VirtioNet;
    use crate::net::ipv4::ipconfig::{DHCPACK, DHCPDISCOVER, DHCPOFFER, DHCPREQUEST};

    let mut net = match VirtioNet::probe() {
        Some(n) => n,
        None => {
            pr_info!("rinux: dhcp: no virtio-net device; skipping");
            return;
        }
    };

    let xid = 0x1b2f_3c4d;
    let offer = match dhcp_send_recv(&mut net, xid, DHCPDISCOVER, None, None) {
        Some(r) if r.msg_type == DHCPOFFER => r,
        _ => {
            pr_err!("rinux: dhcp: no DHCPOFFER received");
            return;
        }
    };
    let ack = match dhcp_send_recv(
        &mut net,
        xid,
        DHCPREQUEST,
        Some(offer.server_id),
        Some(offer.your_ip),
    ) {
        Some(r) if r.msg_type == DHCPACK => r,
        _ => {
            pr_err!("rinux: dhcp: no DHCPACK after request");
            return;
        }
    };

    let prefix: u32 = ack.subnet.iter().map(|b| b.count_ones()).sum();
    pr_info!(
        "rinux: dhcp: ok (leased {}.{}.{}.{}/{} router {}.{}.{}.{} dns {}.{}.{}.{})",
        ack.your_ip[0],
        ack.your_ip[1],
        ack.your_ip[2],
        ack.your_ip[3],
        prefix,
        ack.router[0],
        ack.router[1],
        ack.router[2],
        ack.router[3],
        ack.dns[0],
        ack.dns[1],
        ack.dns[2],
        ack.dns[3]
    );
}

/// The TCP port the `check_tcp` boot check connects to (the host test server
/// that `boot-test.sh` starts; reachable from the guest at 10.0.2.2).
const TCP_TEST_PORT: u16 = 10080;

/// A one-connection TCP context for [`check_tcp`]: addresses and ports plus
/// helpers to send a segment and receive the next one for our port.
struct TcpCtx {
    our_ip: [u8; 4],
    dst_ip: [u8; 4],
    gw_mac: [u8; 6],
    src_port: u16,
    dst_port: u16,
}

impl TcpCtx {
    /// Builds and transmits one TCP segment with the given control fields.
    fn send(
        &self,
        net: &mut crate::drivers::net::virtio_net::VirtioNet,
        seq: u32,
        ack: u32,
        flags: u8,
        payload: &[u8],
    ) -> bool {
        use crate::include::uapi::if_ether::{ETH_HLEN, ETH_P_IP};
        use crate::include::uapi::in_::IPPROTO_TCP;
        use crate::include::uapi::ip::IP_HLEN;
        use crate::include::uapi::tcp::TCP_HLEN;
        use crate::net::ethernet;
        use crate::net::ipv4::{self, tcp};

        let seg_len = TCP_HLEN + payload.len();
        let tcp_off = ETH_HLEN + IP_HLEN;
        let mut frame = [0u8; ETH_HLEN + IP_HLEN + TCP_HLEN + 64];
        frame[tcp_off + TCP_HLEN..tcp_off + TCP_HLEN + payload.len()].copy_from_slice(payload);
        tcp::build(
            &mut frame[tcp_off..tcp_off + seg_len],
            self.src_port,
            self.dst_port,
            seq,
            ack,
            flags,
            64240,
            &self.our_ip,
            &self.dst_ip,
        );
        ipv4::build_header(
            &mut frame[ETH_HLEN..],
            &self.our_ip,
            &self.dst_ip,
            IPPROTO_TCP,
            seg_len,
        );
        ethernet::eth_header(&mut frame, &self.gw_mac, &net.mac, ETH_P_IP);
        net.send(&frame[..tcp_off + seg_len])
    }

    /// Receives the next TCP segment addressed to our port, copying its
    /// payload into `out`. Returns `(seq, ack, flags, payload_len)`.
    fn recv(
        &self,
        net: &mut crate::drivers::net::virtio_net::VirtioNet,
        out: &mut [u8],
    ) -> Option<(u32, u32, u8, usize)> {
        use crate::include::uapi::if_ether::{ETH_FRAME_LEN, ETH_P_IP};
        use crate::include::uapi::in_::IPPROTO_TCP;
        use crate::kernel::time::jiffies::HZ;
        use crate::net::ethernet;
        use crate::net::ipv4::{self, tcp};

        let mut buf = [0u8; ETH_FRAME_LEN];
        for _ in 0..32 {
            let n = net.recv(&mut buf, HZ)?;
            if ethernet::eth_type(&buf[..n]) != Some(ETH_P_IP) {
                continue;
            }
            let pkt = ethernet::eth_payload(&buf[..n]);
            let iph = match ipv4::parse_header(pkt) {
                Some(h) => h,
                None => continue,
            };
            if iph.protocol != IPPROTO_TCP || iph.dst != self.our_ip {
                continue;
            }
            let end = iph.total_len.min(pkt.len());
            let seg = &pkt[iph.payload_off..end];
            let t = match tcp::parse(seg) {
                Some(t) => t,
                None => continue,
            };
            if t.dst_port != self.src_port {
                continue;
            }
            let pl = t.payload.len().min(out.len());
            out[..pl].copy_from_slice(&t.payload[..pl]);
            return Some((t.seq, t.ack, t.flags, pl));
        }
        None
    }
}

/// Checks TCP end to end: open a connection to the host test server through
/// the gateway (SYN / SYN-ACK / ACK), send an HTTP request, read the
/// response, and close. This exercises the three-way handshake, sequence and
/// acknowledgement tracking, and the pseudo-header checksum over a reliable
/// local link. Skipped with no NIC or no reachable server.
pub fn check_tcp() {
    use crate::drivers::net::virtio_net::VirtioNet;
    use crate::include::uapi::tcp::{TCPHDR_ACK, TCPHDR_FIN, TCPHDR_PSH, TCPHDR_SYN};
    use crate::kernel::printk::BStr;

    let mut net = match VirtioNet::probe() {
        Some(n) => n,
        None => {
            pr_info!("rinux: tcp: no virtio-net device; skipping");
            return;
        }
    };

    let ctx = TcpCtx {
        our_ip: [10, 0, 2, 15],
        dst_ip: [10, 0, 2, 2],
        gw_mac: match arp_resolve(&mut net, &[10, 0, 2, 15], &[10, 0, 2, 2]) {
            Some(m) => m,
            None => {
                pr_err!("rinux: tcp: could not resolve the gateway MAC");
                return;
            }
        },
        src_port: 49152,
        dst_port: TCP_TEST_PORT,
    };

    // Three-way handshake: SYN -> SYN-ACK -> ACK.
    let isn = 0x1000u32;
    if !ctx.send(&mut net, isn, 0, TCPHDR_SYN, &[]) {
        pr_err!("rinux: tcp: SYN transmit failed");
        return;
    }
    let mut rbuf = [0u8; 1500];
    let (peer_seq, _ack, flags, _pl) = match ctx.recv(&mut net, &mut rbuf) {
        Some(x) => x,
        None => {
            pr_err!(
                "rinux: tcp: no SYN-ACK (no server on port {}?)",
                ctx.dst_port
            );
            return;
        }
    };
    if flags & (TCPHDR_SYN | TCPHDR_ACK) != (TCPHDR_SYN | TCPHDR_ACK) {
        pr_err!("rinux: tcp: expected SYN-ACK, got flags {:#x}", flags);
        return;
    }
    let mut rcv_nxt = peer_seq.wrapping_add(1);
    let mut snd_nxt = isn.wrapping_add(1);
    ctx.send(&mut net, snd_nxt, rcv_nxt, TCPHDR_ACK, &[]);

    // Send a minimal HTTP request.
    let req = b"GET / HTTP/1.0\r\n\r\n";
    ctx.send(&mut net, snd_nxt, rcv_nxt, TCPHDR_PSH | TCPHDR_ACK, req);
    snd_nxt = snd_nxt.wrapping_add(req.len() as u32);

    // Read the response: ACK each data segment, capture the status line.
    let mut status = [0u8; 48];
    let mut status_len = 0usize;
    let mut got_status = false;
    for _ in 0..16 {
        let (seq, _a, flags, pl) = match ctx.recv(&mut net, &mut rbuf) {
            Some(x) => x,
            None => break,
        };
        if pl > 0 && seq == rcv_nxt {
            if !got_status {
                let line = rbuf[..pl]
                    .split(|&c| c == b'\r' || c == b'\n')
                    .next()
                    .unwrap_or(&[]);
                status_len = line.len().min(status.len());
                status[..status_len].copy_from_slice(&line[..status_len]);
                got_status = line.starts_with(b"HTTP");
            }
            rcv_nxt = rcv_nxt.wrapping_add(pl as u32);
            ctx.send(&mut net, snd_nxt, rcv_nxt, TCPHDR_ACK, &[]);
        }
        if flags & TCPHDR_FIN != 0 {
            rcv_nxt = rcv_nxt.wrapping_add(1);
            ctx.send(&mut net, snd_nxt, rcv_nxt, TCPHDR_FIN | TCPHDR_ACK, &[]);
            break;
        }
        if got_status {
            ctx.send(&mut net, snd_nxt, rcv_nxt, TCPHDR_FIN | TCPHDR_ACK, &[]);
            break;
        }
    }

    if got_status {
        pr_info!(
            "rinux: tcp: ok (connected {}.{}.{}.{}:{}, response \"{}\")",
            ctx.dst_ip[0],
            ctx.dst_ip[1],
            ctx.dst_ip[2],
            ctx.dst_ip[3],
            ctx.dst_port,
            BStr(&status[..status_len])
        );
    } else {
        pr_err!("rinux: tcp: handshake ok but no HTTP response (no server?)");
    }
}

/// Checks the socket layer: bind a UDP socket, send a datagram to the host
/// echo server through the socket API, and read the echoed reply. Exercises
/// `UdpSocket::bind`/`send_to`/`recv_from` over the stack. Under QEMU user
/// networking the echo server is reachable at 10.0.2.2. Skipped with no NIC.
pub fn check_socket() {
    use crate::include::uapi::socket::SockAddrIn;
    use crate::kernel::printk::BStr;
    use crate::net::socket::UdpSocket;

    let mut sock = match UdpSocket::bind(49153) {
        Ok(s) => s,
        Err(_) => {
            pr_info!("rinux: socket: no usable NIC; skipping");
            return;
        }
    };

    // The server at 10.0.2.2:10082 echoes whatever we send it.
    let dst = SockAddrIn::new([10, 0, 2, 2], 10082);
    let msg = b"hello from a rinux UDP socket";
    if sock.send_to(msg, &dst.sin_addr, dst.port()).is_err() {
        pr_err!("rinux: socket: send_to failed");
        return;
    }

    let mut buf = [0u8; 256];
    match sock.recv_from(&mut buf) {
        Ok((n, src, port)) => pr_info!(
            "rinux: socket: ok (echo from {}.{}.{}.{}:{}: \"{}\")",
            src[0],
            src[1],
            src[2],
            src[3],
            port,
            BStr(&buf[..n])
        ),
        Err(_) => pr_err!("rinux: socket: no echo reply (no server?)"),
    }
}

/// Checks the memory character devices: read zeros from `/dev/zero`, confirm
/// `/dev/null` is at end-of-file and swallows writes, and that `/dev/full`
/// fails writes with `ENOSPC`. Exercises per-inode `file_operations` for
/// devices.
pub fn check_dev() {
    use crate::fs::vfs::{filp_open, vfs_read, vfs_write};
    use crate::include::linux::fs::{O_RDONLY, O_WRONLY};

    // /dev/zero yields zero bytes.
    let mut zbuf = [0xaau8; 32];
    let zeros = match filp_open(b"/dev/zero", O_RDONLY, 0) {
        Ok(mut f) => vfs_read(&mut f, &mut zbuf).unwrap_or(0) == 32 && zbuf.iter().all(|&b| b == 0),
        Err(_) => {
            pr_err!("rinux: dev: /dev/zero is missing");
            return;
        }
    };

    // /dev/null is always at EOF and discards writes.
    let (null_eof, null_wrote) = match filp_open(b"/dev/null", O_WRONLY, 0) {
        Ok(mut f) => {
            let wrote = vfs_write(&mut f, b"discard me").unwrap_or(0);
            let mut b = [0u8; 4];
            let eof = {
                let mut r = filp_open(b"/dev/null", O_RDONLY, 0).unwrap();
                vfs_read(&mut r, &mut b).unwrap_or(99) == 0
            };
            (eof, wrote)
        }
        Err(_) => {
            pr_err!("rinux: dev: /dev/null is missing");
            return;
        }
    };

    // /dev/full rejects writes with ENOSPC.
    let full_enospc = match filp_open(b"/dev/full", O_WRONLY, 0) {
        Ok(mut f) => vfs_write(&mut f, b"x").is_err(),
        Err(_) => false,
    };

    // /dev/urandom yields bytes that are not all identical.
    let random_ok = match filp_open(b"/dev/urandom", O_RDONLY, 0) {
        Ok(mut f) => {
            let mut rbuf = [0u8; 16];
            let n = vfs_read(&mut f, &mut rbuf).unwrap_or(0);
            n == 16 && rbuf.iter().any(|&b| b != rbuf[0])
        }
        Err(_) => false,
    };

    if zeros && null_eof && null_wrote == 10 && full_enospc && random_ok {
        pr_info!(
            "rinux: dev: ok (/dev/zero zeros, /dev/null swallowed {} bytes at EOF, /dev/full ENOSPC, /dev/urandom varied)",
            null_wrote
        );
    } else if zeros && null_eof && null_wrote == 10 && full_enospc {
        pr_info!("rinux: dev: ok (zero/null/full ok; /dev/urandom read failed or looked uniform)");
    } else {
        pr_err!(
            "rinux: dev: device semantics wrong (zero={} null_eof={} wrote={} full={})",
            zeros,
            null_eof,
            null_wrote,
            full_enospc
        );
    }
}

/// Checks the 8250/16550 UART: the scratch-register test (detects a 16450+)
/// and the internal loopback (a transmitted byte is received back). Done on
/// COM1 with its registers saved and restored, so the console is undisturbed.
pub fn check_8250() {
    use crate::drivers::tty::serial::serial8250::{COM1, loopback_test, scratch_test};

    let scr = scratch_test(COM1);
    match loopback_test(COM1, 0x55) {
        Some(0x55) => pr_info!(
            "rinux: 8250: ok (COM1 scratch register {}, loopback 0x55 ok)",
            if scr { "ok" } else { "absent" }
        ),
        Some(v) => pr_err!("rinux: 8250: loopback mismatch (sent 0x55, got {:#x})", v),
        None => pr_info!("rinux: 8250: no loopback response (no 16550 at COM1?)"),
    }
}

/// Checks the i8042 PS/2 controller: run its built-in self-test (which
/// should return 0x55) and the keyboard-interface test (0x00). Linux does
/// this when probing the controller; rinux checks it directly.
pub fn check_i8042() {
    use crate::drivers::input::serio::i8042::{RET_CTL_TEST, controller_selftest, keyboard_test};

    match controller_selftest() {
        Some(RET_CTL_TEST) => {
            let kbd = keyboard_test();
            pr_info!(
                "rinux: i8042: ok (controller self-test 0x55, keyboard interface {})",
                match kbd {
                    Some(0) => "ok",
                    Some(_) => "error",
                    None => "no response",
                }
            );
        }
        Some(v) => pr_err!("rinux: i8042: self-test returned {:#x} (expected 0x55)", v),
        None => pr_info!("rinux: i8042: no response (no PS/2 controller?)"),
    }
}

/// Checks interrupt-driven keyboard input: enable IRQ 1 and wait briefly for
/// a keypress, decoding its scancode to ASCII. This needs a key to arrive, so
/// `boot-test.sh` injects one through the QEMU monitor; with no key it simply
/// reports the timeout and the boot continues.
pub fn check_keyboard() {
    use crate::drivers::input::keyboard::atkbd::{kbd_init, read_scancode, scancode_to_ascii};

    kbd_init();
    let deadline = jiffies() + 5 * HZ;
    let mut seen = 0u32;
    while jiffies() < deadline {
        if let Some(code) = read_scancode() {
            seen += 1;
            if let Some(c) = scancode_to_ascii(code) {
                pr_info!(
                    "rinux: keyboard: ok (IRQ1 delivered scancode {:#04x} = '{}')",
                    code,
                    c as char
                );
                return;
            }
        }
        core::hint::spin_loop();
    }
    pr_info!(
        "rinux: keyboard: no key within 5s; skipping (IRQ1 count {}, scancodes seen {})",
        kstat_irqs(1),
        seen
    );
}

/// Checks the CMOS real-time clock: read the wall-clock time and report it.
/// Linux reads the RTC at boot to seed the system clock; rinux reads it as a
/// check. QEMU's RTC reflects the host time (UTC by default).
pub fn check_rtc() {
    use crate::drivers::rtc::rtc_cmos::read_time;

    let t = read_time();
    if t.sec > 59
        || t.min > 59
        || t.hour > 23
        || t.mday == 0
        || t.mday > 31
        || t.month == 0
        || t.month > 12
    {
        pr_err!(
            "rinux: rtc: implausible reading {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            t.year,
            t.month,
            t.mday,
            t.hour,
            t.min,
            t.sec
        );
        return;
    }
    pr_info!(
        "rinux: rtc: ok (CMOS clock reads {:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC)",
        t.year,
        t.month,
        t.mday,
        t.hour,
        t.min,
        t.sec
    );
}

/// Checks the crypto library: hash a known input with the ported SHA-256 and
/// compare against the FIPS 180-4 digest. Linux validates its crypto drivers
/// with known-answer tests; rinux checks the one it has at boot.
pub fn check_crypto() {
    use crate::kernel::printk::FmtBuf;
    use core::fmt::Write;
    use rinux_lib::sha256::sha256;

    // FIPS 180-4: SHA-256("abc").
    const EXPECT: [u8; 32] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    let digest = sha256(b"abc");
    let mut hex = FmtBuf::<16>::new();
    for b in &digest[..6] {
        let _ = write!(hex, "{:02x}", b);
    }
    if digest != EXPECT {
        pr_err!(
            "rinux: crypto: SHA-256 known-answer test failed (got {}...)",
            hex.as_str()
        );
        return;
    }

    // RFC 3174: SHA-1("abc").
    const SHA1_EXPECT: [u8; 20] = [
        0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50, 0xc2,
        0x6c, 0x9c, 0xd0, 0xd8, 0x9d,
    ];
    let sha1_digest = rinux_lib::sha1::sha1(b"abc");
    if sha1_digest != SHA1_EXPECT {
        let mut h1 = FmtBuf::<16>::new();
        for b in &sha1_digest[..6] {
            let _ = write!(h1, "{:02x}", b);
        }
        pr_err!(
            "rinux: crypto: SHA-1 known-answer test failed (got {}...)",
            h1.as_str()
        );
        return;
    }

    // RFC 1321: MD5("abc").
    const MD5_EXPECT: [u8; 16] = [
        0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1, 0x7f,
        0x72,
    ];
    if rinux_lib::md5::md5(b"abc") != MD5_EXPECT {
        pr_err!("rinux: crypto: MD5 known-answer test failed");
        return;
    }

    pr_info!(
        "rinux: crypto: ok (SHA-256(\"abc\") = {}..., SHA-1 and MD5 all match the published vectors)",
        hex.as_str()
    );
}

/// The lib's `RandomSource` and printk hooks, driven by kernel code.
///
/// A UUID drawn from the kernel pool through `PoolRandom` must carry the
/// version 4 and DCE variant bits. A hex dump sent through the kernel's line
/// sink (`lib_line`) must arrive as one line with the expected text.
pub fn check_lib_hooks() {
    use rinux_lib::hexdump::{DumpOpts, DumpPrefix, print_hex_dump};
    use rinux_lib::printk::LOGLEVEL_INFO;
    use rinux_lib::uuid::generate_random_uuid_from;

    use crate::drivers::char::random::PoolRandom;
    use crate::kernel::printk::lib_line;

    let mut uuid = [0u8; 16];
    generate_random_uuid_from(&mut uuid, &mut PoolRandom);
    if uuid[6] >> 4 == 4 && uuid[8] & 0xc0 == 0x80 {
        pr_info!("rinux: random uuid: ok (version 4, DCE variant)");
    } else {
        pr_err!("rinux: random uuid: bad version or variant bits");
    }

    let opts = DumpOpts {
        prefix_str: b"rinux-dump: ",
        prefix_type: DumpPrefix::None,
        rowsize: 16,
        groupsize: 1,
        base: 0,
        ascii: false,
    };
    let mut lines = 0;
    let mut matched = false;
    print_hex_dump(LOGLEVEL_INFO, opts, b"abc", &mut |level, line| {
        lines += 1;
        matched = level == LOGLEVEL_INFO && line.starts_with(b"rinux-dump: 61 62 63");
        lib_line(level, line);
    });
    if lines == 1 && matched {
        pr_info!("rinux: printk sink: ok (hex dump of \"abc\" through lib_line)");
    } else {
        pr_err!("rinux: printk sink: hex dump lines {} matched {}", lines, matched);
    }
}
