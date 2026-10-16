#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0
"""Report how much of the Linux tree has been converted to Rust.

Reads the file table in rinux/PORTING.md and compares it with the C
sources, headers and assembly files tracked in the linux submodule:

    python rinux/scripts/progress.py

A table row counts when its first cell is a backticked path that exists
in linux/ and its status is `done` or `partial`.
"""

import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

RINUX = Path(__file__).resolve().parent.parent
LINUX = RINUX.parent / "linux"
ROW = re.compile(r"^\|\s*`([^`]+)`\s*\|[^|]*\|\s*(done|partial|test)\s*\|")


def tracked_sources():
    out = subprocess.run(
        ["git", "-C", str(LINUX), "ls-files", "*.c", "*.h", "*.S"],
        capture_output=True, text=True, check=True,
    ).stdout
    return set(out.split())


def main():
    sources = tracked_sources()
    if not sources:
        sys.exit("no C files found; is the linux submodule checked out?")

    status = {}
    for line in (RINUX / "PORTING.md").read_text(encoding="utf-8").splitlines():
        m = ROW.match(line)
        if m:
            status[m.group(1)] = m.group(2)

    missing = sorted(p for p in status if p not in sources)
    if missing:
        sys.exit("PORTING.md lists files that are not in linux/: " + ", ".join(missing))

    counts = Counter(status.values())
    converted = len(status)
    total = len(sources)
    print(f"Converted: {converted} of {total} C, header and assembly files ({100 * converted / total:.3f}%)")
    print(f"  done: {counts['done']}, partial: {counts['partial']}, tests: {counts['test']}")

    print("\nBy top-level directory:")
    per_dir_total = Counter(p.split("/")[0] for p in sources)
    per_dir_done = Counter(p.split("/")[0] for p in status)
    for d in sorted(per_dir_done):
        print(f"  {d + '/':<10} {per_dir_done[d]:>5} of {per_dir_total[d]:>6}")


if __name__ == "__main__":
    main()
