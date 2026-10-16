/* SPDX-License-Identifier: GPL-2.0 */
/*
 * rinux user-space init.
 *
 * The first program rinux runs in ring 3. It is a static, freestanding,
 * non-PIE ELF linked at USER_LOAD_BASE (0x100000000000). It greets the
 * console with write(2), reads the system identity with uname(2) (proving a
 * syscall that writes into user memory works), asks for its pid, and exits
 * with a known status -- enough to exercise the syscall path, the ELF
 * loader, and ring-3 entry.
 *
 * Built with -nostdlib -ffreestanding, so there is no libc: the syscalls are
 * issued directly and _start is the entry point.
 */

/* x86-64 Linux syscall numbers. */
#define SYS_read 0
#define SYS_write 1
#define SYS_open 2
#define SYS_stat 4
#define SYS_lseek 8
#define SYS_getdents64 217
#define SYS_close 3
#define O_DIRECTORY 0x10000
#define SYS_getpid 39
#define SYS_uname 63
#define SYS_nanosleep 35
#define SYS_gettimeofday 96
#define SYS_rt_sigaction 13
#define SYS_rt_sigprocmask 14
#define SYS_kill 62
#define SYS_getcwd 79
#define SYS_sysinfo 99
#define SYS_arch_prctl 158
#define ARCH_SET_FS 0x1002
#define ARCH_GET_FS 0x1003
#define SYS_getppid 110
#define SYS_set_tid_address 218
#define SYS_set_robust_list 273
#define SYS_prlimit64 302
#define RLIMIT_STACK 3
#define RLIMIT_NOFILE 7
#define SYS_futex 202
#define FUTEX_WAIT 0
#define FUTEX_WAKE 1
#define SYS_mmap 9
#define SYS_mprotect 10
#define SYS_munmap 11
#define PROT_READ 1
#define PROT_WRITE 2
#define MAP_PRIVATE 2
#define MAP_ANONYMOUS 0x20
#define SYS_getuid 102
#define SYS_getgid 104
#define SYS_capget 125
#define SYS_getrandom 318
#define SYS_clock_gettime 228
#define CLOCK_REALTIME 0
#define SYS_exit 60

static long syscall3(long nr, long a1, long a2, long a3)
{
	long ret;
	__asm__ volatile("syscall"
			 : "=a"(ret)
			 : "a"(nr), "D"(a1), "S"(a2), "d"(a3)
			 : "rcx", "r11", "memory");
	return ret;
}

static long syscall4(long nr, long a1, long a2, long a3, long a4)
{
	long ret;
	register long r10 __asm__("r10") = a4;
	__asm__ volatile("syscall"
			 : "=a"(ret)
			 : "a"(nr), "D"(a1), "S"(a2), "d"(a3), "r"(r10)
			 : "rcx", "r11", "memory");
	return ret;
}

static long syscall6(long nr, long a1, long a2, long a3, long a4, long a5, long a6)
{
	long ret;
	register long r10 __asm__("r10") = a4;
	register long r8 __asm__("r8") = a5;
	register long r9 __asm__("r9") = a6;
	__asm__ volatile("syscall"
			 : "=a"(ret)
			 : "a"(nr), "D"(a1), "S"(a2), "d"(a3), "r"(r10), "r"(r8),
			   "r"(r9)
			 : "rcx", "r11", "memory");
	return ret;
}

/*
 * The signal return trampoline, in asm so it has no prologue that would
 * disturb the signal frame: it just invokes rt_sigreturn (15).
 */
extern void sig_restorer(void);
__asm__(".globl sig_restorer\n"
	"sig_restorer:\n"
	"	movl $15, %eax\n"
	"	syscall\n");

static long my_strlen(const char *s)
{
	long n = 0;
	while (s[n])
		n++;
	return n;
}

static void put(const char *s)
{
	syscall3(SYS_write, 1, (long)s, my_strlen(s));
}

/* Writes an unsigned value as decimal. */
static void putu(unsigned long v)
{
	char b[20];
	int i = 20;
	b[--i] = '\n';
	if (v == 0)
		b[--i] = '0';
	while (v) {
		b[--i] = '0' + (v % 10);
		v /= 10;
	}
	syscall3(SYS_write, 1, (long)&b[i], 20 - i);
}

/* An SA_SIGINFO handler: runs on SIGUSR1 delivery with the siginfo. */
static void sig_handler(int sig, void *info, void *uc)
{
	(void)sig;
	(void)uc;
	int signo = *(int *)info; /* siginfo.si_signo at offset 0 */
	put("init: SIGUSR1 handler ran, si_signo ");
	putu((unsigned long)signo);
}

/* struct new_utsname: six NUL-terminated fields of 65 bytes each. */
#define UTS_LEN 65

void _start(void)
{
	put("init: hello from ring 3 (rinux user space)\n");

	char uts[6 * UTS_LEN];
	syscall3(SYS_uname, (long)uts, 0, 0);
	put("init: uname ");
	put(&uts[0 * UTS_LEN]); /* sysname */
	put(" ");
	put(&uts[2 * UTS_LEN]); /* release */
	put(" ");
	put(&uts[4 * UTS_LEN]); /* machine */
	put("\n");

	/* stat a file and report its size. */
	char st[144];
	if (syscall3(SYS_stat, (long)"/etc/motd", (long)st, 0) == 0) {
		unsigned long size = *(unsigned long *)(st + 48); /* st_size */
		put("init: stat /etc/motd size ");
		putu(size);
	}

	/* Open a file, seek past the first 8 bytes, and read the rest. */
	long fd = syscall3(SYS_open, (long)"/etc/motd", 0 /* O_RDONLY */, 0);
	if (fd >= 0) {
		syscall3(SYS_lseek, fd, 8, 0 /* SEEK_SET */);
		char buf[128];
		long n = syscall3(SYS_read, fd, (long)buf, sizeof(buf));
		if (n > 0) {
			put("init: /etc/motd[8..]: ");
			syscall3(SYS_write, 1, (long)buf, n);
		}
		syscall3(SYS_close, fd, 0, 0);
	}

	/* List the root directory with getdents64. */
	long dfd = syscall3(SYS_open, (long)"/", O_DIRECTORY, 0);
	if (dfd >= 0) {
		char dbuf[1024];
		put("init: ls /:");
		long n;
		while ((n = syscall3(SYS_getdents64, dfd, (long)dbuf, sizeof(dbuf))) > 0) {
			long off = 0;
			while (off < n) {
				/* struct linux_dirent64: name at offset 19. */
				unsigned short reclen =
					*(unsigned short *)(dbuf + off + 16);
				put(" ");
				put(dbuf + off + 19);
				off += reclen;
			}
		}
		put("\n");
		syscall3(SYS_close, dfd, 0, 0);
	}

	/* Read the wall clock. struct timespec: tv_sec at 0, tv_nsec at 8. */
	long ts[2];
	if (syscall3(SYS_clock_gettime, CLOCK_REALTIME, (long)ts, 0) == 0) {
		put("init: clock_gettime REALTIME ");
		putu((unsigned long)ts[0]);
	}

	/*
	 * Sleep ~150 ms and measure the elapsed time with gettimeofday
	 * (struct timeval: tv_sec at 0, tv_usec at 8).
	 */
	long tv0[2], tv1[2];
	long req[2] = { 0, 150000000L }; /* 0 s, 150 ms */
	syscall3(SYS_gettimeofday, (long)tv0, 0, 0);
	syscall3(SYS_nanosleep, (long)req, 0, 0);
	syscall3(SYS_gettimeofday, (long)tv1, 0, 0);
	long elapsed_us = (tv1[0] - tv0[0]) * 1000000L + (tv1[1] - tv0[1]);
	put("init: slept, elapsed_us ");
	putu((unsigned long)elapsed_us);

	/* Ask the kernel for random bytes and report how many and their XOR. */
	unsigned char rnd[16];
	long got = syscall3(SYS_getrandom, (long)rnd, sizeof(rnd), 0);
	if (got > 0) {
		unsigned x = 0;
		for (long i = 0; i < got; i++)
			x ^= rnd[i];
		put("init: getrandom ");
		putu((unsigned long)got);
		put("init: getrandom xor ");
		putu(x);
	}

	/*
	 * Query system statistics. struct sysinfo: uptime at 0, totalram at 32,
	 * freeram at 40, mem_unit (u32) at 104; totalram/freeram are in pages of
	 * mem_unit bytes.
	 */
	unsigned char si[112];
	if (syscall3(SYS_sysinfo, (long)si, 0, 0) == 0) {
		unsigned long uptime = *(unsigned long *)(si + 0);
		unsigned long totalram = *(unsigned long *)(si + 32);
		unsigned long freeram = *(unsigned long *)(si + 40);
		unsigned long unit = *(unsigned *)(si + 104);
		put("init: sysinfo uptime_s ");
		putu(uptime);
		put("init: sysinfo totalram_mb ");
		putu(totalram * unit / (1024 * 1024));
		put("init: sysinfo freeram_mb ");
		putu(freeram * unit / (1024 * 1024));
	}

	/*
	 * Query our capabilities. Header: version (u32) at 0, pid (int) at 4.
	 * Data: two records of {effective, permitted, inheritable} (u32 each),
	 * the low and high halves of the 64-bit sets.
	 */
	unsigned hdr[2] = { 0x20080522u, 0 }; /* _LINUX_CAPABILITY_VERSION_3 */
	unsigned data[6];
	if (syscall3(SYS_capget, (long)hdr, (long)data, 0) == 0) {
		put("init: capget effective_lo ");
		putu(data[0]);
		put("init: capget effective_hi ");
		putu(data[3]);
	}

	/*
	 * Install a SIGUSR1 handler and inspect the blocked-signal mask.
	 * struct sigaction: handler, flags, restorer, mask (u64 each).
	 */
	/* flags: SA_RESTORER (0x04000000) | SA_SIGINFO (0x4). */
	unsigned long act[4] = { (unsigned long)sig_handler, 0x04000004UL,
				 (unsigned long)sig_restorer, 0 };
	unsigned long old[4];
	if (syscall4(SYS_rt_sigaction, 10 /* SIGUSR1 */, (long)act, (long)old, 8) == 0) {
		put(old[0] == 0 ? "init: rt_sigaction old SIG_DFL\n"
				: "init: rt_sigaction old was set\n");
		unsigned long rb[4];
		if (syscall4(SYS_rt_sigaction, 10, 0, (long)rb, 8) == 0)
			put(rb[0] == (unsigned long)sig_handler
				    ? "init: rt_sigaction readback ok\n"
				    : "init: rt_sigaction readback bad\n");
	}
	unsigned long set = 1UL << (2 - 1); /* SIGINT */
	unsigned long cur = 0;
	syscall4(SYS_rt_sigprocmask, 0 /* SIG_BLOCK */, (long)&set, 0, 8);
	syscall4(SYS_rt_sigprocmask, 0, 0, (long)&cur, 8); /* query current mask */
	put((cur & set) ? "init: sigprocmask blocks SIGINT\n"
			: "init: sigprocmask wrong\n");

	/*
	 * Raise SIGUSR1 at ourselves: the kernel should run sig_handler on the
	 * way back from kill(), then rt_sigreturn should resume us right here.
	 */
	long mypid = syscall3(SYS_getpid, 0, 0, 0);
	syscall3(SYS_kill, mypid, 10 /* SIGUSR1 */, 0);
	put("init: resumed after signal\n");

	/* The syscalls a libc makes at startup. */
	put("init: ppid ");
	putu((unsigned long)syscall3(SYS_getppid, 0, 0, 0));
	int tid_slot = 0;
	long tid = syscall3(SYS_set_tid_address, (long)&tid_slot, 0, 0);
	tid_slot = 42; /* the kernel zeroes this when we exit */
	put(tid == syscall3(SYS_getpid, 0, 0, 0)
		    ? "init: set_tid_address returns our pid\n"
		    : "init: set_tid_address wrong\n");
	unsigned long robust[3] = { 0, 0, 0 };
	robust[0] = (unsigned long)robust; /* an empty list points at itself */
	long r1 = syscall3(SYS_set_robust_list, (long)robust, 24, 0);
	long r2 = syscall3(SYS_set_robust_list, (long)robust, 8, 0);
	put((r1 == 0 && r2 == -22) ? "init: set_robust_list ok, rejects bad len\n"
				   : "init: set_robust_list wrong\n");
	unsigned long rl[2] = { 0, 0 };
	syscall4(SYS_prlimit64, 0, RLIMIT_STACK, 0, (long)rl);
	put("init: rlimit STACK cur_mb ");
	putu(rl[0] / (1024 * 1024));
	syscall4(SYS_prlimit64, 0, RLIMIT_NOFILE, 0, (long)rl);
	put("init: rlimit NOFILE cur ");
	putu(rl[0]);
	unsigned long nl[2] = { 512, 4096 };
	syscall4(SYS_prlimit64, 0, RLIMIT_NOFILE, (long)nl, 0);
	syscall4(SYS_prlimit64, 0, RLIMIT_NOFILE, 0, (long)rl);
	put(rl[0] == 512 ? "init: rlimit NOFILE set to 512 ok\n"
			 : "init: rlimit set wrong\n");
	unsigned long bad[2] = { 9000, 4096 }; /* soft above hard */
	put(syscall4(SYS_prlimit64, 0, RLIMIT_NOFILE, (long)bad, 0) == -22
		    ? "init: rlimit rejects cur>max\n"
		    : "init: rlimit accepted bad\n");
	/* The limit is enforced: with NOFILE 4, fd 3 opens but fd 4 is refused. */
	unsigned long tight[2] = { 4, 4096 };
	syscall4(SYS_prlimit64, 0, RLIMIT_NOFILE, (long)tight, 0);
	long fd1 = syscall3(SYS_open, (long)"/etc/motd", 0, 0);
	long fd2 = syscall3(SYS_open, (long)"/etc/motd", 0, 0);
	put((fd1 == 3 && fd2 == -24)
		    ? "init: rlimit NOFILE enforced (fd 3, then EMFILE)\n"
		    : "init: rlimit NOFILE not enforced\n");
	syscall3(SYS_close, fd1, 0, 0);
	unsigned long loose[2] = { 1024, 4096 };
	syscall4(SYS_prlimit64, 0, RLIMIT_NOFILE, (long)loose, 0);

	/*
	 * Futexes. A WAIT whose expected value no longer matches fails at once
	 * with EAGAIN; one that matches sleeps until woken or timed out -- and
	 * with no other thread to wake us, the 50 ms timeout is what returns.
	 */
	unsigned int fword = 1;
	long fe = syscall4(SYS_futex, (long)&fword, FUTEX_WAIT, 0, 0);
	put(fe == -11 ? "init: futex WAIT on a changed word -> EAGAIN\n"
		      : "init: futex WAIT mismatch wrong\n");
	long ft0[2], ft1[2];
	long fto[2] = { 0, 50000000L }; /* 50 ms */
	syscall3(SYS_gettimeofday, (long)ft0, 0, 0);
	long fw = syscall4(SYS_futex, (long)&fword, FUTEX_WAIT, 1, (long)fto);
	syscall3(SYS_gettimeofday, (long)ft1, 0, 0);
	long fus = (ft1[0] - ft0[0]) * 1000000L + (ft1[1] - ft0[1]);
	put(fw == -110 && fus >= 50000 ? "init: futex WAIT timed out, elapsed_us "
					: "init: futex WAIT timeout wrong, elapsed_us ");
	putu((unsigned long)fus);
	long fn = syscall3(SYS_futex, (long)&fword, FUTEX_WAKE, 1);
	put(fn == 0 ? "init: futex WAKE woke 0 (no waiters)\n"
		    : "init: futex WAKE wrong\n");

	/*
	 * Anonymous memory: two zeroed, writable pages placed by the kernel,
	 * their protection changed in place, then unmapped.
	 */
	long m = syscall6(SYS_mmap, 0, 8192, PROT_READ | PROT_WRITE,
			  MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	put(m > 0 && (m & 4095) == 0 ? "init: mmap 2 anonymous pages, page aligned\n"
				     : "init: mmap failed\n");
	unsigned long *mp = (unsigned long *)m;
	mp[0] = 0x1234;
	mp[1023] = 0x5678; /* last word of the second page */
	put(mp[0] == 0x1234 && mp[1023] == 0x5678 && mp[512] == 0
		    ? "init: mmap pages writable and zero-filled\n"
		    : "init: mmap pages wrong\n");
	long pr1 = syscall3(SYS_mprotect, m, 8192, PROT_READ);
	long pr2 = syscall3(SYS_mprotect, m, 8192, PROT_READ | PROT_WRITE);
	mp[1] = 7;
	put(pr1 == 0 && pr2 == 0 && mp[1] == 7 ? "init: mprotect ok\n"
					       : "init: mprotect wrong\n");
	long mbad = syscall6(SYS_mmap, 0, 4096, PROT_READ, MAP_PRIVATE, -1, 0);
	put(mbad == -9 ? "init: mmap without MAP_ANONYMOUS -> EBADF\n"
		       : "init: mmap bad flags wrong\n");
	long um = syscall3(SYS_munmap, m, 8192, 0);
	long um_bad = syscall3(SYS_munmap, m + 1, 4096, 0);
	long pr_gone = syscall3(SYS_mprotect, m, 4096, PROT_READ);
	put(um == 0 && um_bad == -22 && pr_gone == -12
		    ? "init: munmap ok, rejects unaligned, mprotect of a hole -> ENOMEM\n"
		    : "init: munmap wrong\n");

	/*
	 * Set the FS base to our own array and read it back through the %fs
	 * segment -- this is how user space implements thread-local storage.
	 */
	unsigned long tls[2] = { 0xdeadbeefUL, 0 };
	syscall3(SYS_arch_prctl, ARCH_SET_FS, (long)tls, 0);
	unsigned long via_fs;
	__asm__ volatile("movq %%fs:0, %0" : "=r"(via_fs));
	put(via_fs == 0xdeadbeefUL ? "init: arch_prctl FS/TLS ok\n"
				   : "init: arch_prctl FS/TLS fail\n");
	unsigned long base = 0;
	syscall3(SYS_arch_prctl, ARCH_GET_FS, (long)&base, 0);
	put(base == (unsigned long)tls ? "init: arch_prctl GET_FS ok\n"
				       : "init: arch_prctl GET_FS fail\n");

	/* Report the current working directory. */
	char cwd[64];
	if (syscall3(SYS_getcwd, (long)cwd, sizeof(cwd), 0) > 0) {
		put("init: cwd ");
		put(cwd);
		put("\n");
	}

	/* Report our identity. */
	put("init: uid ");
	putu((unsigned long)syscall3(SYS_getuid, 0, 0, 0));
	put("init: gid ");
	putu((unsigned long)syscall3(SYS_getgid, 0, 0, 0));

	syscall3(SYS_getpid, 0, 0, 0);
	syscall3(SYS_exit, 42, 0, 0);
	for (;;)
		;
}
