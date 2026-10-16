// SPDX-License-Identifier: GPL-2.0-only
//! The interrupt descriptor table.
//!
//! A partial port of `arch/x86/kernel/idt.c`. Not yet ported: the APIC and
//! IPI vectors (`apic_idts`, which need the local APIC), the 32-bit compat
//! `int $0x80` gate, and the read-only alias in the CPU entry area.

use crate::arch::x86::entry::entry_64::*;
use crate::arch::x86::include::desc::{
    DescPtr, GATE_INTERRUPT, GateStruct, IDT_ENTRIES, IdtBits, IdtData, idt_init_desc, load_idt,
};
use crate::arch::x86::include::irq_vectors::{
    FIRST_EXTERNAL_VECTOR, FIRST_SYSTEM_VECTOR, NR_VECTORS,
};
use crate::arch::x86::include::segment::__KERNEL_CS;
use crate::arch::x86::include::trapnr::*;
use crate::arch::x86::kernel::cpu::common::{
    IST_INDEX_DB, IST_INDEX_DF, IST_INDEX_MCE, IST_INDEX_NMI,
};
use crate::kernel::locking::RacyCell;

const DPL0: u16 = 0x0;
const DPL3: u16 = 0x3;
const DEFAULT_STACK: u16 = 0;

const fn g(
    vector: u32,
    addr: unsafe extern "C" fn(),
    ist: u16,
    typ: u16,
    dpl: u16,
    segment: u16,
) -> IdtData {
    IdtData {
        vector,
        segment,
        bits: IdtBits::new(ist, typ, dpl, true),
        addr,
    }
}

/// `INTG()`: interrupt gate.
const fn intg(vector: u32, addr: unsafe extern "C" fn()) -> IdtData {
    g(
        vector,
        addr,
        DEFAULT_STACK,
        GATE_INTERRUPT,
        DPL0,
        __KERNEL_CS,
    )
}

/// `SYSG()`: system interrupt gate, reachable from user mode.
const fn sysg(vector: u32, addr: unsafe extern "C" fn()) -> IdtData {
    g(
        vector,
        addr,
        DEFAULT_STACK,
        GATE_INTERRUPT,
        DPL3,
        __KERNEL_CS,
    )
}

/// `ISTG()`: interrupt gate on an interrupt stack. The descriptor counts
/// IST slots from 1.
const fn istg(vector: u32, addr: unsafe extern "C" fn(), ist: usize) -> IdtData {
    g(
        vector,
        addr,
        ist as u16 + 1,
        GATE_INTERRUPT,
        DPL0,
        __KERNEL_CS,
    )
}

/// Early traps on the default stack: interrupt stacks need the TSS, which
/// `cpu_init_exception_handling()` sets up later.
static EARLY_IDTS: [IdtData; 2] = [
    intg(X86_TRAP_DB, asm_exc_debug),
    sysg(X86_TRAP_BP, asm_exc_int3),
];

/// The default traps, set up in `trap_init()`.
static DEF_IDTS: [IdtData; 17] = [
    intg(X86_TRAP_DE, asm_exc_divide_error),
    istg(X86_TRAP_NMI, asm_exc_nmi, IST_INDEX_NMI),
    intg(X86_TRAP_BR, asm_exc_bounds),
    intg(X86_TRAP_UD, asm_exc_invalid_op),
    intg(X86_TRAP_NM, asm_exc_device_not_available),
    intg(X86_TRAP_OLD_MF, asm_exc_coproc_segment_overrun),
    intg(X86_TRAP_TS, asm_exc_invalid_tss),
    intg(X86_TRAP_NP, asm_exc_segment_not_present),
    intg(X86_TRAP_SS, asm_exc_stack_segment),
    intg(X86_TRAP_GP, asm_exc_general_protection),
    intg(X86_TRAP_SPURIOUS, asm_exc_spurious_interrupt_bug),
    intg(X86_TRAP_MF, asm_exc_coprocessor_error),
    intg(X86_TRAP_AC, asm_exc_alignment_check),
    intg(X86_TRAP_XF, asm_exc_simd_coprocessor_error),
    istg(X86_TRAP_DF, asm_exc_double_fault, IST_INDEX_DF),
    istg(X86_TRAP_DB, asm_exc_debug, IST_INDEX_DB),
    istg(X86_TRAP_MC, asm_exc_machine_check, IST_INDEX_MCE),
];

/// The page fault gate, which on 64-bit never uses an IST stack.
static EARLY_PF_IDTS: [IdtData; 1] = [intg(X86_TRAP_PF, asm_exc_page_fault)];

/// `idt_table`: page aligned, as Linux maps it into the CPU entry area.
#[repr(C, align(4096))]
struct IdtTable([GateStruct; IDT_ENTRIES]);

static IDT_TABLE: RacyCell<IdtTable> = RacyCell::new(IdtTable(
    [GateStruct {
        offset_low: 0,
        segment: 0,
        bits: IdtBits(0),
        offset_middle: 0,
        offset_high: 0,
        reserved: 0,
    }; IDT_ENTRIES],
));

/// `system_vectors`: vectors with dedicated handlers.
static SYSTEM_VECTORS: RacyCell<[u64; NR_VECTORS as usize / 64]> =
    RacyCell::new([0; NR_VECTORS as usize / 64]);

/// `idt_descr`.
fn idt_descr() -> DescPtr {
    DescPtr {
        size: (IDT_ENTRIES * size_of::<GateStruct>() - 1) as u16,
        address: IDT_TABLE.get() as u64,
    }
}

/// `load_current_idt()`.
pub fn load_current_idt() {
    // SAFETY: IDT_TABLE is a static table of valid (or not-present) gates.
    unsafe { load_idt(&idt_descr()) };
}

fn write_idt_entry(vector: u32, gate: GateStruct) {
    // SAFETY: boot CPU with interrupts off; the CPU reads the table only on
    // interrupt delivery, and each entry is replaced whole.
    unsafe { (*IDT_TABLE.get()).0[vector as usize] = gate };
}

/// `idt_setup_from_table()`.
fn idt_setup_from_table(t: &[IdtData], sys: bool) {
    for d in t {
        write_idt_entry(d.vector, idt_init_desc(d));
        if sys {
            // SAFETY: as for write_idt_entry.
            unsafe { (*SYSTEM_VECTORS.get())[d.vector as usize / 64] |= 1 << (d.vector % 64) };
        }
    }
}

/// `set_intr_gate()`.
fn set_intr_gate(n: u32, addr: unsafe extern "C" fn()) {
    idt_setup_from_table(&[intg(n, addr)], false);
}

/// `idt_setup_early_traps()`: the debug and breakpoint gates.
pub fn idt_setup_early_traps() {
    idt_setup_from_table(&EARLY_IDTS, true);
    load_current_idt();
}

/// `idt_setup_traps()`: the default exception gates.
pub fn idt_setup_traps() {
    idt_setup_from_table(&DEF_IDTS, true);
}

/// `idt_setup_early_pf()`: the page fault gate.
pub fn idt_setup_early_pf() {
    idt_setup_from_table(&EARLY_PF_IDTS, true);
}

fn test_system_vector(v: u32) -> bool {
    // SAFETY: as for write_idt_entry.
    unsafe { (*SYSTEM_VECTORS.get())[v as usize / 64] & (1 << (v % 64)) != 0 }
}

/// `idt_setup_apic_and_irq_gates()`: point every external vector at its
/// `irq_entries_start` stub and every free system vector at its spurious
/// stub.
pub fn idt_setup_apic_and_irq_gates() {
    let stub = |base: unsafe extern "C" fn(), index: u32| {
        let addr = base as usize + IDT_ALIGN * index as usize;
        // SAFETY: base is a table of IDT_ALIGN-byte entry stubs in
        // entry_64, so addr is the start of one of them.
        unsafe { core::mem::transmute::<usize, unsafe extern "C" fn()>(addr) }
    };
    for i in FIRST_EXTERNAL_VECTOR..FIRST_SYSTEM_VECTOR {
        if !test_system_vector(i) {
            set_intr_gate(i, stub(irq_entries_start, i - FIRST_EXTERNAL_VECTOR));
        }
    }
    for i in FIRST_SYSTEM_VECTOR..NR_VECTORS {
        // Unassigned system vectors stay out of system_vectors so they don't
        // show up as handled.
        if !test_system_vector(i) {
            set_intr_gate(i, stub(spurious_entries_start, i - FIRST_SYSTEM_VECTOR));
        }
    }
    load_current_idt();
}
