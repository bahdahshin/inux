// SPDX-License-Identifier: GPL-2.0-only
//! The INT3 self-test.
//!
//! A partial port of `arch/x86/kernel/alternative.c`: `int3_selftest()`,
//! which proves the breakpoint path works end to end, with the call
//! emulation helpers of `arch/x86/include/asm/text-patching.h`. Code
//! patching itself (alternatives, static calls, jump labels) is not ported.

use core::arch::global_asm;

use crate::arch::x86::include::kdebug::{DIE_INT3, DieArgs};
use crate::arch::x86::include::ptrace::{PtRegs, user_mode};
use crate::kernel::notifier::{
    NOTIFY_DONE, NOTIFY_STOP, NotifierBlock, register_die_notifier, unregister_die_notifier,
};

/// `INT3_INSN_SIZE`.
const INT3_INSN_SIZE: u64 = 1;
/// `CALL_INSN_SIZE`.
const CALL_INSN_SIZE: u64 = 5;

/// `int3_emulate_push()`: pushes onto the interrupted stack, which the
/// entry code left a gap below.
fn int3_emulate_push(regs: &mut PtRegs, val: u64) {
    regs.sp -= 8;
    // SAFETY: the int3 entry stub copied the exception frame down by six
    // words when coming from the kernel, so regs.sp now points into that
    // gap rather than at live data.
    unsafe { (regs.sp as *mut u64).write(val) };
}

/// `int3_emulate_jmp()`.
fn int3_emulate_jmp(regs: &mut PtRegs, ip: u64) {
    regs.ip = ip;
}

/// `int3_emulate_call()`: makes the INT3 behave like a CALL to `func`
/// that returns to `ip`.
fn int3_emulate_call(regs: &mut PtRegs, ip: u64, func: u64) {
    int3_emulate_push(regs, ip);
    int3_emulate_jmp(regs, func);
}

unsafe extern "C" {
    fn int3_selftest_asm(ptr: *mut u32);
    fn int3_selftest_callee(ptr: *mut u32);
}

global_asm!(
    r#"
    .pushsection .text, "ax"
    .type int3_selftest_asm, @function
int3_selftest_asm:
    /*
     * INT3 padded with NOP to CALL_INSN_SIZE. The INT3 triggers an
     * exception, then the notifier emulates a call to
     * int3_selftest_callee().
     */
    int3; nop; nop; nop; nop
    ret
    .size int3_selftest_asm, . - int3_selftest_asm

    .type int3_selftest_callee, @function
int3_selftest_callee:
    movl $0x1234, (%rdi)
    ret
    .size int3_selftest_callee, . - int3_selftest_callee
    .popsection
"#,
    options(att_syntax),
);

fn int3_exception_notify(_nb: &NotifierBlock<DieArgs>, val: u64, args: &mut DieArgs) -> i32 {
    let selftest = int3_selftest_asm as *const () as u64;
    if args.regs.is_null() || val != DIE_INT3 {
        return NOTIFY_DONE;
    }
    // SAFETY: notify_die() passes the registers of the exception being
    // handled on this CPU.
    let regs = unsafe { &mut *args.regs };
    if user_mode(regs) || regs.ip - INT3_INSN_SIZE != selftest {
        return NOTIFY_DONE;
    }
    let ip = regs.ip - INT3_INSN_SIZE + CALL_INSN_SIZE;
    int3_emulate_call(regs, ip, int3_selftest_callee as *const () as u64);
    NOTIFY_STOP
}

static INT3_EXCEPTION_NB: NotifierBlock<DieArgs> = NotifierBlock {
    notifier_call: int3_exception_notify,
    priority: i32::MAX - 1, // last
};

/// `int3_selftest()`: "basically `int3_selftest_callee(&val)`, but really
/// complicated".
fn int3_selftest() {
    let mut val: u32 = 0;
    rinux_lib::bug_on!(register_die_notifier(&INT3_EXCEPTION_NB).is_err());
    // SAFETY: int3_selftest_asm follows the C ABI and only writes *ptr,
    // through the emulated call.
    unsafe { int3_selftest_asm(&mut val) };
    rinux_lib::bug_on!(val != 0x1234);
    let _ = unregister_die_notifier(&INT3_EXCEPTION_NB);
}

/// `alternative_instructions()`: no patching yet, just the self-test.
pub fn alternative_instructions() {
    int3_selftest();
}
