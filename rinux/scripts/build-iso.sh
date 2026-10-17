#!/bin/bash
# SPDX-License-Identifier: GPL-2.0
#
# Build rinux-boot.iso: the rinux kernel and a GRUB that boots it with
# multiboot2, under both BIOS and UEFI. The ISO lands next to
# linux-boot.iso at the repository root.
#
# Run it in WSL (or any Linux) from the repository root:
#
#   bash rinux/scripts/build-iso.sh            # release kernel
#   bash rinux/scripts/build-iso.sh --debug    # debug kernel
#   CMDLINE="earlyprintk=ttyS0 loglevel=6" bash rinux/scripts/build-iso.sh
#
# Needs cargo with the x86_64-unknown-none target, grub-mkrescue
# (grub-pc-bin and grub-efi-amd64-bin), xorriso and mtools.

set -euo pipefail

RINUX=$(cd "$(dirname "$0")/.." && pwd)
PROFILE=release
if [ "${1:-}" = "--debug" ]; then
	PROFILE=debug
fi

CARGO=${CARGO:-$(command -v cargo || echo "$HOME/.cargo/bin/cargo")}
# Keep build output on the Linux filesystem; building on /mnt/c is slow.
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/.cache/rinux-kernel-target}
OUT=${OUT:-$RINUX/../rinux-boot.iso}
# earlyprintk=serial,ttyS0,115200,vga brings up both early consoles, as it
# would for Linux.
CMDLINE=${CMDLINE:-earlyprintk=serial,ttyS0,115200,vga}

missing=()
for tool in grub-mkrescue xorriso mformat; do
	command -v "$tool" >/dev/null || missing+=("$tool")
done
if [ ${#missing[@]} -ne 0 ]; then
	echo "missing: ${missing[*]}" >&2
	echo "install: sudo apt install grub-pc-bin grub-efi-amd64-bin xorriso mtools" >&2
	exit 1
fi

if [ "$PROFILE" = release ]; then
	(cd "$RINUX/kernel" && "$CARGO" build --release)
else
	(cd "$RINUX/kernel" && "$CARGO" build)
fi
KERNEL=$CARGO_TARGET_DIR/x86_64-unknown-none/$PROFILE/rinux

ISO_DIR=$(mktemp -d)
trap 'rm -rf "$ISO_DIR"' EXIT
mkdir -p "$ISO_DIR/boot/grub"
cp "$KERNEL" "$ISO_DIR/boot/rinux"

# Build a small newc cpio initramfs, loaded as a GRUB module and unpacked
# into the rootfs at boot. It carries /init: a static ring-3 program the
# kernel execs as its user-space check (the step-8 milestone).
INITRAMFS=${INITRAMFS:-}
if [ -z "$INITRAMFS" ]; then
	CPIO_DIR=$(mktemp -d)
	mkdir -p "$CPIO_DIR/etc" "$CPIO_DIR/bin"
	echo "Welcome to rinux." > "$CPIO_DIR/etc/motd"
	echo "root:x:0:0:root:/root:/bin/sh" > "$CPIO_DIR/etc/passwd"
	echo "rinux initramfs, unpacked by the ported cpio reader." > "$CPIO_DIR/README"

	# Compile and link the user-space init program, linked at USER_LOAD_BASE
	# as a single PT_LOAD segment (see user/init.ld). Freestanding C, no libc.
	CC=${CC:-$(command -v x86_64-linux-gnu-gcc || command -v gcc)}
	LD=${LD:-$(command -v x86_64-linux-gnu-ld || command -v ld)}
	OBJ=$(mktemp -d)
	# -mgeneral-regs-only: ring-3 init runs with no FPU/SSE state set up (the
	# kernel is built soft-float), so the compiler must not auto-vectorise with
	# SSE/MMX/x87 -- an XMM instruction would fault in user space.
	# init is a static image linked at USER_LOAD_BASE, 16 TiB up, so its
	# references must be 64-bit absolute: -fno-pic with the large code model.
	# Distros that default to PIE would otherwise emit GOT-relative addresses
	# that this static link cannot resolve.
	"$CC" -c -nostdlib -ffreestanding -fno-builtin -fno-stack-protector \
		-fno-pic -fno-pie -mcmodel=large -mgeneral-regs-only -O2 \
		-o "$OBJ/init.o" "$RINUX/user/init.c"
	"$LD" -nostdlib --build-id=none -T "$RINUX/user/init.ld" \
		-o "$CPIO_DIR/init" "$OBJ/init.o"
	chmod +x "$CPIO_DIR/init"
	rm -rf "$OBJ"

	( cd "$CPIO_DIR" && find . -mindepth 1 | sed 's|^\./||' | sort |
		cpio --quiet -o -H newc ) > "$ISO_DIR/boot/initramfs.cpio"
	rm -rf "$CPIO_DIR"
	INITRAMFS="$ISO_DIR/boot/initramfs.cpio"
else
	cp "$INITRAMFS" "$ISO_DIR/boot/initramfs.cpio"
fi

cat > "$ISO_DIR/boot/grub/grub.cfg" <<EOF
set timeout=0
set default=0

serial --unit=0 --speed=115200
terminal_input serial console
terminal_output serial console

menuentry "rinux" {
	multiboot2 /boot/rinux $CMDLINE
	module2 /boot/initramfs.cpio initramfs
	boot
}
EOF

grub-mkrescue -o "$OUT" "$ISO_DIR"
echo "Wrote $OUT"
