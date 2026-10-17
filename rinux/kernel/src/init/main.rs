// SPDX-License-Identifier: GPL-2.0-only
//! `start_kernel()` and early parameters.
//!
//! A partial port of `init/main.c`.
//!
//! Copyright (C) 1991, 1992  Linus Torvalds

use core::sync::atomic::{AtomicBool, Ordering};

use rinux_lib::cmdline::get_option;
use rinux_lib::errno::EINVAL;
use rinux_lib::string::strscpy;

use crate::arch::x86::include::irqflags::{local_irq_disable, local_irq_enable};
use crate::arch::x86::kernel::alternative::alternative_instructions;
use crate::arch::x86::kernel::cpu::common::{boot_cpu_data, print_cpu_info};
use crate::arch::x86::kernel::irqinit::init_irq;
use crate::arch::x86::kernel::setup::setup_arch;
use crate::arch::x86::kernel::time::time_init;
use crate::arch::x86::kernel::traps::trap_init;
use crate::include::linux::init::setup_params;
use crate::init::calibrate::calibrate_delay;
use crate::init::version::LINUX_BANNER;
use crate::kernel::locking::SpinLock;
use crate::kernel::params::{parameq, parse_args};
use crate::kernel::printk::{
    BStr, CONSOLE_LOGLEVEL, CONSOLE_LOGLEVEL_DEBUG, CONSOLE_LOGLEVEL_QUIET,
};
use crate::kernel::sched::core::{cpu_idle_loop, sched_init};
use crate::mm::mm_init::mm_core_init;

/// `COMMAND_LINE_SIZE` from `arch/x86/include/asm/setup.h`.
pub const COMMAND_LINE_SIZE: usize = 2048;

/// `boot_command_line`: the command line from the boot loader, untouched.
pub static BOOT_COMMAND_LINE: SpinLock<[u8; COMMAND_LINE_SIZE]> =
    SpinLock::new([0; COMMAND_LINE_SIZE]);

/// A copy of `boot_command_line`, for procfs.
pub fn boot_command_line() -> [u8; COMMAND_LINE_SIZE] {
    *BOOT_COMMAND_LINE.lock_irqsave()
}

fn debug_kernel(_str: Option<&[u8]>) -> i32 {
    CONSOLE_LOGLEVEL.store(CONSOLE_LOGLEVEL_DEBUG, Ordering::Relaxed);
    0
}

fn quiet_kernel(_str: Option<&[u8]>) -> i32 {
    CONSOLE_LOGLEVEL.store(CONSOLE_LOGLEVEL_QUIET, Ordering::Relaxed);
    0
}

crate::early_param!("debug", debug_kernel);
crate::early_param!("quiet", quiet_kernel);

fn loglevel(str: Option<&[u8]>) -> i32 {
    // Only update the level when a valid one was passed, to avoid blind
    // crashes (loglevel=0) that are hard to debug.
    let mut s = str.unwrap_or(b"");
    let mut newlevel = 0;
    if get_option(&mut s, Some(&mut newlevel)) != 0 {
        CONSOLE_LOGLEVEL.store(newlevel, Ordering::Relaxed);
        return 0;
    }
    EINVAL.to_errno()
}

crate::early_param!("loglevel", loglevel);

/// `do_early_param()`: runs every early handler whose name matches.
fn do_early_param(param: &[u8], val: Option<&[u8]>) -> i32 {
    for p in setup_params() {
        if p.early && parameq(param, p.str.as_bytes()) && (p.setup_func)(val) != 0 {
            pr_warn!("Malformed early option '{}'", BStr(param));
        }
    }
    // We accept everything at this stage.
    0
}

/// `parse_early_options()`.
pub fn parse_early_options(cmdline: &[u8]) {
    parse_args(cmdline, do_early_param);
}

/// `parse_early_param()`: runs the early parameter handlers, once.
pub fn parse_early_param() {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.load(Ordering::Relaxed) {
        return;
    }
    // All fall through to do_early_param.
    let mut tmp_cmdline = [0u8; COMMAND_LINE_SIZE];
    let _ = strscpy(&mut tmp_cmdline, &*BOOT_COMMAND_LINE.lock());
    parse_early_options(&tmp_cmdline);
    DONE.store(true, Ordering::Relaxed);
}

/// `late_time_init`: set by `time_init()`, run once interrupts are on.
static LATE_TIME_INIT: SpinLock<Option<fn()>> = SpinLock::new(None);

/// Sets `late_time_init`.
pub fn set_late_time_init(f: fn()) {
    *LATE_TIME_INIT.lock_irqsave() = Some(f);
}

/// `vfs_init()`: mount the rootfs, bring up procfs, and unpack the
/// initramfs module if the boot loader supplied one.
fn vfs_init() {
    crate::fs::ramfs::mount_rootfs();
    crate::fs::proc::proc_root_init();
    crate::drivers::char::mem::mem_init();
    crate::drivers::char::random::random_init();
    crate::fs::sysfs::ksysfs_init();
    match crate::init::initramfs::populate_rootfs() {
        Some(n) => pr_info!("initramfs: unpacked {} entries into rootfs", n),
        None => pr_info!("initramfs: no archive supplied by the boot loader"),
    }
}

/// `arch_cpu_finalize_init()` for x86 without SMP.
fn arch_cpu_finalize_init() {
    print_cpu_info(&boot_cpu_data());
    alternative_instructions();
}

/// `start_kernel()`: the architecture-independent entry point.
pub fn start_kernel() -> ! {
    local_irq_disable();

    pr_notice!("{}", LINUX_BANNER);
    let mut command_line = [0u8; COMMAND_LINE_SIZE];
    setup_arch(&mut command_line);
    pr_notice!("Kernel command line: {}", BStr(&command_line));

    mm_core_init();

    // Turn the boot context into the idle task swapper/0.
    // SAFETY: the boot CPU runs this once; IDLE_TASK is its static.
    sched_init(IDLE_TASK.get());

    trap_init();
    // rcu_init() and workqueue_init() come with later steps.
    init_irq();
    time_init();

    local_irq_enable();

    if let Some(late_time_init) = *LATE_TIME_INIT.lock_irqsave() {
        late_time_init();
    }
    calibrate_delay();
    arch_cpu_finalize_init();

    // Anchor CLOCK_REALTIME to the RTC now that jiffies is ticking.
    crate::kernel::time::timekeeping::timekeeping_init();

    // rest_init() would start init and kthreadd here.
    vfs_init();

    crate::rinux::crashtest::run();
    crate::rinux::selftest::check_timer();
    crate::rinux::selftest::check_rtc();
    crate::rinux::selftest::check_i8042();
    crate::rinux::selftest::check_8250();
    crate::rinux::selftest::check_crypto();
    crate::rinux::selftest::check_lib_hooks();
    crate::rinux::selftest::check_mm();
    crate::rinux::selftest::check_sched();
    crate::rinux::selftest::check_preempt();
    crate::rinux::selftest::check_fs();
    crate::rinux::selftest::check_dev();
    crate::rinux::selftest::check_pci();
    crate::rinux::selftest::check_virtio();
    crate::rinux::selftest::check_virtio_console();
    crate::rinux::selftest::check_block();
    crate::rinux::selftest::check_partitions();
    crate::rinux::selftest::check_gpt();
    crate::rinux::selftest::check_ext2();
    crate::rinux::selftest::check_ext4();
    crate::rinux::selftest::check_fat();
    crate::rinux::selftest::check_net();
    crate::rinux::selftest::check_ping();
    crate::rinux::selftest::check_dhcp();
    crate::rinux::selftest::check_tcp();
    crate::rinux::selftest::check_socket();
    crate::rinux::selftest::check_user();
    crate::rinux::selftest::check_keyboard();
    pr_notice!("rinux: reached the end of the ported boot path, idling");
    // cpu_startup_entry() -> do_idle(): become the idle loop.
    cpu_idle_loop()
}

/// The boot CPU's idle task (`swapper/0`).
static IDLE_TASK: crate::kernel::locking::RacyCell<crate::include::linux::sched::TaskStruct> =
    crate::kernel::locking::RacyCell::new(crate::include::linux::sched::TaskStruct::new());
