// SPDX-License-Identifier: GPL-2.0
//! Signal delivery on x86-64.
//!
//! A port of `arch/x86/kernel/signal_64.c` (`x64_setup_rt_frame`,
//! `__x64_sys_rt_sigreturn`) and `get_sigframe()`/`do_signal()` of
//! `arch/x86/kernel/signal.c`: when a syscall returns to a task with a pending,
//! unblocked signal that has a handler, build a `struct rt_sigframe` on the
//! user stack holding the interrupted register state, then re-point the task at
//! the handler. The handler's return trampoline (`sa_restorer`) invokes
//! `rt_sigreturn`, which restores that state.
//!
//! Simplified from Linux's: no FPU/extended state in the frame (rinux runs
//! user space soft-float, so `fpstate` is NULL), no alternate signal stack, no
//! `SA_SIGINFO` payload beyond the zeroed `siginfo`, and no shadow stack.

use core::sync::atomic::Ordering;

use crate::arch::x86::include::ptrace::{PtRegs, user_mode};
use crate::arch::x86::include::segment::{__USER_CS, __USER_DS};
use crate::kernel::signal::{
    BLOCKED, KSigaction, PENDING, SA_RESTORER, SA_SIGINFO, SIG_UNBLOCKABLE, SIGHAND,
};

// `struct rt_sigframe` field offsets, and the `struct sigcontext` offsets
// within `uc.uc_mcontext`.
const PRETCODE_OFF: u64 = 0;
const UC_OFF: u64 = 8;
const SIGCONTEXT_OFF: u64 = UC_OFF + 40; // ucontext.uc_mcontext
const UC_SIGMASK_OFF: u64 = SIGCONTEXT_OFF + 256; // after the 256-byte sigcontext
const INFO_OFF: u64 = UC_OFF + 304; // after the whole ucontext
const FRAME_SIZE: u64 = 448; // >= INFO_OFF + sizeof(siginfo)=128

// `struct sigcontext_64` field offsets.
const SC_R8: u64 = 0;
const SC_R9: u64 = 8;
const SC_R10: u64 = 16;
const SC_R11: u64 = 24;
const SC_R12: u64 = 32;
const SC_R13: u64 = 40;
const SC_R14: u64 = 48;
const SC_R15: u64 = 56;
const SC_DI: u64 = 64;
const SC_SI: u64 = 72;
const SC_BP: u64 = 80;
const SC_BX: u64 = 88;
const SC_DX: u64 = 96;
const SC_AX: u64 = 104;
const SC_CX: u64 = 112;
const SC_SP: u64 = 120;
const SC_IP: u64 = 128;
const SC_FLAGS: u64 = 136;
const SC_CS: u64 = 144;
const SC_SS: u64 = 150;
const SC_FPSTATE: u64 = 184;

/// `X86_EFLAGS_FIXED | X86_EFLAGS_IF`: the reserved bit and interrupt flag,
/// forced on when restoring so user space keeps running with interrupts.
const EFLAGS_FORCED_ON: u64 = 0x202;

#[inline]
unsafe fn wu64(addr: u64, v: u64) {
    // SAFETY: the caller guarantees `addr` is a writable user address.
    unsafe { (addr as *mut u64).write_unaligned(v) };
}
#[inline]
unsafe fn wu16(addr: u64, v: u16) {
    // SAFETY: as `wu64`.
    unsafe { (addr as *mut u16).write_unaligned(v) };
}
#[inline]
unsafe fn wu32(addr: u64, v: u32) {
    // SAFETY: as `wu64`.
    unsafe { (addr as *mut u32).write_unaligned(v) };
}
#[inline]
unsafe fn ru64(addr: u64) -> u64 {
    // SAFETY: the caller guarantees `addr` is a readable user address.
    unsafe { (addr as *const u64).read_unaligned() }
}

/// `x64_setup_rt_frame()`: build the signal frame and point `regs` at the
/// handler. Returns false if the action lacks a restorer (x86-64 requires one).
fn setup_rt_frame(sig: u32, ka: &KSigaction, regs: &mut PtRegs) -> bool {
    if ka.flags & SA_RESTORER == 0 {
        return false;
    }

    // get_sigframe(): skip the 128-byte red zone, drop the frame, align to 16
    // then back off 8 so the handler sees the ABI's `rsp % 16 == 8`.
    let sp = regs.sp - 128 - FRAME_SIZE;
    let frame = (sp & !0xf) - 8;
    let sc = frame + SIGCONTEXT_OFF;

    // SAFETY: syscall context on the user's page tables; a bad frame address
    // faults and kills the task, as a stack overflow should.
    unsafe {
        wu64(frame + PRETCODE_OFF, ka.restorer);
        wu64(frame + UC_OFF, 0); // uc_flags
        wu64(frame + UC_OFF + 8, 0); // uc_link
        wu64(frame + UC_OFF + 16, 0); // uc_stack.ss_sp
        wu64(frame + UC_OFF + 24, 0); // uc_stack.ss_flags/ss_size
        wu64(frame + UC_OFF + 32, 0);

        wu64(sc + SC_R8, regs.r8);
        wu64(sc + SC_R9, regs.r9);
        wu64(sc + SC_R10, regs.r10);
        wu64(sc + SC_R11, regs.r11);
        wu64(sc + SC_R12, regs.r12);
        wu64(sc + SC_R13, regs.r13);
        wu64(sc + SC_R14, regs.r14);
        wu64(sc + SC_R15, regs.r15);
        wu64(sc + SC_DI, regs.di);
        wu64(sc + SC_SI, regs.si);
        wu64(sc + SC_BP, regs.bp);
        wu64(sc + SC_BX, regs.bx);
        wu64(sc + SC_DX, regs.dx);
        wu64(sc + SC_AX, regs.ax);
        wu64(sc + SC_CX, regs.cx);
        wu64(sc + SC_SP, regs.sp);
        wu64(sc + SC_IP, regs.ip);
        wu64(sc + SC_FLAGS, regs.flags);
        wu16(sc + SC_CS, regs.cs as u16);
        wu16(sc + SC_SS, regs.ss as u16);
        wu64(sc + SC_FPSTATE, 0); // no FPU state

        wu64(frame + UC_SIGMASK_OFF, BLOCKED.load(Ordering::Relaxed));

        // For an SA_SIGINFO handler, fill the siginfo it receives as its
        // second argument. The signal was raised by kill(), so the code is
        // SI_USER and the sender fields come from the current task. x86-64
        // `siginfo`: si_signo@0, si_errno@4, si_code@8, then si_pid@16,
        // si_uid@20 for the SI_USER layout.
        if ka.flags & SA_SIGINFO != 0 {
            let info = frame + INFO_OFF;
            let (pid, uid) = (
                (*crate::include::linux::sched::current()).pid as u32,
                crate::kernel::cred::current_cred().uid,
            );
            wu32(info, sig); // si_signo
            wu32(info + 4, 0); // si_errno
            wu32(info + 8, 0); // si_code = SI_USER
            wu32(info + 16, pid); // si_pid
            wu32(info + 20, uid); // si_uid
        }
    }

    // Registers for the handler: handler(sig, &info, &uc) with sp at the frame.
    regs.di = sig as u64;
    regs.ax = 0;
    regs.si = frame + INFO_OFF;
    regs.dx = frame + UC_OFF;
    regs.ip = ka.handler;
    regs.sp = frame;
    regs.cs = __USER_CS as u64;
    regs.ss = __USER_DS as u64;
    true
}

/// `do_signal()`: deliver one pending, unblocked signal, if any. Called on the
/// return path of every syscall from user space.
pub fn do_signal(regs: &mut PtRegs) {
    if !user_mode(regs) {
        return;
    }
    let deliverable = PENDING.load(Ordering::Relaxed) & !BLOCKED.load(Ordering::Relaxed);
    if deliverable == 0 {
        return;
    }
    let sig = deliverable.trailing_zeros() + 1; // lowest pending signal, 1-based
    PENDING.fetch_and(!(1u64 << (sig - 1)), Ordering::Relaxed);

    let ka = SIGHAND.lock()[(sig - 1) as usize];
    // SIG_DFL (0) and SIG_IGN (1): rinux has no default actions, so ignore.
    if ka.handler == 0 || ka.handler == 1 {
        return;
    }
    if setup_rt_frame(sig, &ka, regs) {
        // Block this signal (and the action's mask) while the handler runs.
        BLOCKED.fetch_or((1u64 << (sig - 1)) | ka.mask, Ordering::Relaxed);
    }
}

/// `__x64_sys_rt_sigreturn()`: restore the pre-signal register state from the
/// frame the handler's restorer left on the stack, and return the restored
/// `ax` so the dispatcher does not clobber it.
pub fn sys_rt_sigreturn(regs: &mut PtRegs) -> i64 {
    let frame = regs.sp - 8;
    let sc = frame + SIGCONTEXT_OFF;
    // SAFETY: syscall context on the user's page tables; a forged frame faults.
    unsafe {
        regs.r8 = ru64(sc + SC_R8);
        regs.r9 = ru64(sc + SC_R9);
        regs.r10 = ru64(sc + SC_R10);
        regs.r11 = ru64(sc + SC_R11);
        regs.r12 = ru64(sc + SC_R12);
        regs.r13 = ru64(sc + SC_R13);
        regs.r14 = ru64(sc + SC_R14);
        regs.r15 = ru64(sc + SC_R15);
        regs.di = ru64(sc + SC_DI);
        regs.si = ru64(sc + SC_SI);
        regs.bp = ru64(sc + SC_BP);
        regs.bx = ru64(sc + SC_BX);
        regs.dx = ru64(sc + SC_DX);
        regs.ax = ru64(sc + SC_AX);
        regs.cx = ru64(sc + SC_CX);
        regs.sp = ru64(sc + SC_SP);
        regs.ip = ru64(sc + SC_IP);
        regs.flags = ru64(sc + SC_FLAGS) | EFLAGS_FORCED_ON;
        // Force a valid, user-mode CS/SS rather than trusting the frame.
        regs.cs = __USER_CS as u64;
        regs.ss = __USER_DS as u64;

        let mask = ru64(frame + UC_SIGMASK_OFF) & !SIG_UNBLOCKABLE;
        BLOCKED.store(mask, Ordering::Relaxed);
    }
    regs.ax as i64
}
