#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0
"""Generate Rust constants from the numeric #defines of Linux headers.

Hardware headers such as processor-flags.h, msr-index.h and serial_reg.h
are long lists of register numbers and bit masks. This script evaluates
each object-like #define (numbers, references to earlier defines, BIT(),
GENMASK(), shifts and ORs) and writes it as a Rust `pub const`.

Function-like macros and expressions it cannot evaluate exactly (casts,
`~` on 32-bit values, unknown names) are skipped and listed at the top of
the generated file, so nothing is dropped silently.

    python rinux/scripts/gen_consts.py
"""

import re
import sys
from pathlib import Path

RINUX = Path(__file__).resolve().parent.parent
LINUX = RINUX.parent / "linux"
KERNEL_SRC = RINUX / "kernel" / "src"

# (Linux header, Rust output, Rust type)
HEADERS = [
    ("arch/x86/include/uapi/asm/processor-flags.h",
     "arch/x86/include/processor_flags.rs", "u64"),
    ("arch/x86/include/asm/msr-index.h",
     "arch/x86/include/msr_index.rs", "u64"),
    ("include/uapi/linux/serial_reg.h",
     "include/uapi/serial_reg.rs", "u32"),
    ("arch/x86/include/asm/segment.h",
     "arch/x86/include/segment.rs", "u16"),
    ("arch/x86/include/asm/trapnr.h",
     "arch/x86/include/trapnr.rs", "u32"),
    ("arch/x86/include/asm/irq_vectors.h",
     "arch/x86/include/irq_vectors.rs", "u32"),
]

# Macros the C preprocessor would see when building x86_64_defconfig.
PREDEFINED = {
    "__x86_64__", "__KERNEL__", "CONFIG_X86_64",
    "CONFIG_X86_LOCAL_APIC", "CONFIG_X86_IO_APIC", "CONFIG_PCI_MSI",
}

LIMITS = {"u8": 0xFF, "u16": 0xFFFF, "u32": 0xFFFF_FFFF, "u64": 0xFFFF_FFFF_FFFF_FFFF}

DEFINE = re.compile(r"^\s*#\s*define\s+([A-Za-z_]\w*)(\()?\s*(.*)$")
DIRECTIVE = re.compile(r"^\s*#\s*(ifdef|ifndef|if|elif|else|endif)\b\s*(.*)$")


def condition(expr, defined, known):
    """Evaluate an #if expression. Earlier numeric defines have their
    values; other defined names are 1 and unknown ones 0, as in C."""
    expr = re.sub(r"\bdefined\s*\(?\s*(\w+)\s*\)?", lambda m: "1" if m.group(1) in defined else "0", expr)
    expr = re.sub(r"\bIS_ENABLED\s*\(\s*(\w+)\s*\)", lambda m: "1" if m.group(1) in defined else "0", expr)
    expr = to_python(expr)
    expr = re.sub(r"\b[A-Za-z_]\w*\b",
                  lambda m: str(known[m.group(0)]) if m.group(0) in known
                  else "1" if m.group(0) in defined else "0", expr)
    expr = re.sub(r"!(?!=)", " not ", expr.replace("&&", " and ").replace("||", " or "))
    try:
        return bool(eval(expr, {"__builtins__": {}}, {}))
    except Exception:
        return False


def logical_lines(text):
    """Join backslash continuations and strip comments."""
    text = re.sub(r"/\*.*?\*/", lambda m: _comment_marker(m.group(0)), text, flags=re.S)
    text = re.sub(r"//([^\n]*)", lambda m: _comment_marker("/*" + m.group(1) + "*/"), text)
    return text.replace("\\\n", " ").splitlines()


_comments = []


def _comment_marker(comment):
    _comments.append(" ".join(comment[2:-2].split()))
    return f"@@C{len(_comments) - 1}@@"


def to_python(expr):
    expr = re.sub(r"\b(0[xX][0-9a-fA-F]+|\d+)(?:[uU]?[lL]{0,2}|[lL]{1,2}[uU])\b", r"\1", expr)
    expr = re.sub(r"\b(?:BIT_ULL|BIT|_BITULL|_BITUL)\s*\(", "_bit(", expr)
    expr = re.sub(r"\b(?:GENMASK_ULL|GENMASK)\s*\(", "_genmask(", expr)
    expr = re.sub(r"\b_AC\s*\(\s*([^,()]+?)\s*,\s*\w+\s*\)", r"(\1)", expr)
    return expr.replace("/", "//")


def evaluate(expr, known):
    if "~" in expr or "?" in expr or '"' in expr or "'" in expr:
        return None
    py = to_python(expr)
    names = set(re.findall(r"\b[A-Za-z_]\w*\b", py)) - {"_bit", "_genmask"}
    if any(n not in known for n in names):
        return None
    if re.search(r"\(\s*[a-z_]+\d*_?t?\s*\)", expr):  # C cast
        return None
    try:
        value = eval(py, {"__builtins__": {}}, {
            **known,
            "_bit": lambda n: 1 << n,
            "_genmask": lambda h, l: ((1 << (h - l + 1)) - 1) << l,
        })
    except Exception:
        return None
    return value if isinstance(value, int) else None


def convert(rel, out_rel, rtype):
    src = (LINUX / rel).read_text(encoding="utf-8")
    _comments.clear()
    known, consts, skipped = {}, [], []
    defined = set(PREDEFINED)
    # One entry per open #if: (this branch active, some branch already taken).
    stack = []
    guard = None
    for line in logical_lines(src):
        doc = " ".join(_comments[int(i)] for i in re.findall(r"@@C(\d+)@@", line))
        line = re.sub(r"@@C\d+@@", " ", line)
        d = DIRECTIVE.match(line)
        if d:
            kind, arg = d.group(1), d.group(2).strip()
            outer = all(active for active, _ in stack)
            if kind in ("ifdef", "ifndef", "if"):
                if kind == "ifdef":
                    cond = arg.split()[0] in defined
                elif kind == "ifndef":
                    cond = arg.split()[0] not in defined
                else:
                    cond = condition(arg, defined, known)
                stack.append((outer and cond, cond))
            elif kind == "elif" and stack:
                _, taken = stack.pop()
                cond = not taken and condition(arg, defined, known)
                stack.append((all(a for a, _ in stack) and cond, taken or cond))
            elif kind == "else" and stack:
                _, taken = stack.pop()
                stack.append((all(a for a, _ in stack) and not taken, True))
            elif kind == "endif" and stack:
                stack.pop()
            continue
        if not all(active for active, _ in stack):
            continue
        m = DEFINE.match(line)
        if not m:
            continue
        name, func, body = m.group(1), m.group(2), m.group(3).strip()
        defined.add(name)
        if guard is None and not body:
            guard = name
            continue
        if func:
            skipped.append(f"{name}()")
            continue
        if not body:
            continue
        value = evaluate(body, known)
        if value is None or value < 0 or value > LIMITS[rtype]:
            skipped.append(name)
            continue
        known[name] = value
        consts.append((name, value, doc))

    lines = [
        "// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note"
        if "uapi" in rel else "// SPDX-License-Identifier: GPL-2.0",
        "//",
        f"// @generated by rinux/scripts/gen_consts.py from linux/{rel}.",
        "// Do not edit by hand; re-run the script instead.",
    ]
    if skipped:
        lines.append("//")
        lines.append("// Not converted (function-like macros or expressions that are not")
        lines.append("// plain numbers):")
        for i in range(0, len(skipped), 4):
            lines.append("//   " + ", ".join(skipped[i:i + 4]))
    lines.append("")
    for name, value, doc in consts:
        if doc:
            lines.append(f"/// {doc}")
        lit = f"{value:#x}" if value > 9 else str(value)
        lines.append(f"pub const {name}: {rtype} = {lit};")
    out = KERNEL_SRC / out_rel
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"{out_rel}: {len(consts)} constants, {len(skipped)} skipped")


def main():
    if not LINUX.exists():
        sys.exit("linux submodule not found next to rinux/")
    for rel, out_rel, rtype in HEADERS:
        convert(rel, out_rel, rtype)


if __name__ == "__main__":
    main()
