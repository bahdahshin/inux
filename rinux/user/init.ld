/* SPDX-License-Identifier: GPL-2.0 */
/*
 * Linker script for the rinux user-space init program.
 *
 * Lays everything out in a single PT_LOAD segment at USER_LOAD_BASE
 * (0x100000000000), which is what the ported ELF loader expects: one
 * read/write/execute segment, non-PIE, ET_EXEC. The ELF and program
 * headers are placed in the segment so the loader maps them harmlessly
 * along with the code; the entry point is _start.
 */

ENTRY(_start)

PHDRS
{
	load PT_LOAD FLAGS(7);	/* R | W | X */
}

SECTIONS
{
	. = 0x100000000000;
	.all : {
		*(.text*)
		*(.rodata*)
		*(.data*)
		*(.bss*)
	} :load

	/DISCARD/ : {
		*(.note*)
		*(.comment*)
		*(.eh_frame*)
		*(.eh_frame_hdr*)
	}
}
