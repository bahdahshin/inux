/* SPDX-License-Identifier: GPL-2.0 */
/*
 * rinux user-space init.
 *
 * The first program rinux runs in ring 3. It is a static, non-PIE ELF
 * linked at USER_LOAD_BASE (0x100000000000), so the ported ELF loader can
 * place it without a high-half kernel or relocation. It writes a line to
 * stdout through sys_write, asks for its pid through sys_getpid, and then
 * exits with a known status through sys_exit -- enough to prove the
 * syscall path, the ELF loader, and ring-3 entry all work.
 *
 * x86-64 Linux syscall ABI: number in %rax, args in %rdi, %rsi, %rdx,
 * %r10, %r8, %r9; the `syscall` instruction; result in %rax.
 */

	.text
	.globl _start
_start:
	/* write(1, msg, msglen) */
	movq	$1, %rax		/* __NR_write */
	movq	$1, %rdi		/* fd = stdout */
	leaq	msg(%rip), %rsi		/* buf */
	movq	$msglen, %rdx		/* count */
	syscall

	/* getpid(): exercise a second syscall; result ignored here */
	movq	$39, %rax		/* __NR_getpid */
	syscall

	/* exit(42) */
	movq	$60, %rax		/* __NR_exit */
	movq	$42, %rdi		/* status */
	syscall

	/* Not reached: if exit ever returns, fault loudly. */
	ud2

	.section .rodata
msg:
	.ascii	"init: hello from ring 3 (rinux user space)\n"
	.set	msglen, . - msg
