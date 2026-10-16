/* SPDX-License-Identifier: GPL-2.0 */
/*
 * Linker script for the rinux kernel image.
 *
 * Loosely follows arch/x86/kernel/vmlinux.lds.S, but the image is loaded
 * by GRUB's multiboot2 loader at its physical link address (1 MiB) and
 * runs identity-mapped. Linux runs at 0xffffffff81000000; rinux will move
 * there when memory management is ported.
 */

ENTRY(startup_32)

SECTIONS
{
	. = 1M;
	_text = .;

	/* The multiboot2 header must be in the first 32 KiB of the file. */
	.multiboot2 : {
		KEEP(*(.multiboot2))
	}

	.text : ALIGN(4K) {
		_stext = .;
		KEEP(*(.head.text))
		*(.text .text.*)
		_etext = .;
	}

	.rodata : ALIGN(4K) {
		*(.rodata .rodata.*)
	}

	/* early_param() and __setup() entries, as in include/linux/init.h. */
	.init.setup : ALIGN(8) {
		__setup_start = .;
		KEEP(*(.init.setup))
		__setup_end = .;
	}

	.data : ALIGN(4K) {
		_sdata = .;
		*(.data .data.*)
		_edata = .;
	}

	.bss : ALIGN(4K) {
		__bss_start = .;
		*(.bss .bss.*)
		*(COMMON)
		__bss_stop = .;
	}

	_end = .;

	/DISCARD/ : {
		*(.eh_frame .eh_frame_hdr)
		*(.comment)
		*(.note .note.*)
	}
}
