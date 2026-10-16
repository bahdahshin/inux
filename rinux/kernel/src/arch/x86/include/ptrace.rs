// SPDX-License-Identifier: GPL-2.0
//! Saved registers.
//!
//! Ported from the x86-64 part of `arch/x86/include/asm/ptrace.h`.

/// `struct pt_regs`: the registers the entry code saves on the stack.
///
/// The order matches what `PUSH_REGS` pushes in `entry_64` followed by the
/// hardware exception frame, so a pointer to the saved registers can be
/// used as a `&mut PtRegs`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PtRegs {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub bp: u64,
    pub bx: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub ax: u64,
    pub cx: u64,
    pub dx: u64,
    pub si: u64,
    pub di: u64,
    /// Syscall number on syscall entry, error code on exceptions, or -1.
    pub orig_ax: u64,
    // The hardware exception frame.
    pub ip: u64,
    pub cs: u64,
    pub flags: u64,
    pub sp: u64,
    pub ss: u64,
}

/// Offset of `orig_ax` in `PtRegs`, used by the entry code.
pub const ORIG_RAX: usize = 15 * 8;

/// `user_mode()`: true if the registers were saved in user mode.
#[inline]
pub fn user_mode(regs: &PtRegs) -> bool {
    regs.cs & 3 != 0
}
