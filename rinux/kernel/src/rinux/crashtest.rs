// SPDX-License-Identifier: GPL-2.0
//! `rinux.crashtest=`: deliberately crash at the end of boot.
//!
//! Linux tests its crash paths with LKDTM (`drivers/misc/lkdtm`), driven
//! through debugfs. rinux has no debugfs yet, so this boot parameter
//! triggers the same kinds of faults so `boot-test.sh` can check the oops
//! output. It is rinux-only and does nothing unless given.
//!
//! Values: `divide_error`, `invalid_op`, `page_fault`,
//! `general_protection`, `null_deref`, `int3`, `bug`, `panic`.

use core::arch::asm;
use core::sync::atomic::{AtomicU8, Ordering};

use crate::kernel::panic::panic;

const NONE: u8 = 0;
const TESTS: &[(&str, u8)] = &[
    ("divide_error", 1),
    ("invalid_op", 2),
    ("page_fault", 3),
    ("general_protection", 5),
    ("null_deref", 9),
    ("int3", 6),
    ("bug", 7),
    ("panic", 8),
];

static CRASHTEST: AtomicU8 = AtomicU8::new(NONE);

fn setup_crashtest(val: Option<&[u8]>) -> i32 {
    let Some(val) = val else { return -1 };
    match TESTS.iter().find(|(name, _)| name.as_bytes() == val) {
        Some(&(_, id)) => {
            CRASHTEST.store(id, Ordering::Relaxed);
            0
        }
        None => -1,
    }
}

crate::early_param!("rinux.crashtest", setup_crashtest);

/// Runs the requested crash, if any.
pub fn run() {
    let id = CRASHTEST.load(Ordering::Relaxed);
    let Some(&(name, _)) = TESTS.iter().find(|(_, t)| *t == id) else {
        return;
    };
    pr_crit!("rinux: crashtest: {}", name);
    // SAFETY (all arms): each instruction faults on purpose; the handlers
    // oops and never return here, except int3, which oopses too.
    match id {
        1 => unsafe {
            asm!("xor %ecx, %ecx", "div %ecx", out("eax") _, out("ecx") _, out("edx") _, options(att_syntax, nostack))
        },
        2 => unsafe { asm!("ud2", options(nomem, nostack)) },
        // 64 GiB: past the identity map of the first 4 GiB.
        3 => unsafe {
            asm!("movq (%rax), %rax", inout("rax") 0x10_0000_0000u64 => _, options(att_syntax, nostack))
        },
        // A non-canonical address.
        5 => unsafe {
            asm!("movq (%rax), %rax", inout("rax") 0x8000_0000_0000_0000u64 => _, options(att_syntax, nostack))
        },
        // Page 0 is unmapped once memory management is up.
        9 => unsafe { asm!("movq $0, (%rax)", in("rax") 0u64, options(att_syntax, nostack)) },
        6 => unsafe { asm!("int3", options(nomem, nostack)) },
        7 => {
            let v: [u32; 1] = [0];
            let i = core::hint::black_box(4usize);
            let _ = v[i];
        }
        8 => panic(format_args!("rinux.crashtest=panic")),
        _ => {}
    }
}
