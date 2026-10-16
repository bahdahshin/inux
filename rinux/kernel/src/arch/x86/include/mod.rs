// SPDX-License-Identifier: GPL-2.0
//! `arch/x86/include`.

pub mod desc;
pub mod i8259;
pub mod io;
pub mod irqflags;
pub mod kdebug;
pub mod page;
pub mod pgtable;
pub mod processor;
pub mod ptrace;

#[allow(dead_code, clippy::all)]
#[rustfmt::skip]
pub mod irq_vectors;

#[allow(dead_code, non_upper_case_globals, clippy::all)]
#[rustfmt::skip]
pub mod msr_index;

#[allow(dead_code, clippy::all)]
#[rustfmt::skip]
pub mod processor_flags;

#[allow(dead_code, clippy::all)]
#[rustfmt::skip]
pub mod segment;

#[allow(dead_code, clippy::all)]
#[rustfmt::skip]
pub mod trapnr;
