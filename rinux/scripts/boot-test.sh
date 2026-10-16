#!/bin/bash
# SPDX-License-Identifier: GPL-2.0
#
# Boot rinux-boot.iso in QEMU without a display and check the serial log.
#
#   bash rinux/scripts/boot-test.sh            # BIOS
#   bash rinux/scripts/boot-test.sh --uefi     # UEFI (needs OVMF)
#
# Passes when the kernel prints EXPECT (default: the line start_kernel()
# prints at the end of the ported boot path) within TIMEOUT seconds. The
# serial log and a screenshot of the VGA screen are left in OUT_DIR.

set -euo pipefail

RINUX=$(cd "$(dirname "$0")/.." && pwd)
ISO=${ISO:-$RINUX/../rinux-boot.iso}
TIMEOUT=${TIMEOUT:-30}
EXPECT=${EXPECT:-"reached the end of the ported boot path"}
OUT_DIR=${OUT_DIR:-$HOME/.cache/rinux-boot-test}
MODE=bios
FIRMWARE=()
if [ "${1:-}" = "--uefi" ]; then
	MODE=uefi
	OVMF=${OVMF:-/usr/share/ovmf/OVMF.fd}
	FIRMWARE=(-bios "$OVMF")
fi

mkdir -p "$OUT_DIR"
LOG=$OUT_DIR/serial-$MODE.log
SHOT=$OUT_DIR/vga-$MODE.ppm
PORT=$((4400 + RANDOM % 500))
# A second monitor, on its own port, is used only to inject a keypress for the
# keyboard self-test, so it never contends with the screendump on $PORT.
KPORT=$((PORT + 1000))
rm -f "$LOG" "$SHOT"

# A small raw disk with a known signature in sector 0, for the virtio-blk
# block-device self-test to read back.
DISK=$OUT_DIR/disk-$MODE.img
printf 'RINUX-DISK: hello from virtio-blk sector 0\n' > "$DISK"
truncate -s 1M "$DISK"

# A second disk holding an ext2 image with a couple of files, for the ext2
# self-test to mount and read. Built with mke2fs if available.
EXT2=$OUT_DIR/ext2-$MODE.img
EXT2_ARGS=()
if command -v mke2fs >/dev/null; then
	MROOT=$(mktemp -d)
	printf 'hello from an ext2 file on a virtio-blk disk, read by rinux\n' > "$MROOT/hello.txt"
	printf 'rinux ext2 reader: superblock, inodes, directories, data blocks\n' > "$MROOT/README"
	mkdir -p "$MROOT/dir"
	printf 'nested\n' > "$MROOT/dir/nested.txt"
	rm -f "$EXT2"
	truncate -s 4M "$EXT2"
	mke2fs -q -F -t ext2 -b 1024 -d "$MROOT" "$EXT2" >/dev/null 2>&1
	rm -rf "$MROOT"
	EXT2_ARGS=(-drive "file=$EXT2,if=none,format=raw,id=ext2disk" -device virtio-blk-pci,drive=ext2disk)
else
	echo "note: mke2fs not found; ext2 self-test will be skipped" >&2
fi

# A third disk holding an ext4 image (extent-mapped) with /ext4.txt, for the
# ext4 self-test. Attached after the ext2 disk so the ext2 test still picks
# the ext2 image first.
EXT4=$OUT_DIR/ext4-$MODE.img
EXT4_ARGS=()
if command -v mkfs.ext4 >/dev/null; then
	E4ROOT=$(mktemp -d)
	printf 'this file lives on an ext4 image, read through the extent tree\n' > "$E4ROOT/ext4.txt"
	printf 'rinux ext4 reader: superblock, inodes, extents, directories\n' > "$E4ROOT/README"
	rm -f "$EXT4"
	truncate -s 8M "$EXT4"
	# Default mkfs.ext4 uses extents; disable features rinux does not read.
	mkfs.ext4 -q -F -b 1024 -O ^64bit,^metadata_csum,^has_journal -d "$E4ROOT" "$EXT4" >/dev/null 2>&1
	rm -rf "$E4ROOT"
	EXT4_ARGS=(-drive "file=$EXT4,if=none,format=raw,id=ext4disk" -device virtio-blk-pci,drive=ext4disk)
else
	echo "note: mkfs.ext4 not found; ext4 self-test will be skipped" >&2
fi

# A fourth disk holding a FAT32 image with /fat.txt, built with mtools, for
# the FAT self-test. Attached after the ext disks so they still match first.
FAT=$OUT_DIR/fat-$MODE.img
FAT_ARGS=()
if command -v mformat >/dev/null && command -v mcopy >/dev/null; then
	rm -f "$FAT"
	truncate -s 48M "$FAT"
	mformat -i "$FAT" -F :: >/dev/null 2>&1
	TMPF=$(mktemp)
	printf 'this file lives on a FAT32 image, read by rinux through the cluster chain\n' > "$TMPF"
	mcopy -i "$FAT" "$TMPF" ::fat.txt >/dev/null 2>&1
	rm -f "$TMPF"
	FAT_ARGS=(-drive "file=$FAT,if=none,format=raw,id=fatdisk" -device virtio-blk-pci,drive=fatdisk)
else
	echo "note: mtools (mformat/mcopy) not found; FAT self-test will be skipped" >&2
fi

# A disk with a hand-crafted MBR partition table, for the partition self-test.
PART=$OUT_DIR/part-$MODE.img
PART_ARGS=()
if command -v python3 >/dev/null; then
	python3 - "$PART" <<'PY'
import struct, sys
disk = bytearray(4 * 1024 * 1024)
def ent(boot, ptype, start, size):
    return bytes([boot, 0, 0, 0, ptype, 0, 0, 0]) + struct.pack('<II', start, size)
disk[0x1be:0x1be+16] = ent(0x80, 0x83, 2048, 1024)   # bootable Linux
disk[0x1be+16:0x1be+32] = ent(0x00, 0x83, 4096, 2048) # Linux
disk[510] = 0x55
disk[511] = 0xaa
open(sys.argv[1], 'wb').write(disk)
PY
	PART_ARGS=(-drive "file=$PART,if=none,format=raw,id=partdisk" -device virtio-blk-pci,drive=partdisk)
fi

# A disk with a GUID Partition Table, for the GPT self-test (CRC-valid header
# and entry array, two Linux partitions).
GPT=$OUT_DIR/gpt-$MODE.img
GPT_ARGS=()
if command -v python3 >/dev/null; then
	python3 - "$GPT" <<'PY'
import struct, zlib, uuid, sys
SEC = 512
nsec = 8192  # 4 MiB
LINUX = uuid.UUID("0FC63DAF-8483-4772-8E79-3D69D8477DE4").bytes_le
def entry(first, last, name):
    e = bytearray(128)
    e[0:16] = LINUX
    e[16:32] = uuid.uuid4().bytes_le
    struct.pack_into('<Q', e, 32, first)
    struct.pack_into('<Q', e, 40, last)
    nm = name.encode('utf-16-le')
    e[56:56+len(nm)] = nm
    return e
entries = bytearray(128 * 128)  # 128 entries
entries[0:128] = entry(34, 2047, "rootfs")
entries[128:256] = entry(2048, 4095, "data")
entry_crc = zlib.crc32(bytes(entries)) & 0xffffffff
hdr = bytearray(92)
hdr[0:8] = b"EFI PART"
struct.pack_into('<I', hdr, 8, 0x00010000)
struct.pack_into('<I', hdr, 12, 92)
struct.pack_into('<Q', hdr, 24, 1)
struct.pack_into('<Q', hdr, 32, nsec - 1)
struct.pack_into('<Q', hdr, 40, 34)
struct.pack_into('<Q', hdr, 48, nsec - 34)
hdr[56:72] = uuid.uuid4().bytes_le
struct.pack_into('<Q', hdr, 72, 2)
struct.pack_into('<I', hdr, 80, 128)
struct.pack_into('<I', hdr, 84, 128)
struct.pack_into('<I', hdr, 88, entry_crc)
struct.pack_into('<I', hdr, 16, zlib.crc32(bytes(hdr)) & 0xffffffff)
disk = bytearray(SEC * nsec)
disk[SEC:SEC+92] = hdr            # LBA 1: GPT header
disk[2*SEC:2*SEC+len(entries)] = entries  # LBA 2+: entry array
open(sys.argv[1], 'wb').write(disk)
PY
	GPT_ARGS=(-drive "file=$GPT,if=none,format=raw,id=gptdisk" -device virtio-blk-pci,drive=gptdisk)
fi

# A virtio-console device whose port output the host routes to a file, for the
# virtio-console self-test. The guest sends a marker line to the port (we grep
# the output file for it) and reads a marker the host feeds in through the
# chardev's input-path (the guest prints it on the serial log). virtio-serial-
# pci is transitional on i440fx, so it exposes the legacy I/O BAR the ported
# transport uses.
VCON=$OUT_DIR/vcon-$MODE.log
VCON_IN=$OUT_DIR/vcon-in-$MODE.txt
rm -f "$VCON"
printf 'rinux-host-input-ok' > "$VCON_IN"
# max_ports=1 keeps the device to port 0's queues plus the two control queues
# (4 total), which the driver programs; the default (31 ports) would leave the
# host polling dozens of queues the driver never sets up.
VCON_ARGS=(-chardev "file,id=vcon0,path=$VCON,input-path=$VCON_IN" \
	-device virtio-serial-pci,max_ports=1 -device virtconsole,chardev=vcon0)

# A tiny host HTTP server for the TCP self-test. The guest reaches the host
# at 10.0.2.2 through QEMU user networking, so a connection to 10.0.2.2:10080
# lands here. It answers one request with a fixed response, then loops for
# the next boot's connection.
TCPSRV=
NETSRV=
if command -v python3 >/dev/null; then
	# TCP HTTP server on :10080 and a UDP echo server on :10082.
	python3 - <<'PY' &
import socket, threading
def tcp():
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("0.0.0.0", 10080))
    srv.listen(4)
    while True:
        try:
            c, _ = srv.accept()
            c.recv(1024)
            c.sendall(b"HTTP/1.0 200 OK\r\nContent-Length: 13\r\n\r\nhello, rinux\n")
            c.close()
        except Exception:
            pass
def udp():
    srv = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("0.0.0.0", 10082))
    while True:
        try:
            data, addr = srv.recvfrom(1024)
            srv.sendto(data, addr)
        except Exception:
            pass
threading.Thread(target=tcp, daemon=True).start()
udp()
PY
	NETSRV=$!
	TCPSRV=$NETSRV
else
	echo "note: python3 not found; TCP/socket self-tests will be skipped" >&2
fi

# virtio-rng and virtio-blk let the virtio and block self-tests run real
# virtqueue transactions; on the i440fx machine both are transitional
# devices, so they expose the legacy I/O BAR the ported drivers use.
qemu-system-x86_64 -m 256M -cdrom "$ISO" "${FIRMWARE[@]}" \
	-display none -vga std -no-reboot \
	-device virtio-rng-pci \
	-drive "file=$DISK,if=none,format=raw,id=rinuxdisk" \
	-device virtio-blk-pci,drive=rinuxdisk \
	"${EXT2_ARGS[@]}" \
	"${EXT4_ARGS[@]}" \
	"${FAT_ARGS[@]}" \
	"${PART_ARGS[@]}" \
	"${GPT_ARGS[@]}" \
	"${VCON_ARGS[@]}" \
	-netdev user,id=net0 \
	-device virtio-net-pci,netdev=net0 \
	-serial "file:$LOG" \
	-monitor "tcp:127.0.0.1:$PORT,server,nowait" \
	-monitor "tcp:127.0.0.1:$KPORT,server,nowait" &
QEMU=$!

# Inject a keypress for the keyboard self-test. check_keyboard() waits ~5s for
# a key, so send 'a' repeatedly over that window (through the dedicated key
# monitor) until one lands while it is waiting.
send_keys() {
	# Send 'a' continuously until the EXIT trap kills us, so a key is arriving
	# whenever the guest's check_keyboard() window opens (BIOS and the slower
	# UEFI boot reach it at different times). A fresh connection per keypress:
	# holding one open and sending repeatedly stalls the HMP monitor.
	while true; do
		if exec 3<>"/dev/tcp/127.0.0.1/$KPORT" 2>/dev/null; then
			echo "sendkey a" >&3
			exec 3>&-
		fi
		sleep 0.3
	done
}
send_keys &
KEYPID=$!
trap 'kill $QEMU 2>/dev/null || true; [ -n "$TCPSRV" ] && kill $TCPSRV 2>/dev/null || true; kill $KEYPID 2>/dev/null || true' EXIT

status=1
for _ in $(seq "$TIMEOUT"); do
	sleep 1
	if grep -qF "$EXPECT" "$LOG" 2>/dev/null; then
		status=0
		break
	fi
	kill -0 $QEMU 2>/dev/null || break
done

# Grab the screen, then stop QEMU.
if kill -0 $QEMU 2>/dev/null; then
	exec 3<>"/dev/tcp/127.0.0.1/$PORT"
	echo "screendump $SHOT" >&3
	sleep 1
	echo "quit" >&3
	exec 3>&-
	wait $QEMU 2>/dev/null || true
fi

# GRUB writes terminal escape codes to the serial port; show only text.
sed -e 's/\x1b\[[0-9;?]*[A-Za-z]//g' -e 's/\r//g' "$LOG" | grep -v '^\s*$' || true
if [ $status -eq 0 ]; then
	echo "PASS ($MODE): found \"$EXPECT\""
else
	echo "FAIL ($MODE): \"$EXPECT\" not seen within ${TIMEOUT}s" >&2
fi
echo "serial log: $LOG"
[ -f "$SHOT" ] && echo "screenshot: $SHOT"

# The virtio-console port output should contain the marker the guest sent.
if [ -f "$VCON" ] && grep -qF "rinux-virtio-console: hello from the port" "$VCON"; then
	echo "virtio-console: host received the port marker"
elif [ -s "$VCON" ]; then
	echo "virtio-console: port file has unexpected content:" >&2
	cat "$VCON" >&2
fi

# The guest should have read the marker the host fed in through input-path.
# (The UEFI firmware consumes the port input first, so this is checked on BIOS.)
if grep -qF "virtio-console: received" "$LOG" 2>/dev/null &&
	grep -qF "rinux-host-input-ok" "$LOG" 2>/dev/null; then
	echo "virtio-console: guest read the host input marker"
fi

exit $status
