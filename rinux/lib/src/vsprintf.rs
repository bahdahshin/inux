// SPDX-License-Identifier: GPL-2.0-only
//! The printk format engine: `vsnprintf()`, `vsscanf()` and friends.
//!
//! Ported from `lib/vsprintf.c`.
//!
//! Copyright (C) 1991, 1992  Linus Torvalds
//!
//! vsprintf.c -- Lars Wirzenius & Linus Torvalds.
//!
//! # Arguments
//!
//! C passes a `va_list`; here the arguments are an [`Arg`] slice, and the
//! [`snprintf!`](crate::snprintf), [`scnprintf!`](crate::scnprintf) and
//! [`sprintf!`](crate::sprintf) macros build it with `Arg::from`. As in C
//! the conversion decides how an argument is read:
//!
//! - Integer conversions take the argument's integer value (a pointer's
//!   address for pointer arguments) and truncate or sign-extend it to the
//!   length modifier, so `%hhu` of 257 prints 1. Signed Rust integers are
//!   sign-extended to 64 bits when they become an [`Arg::Num`].
//! - `%s` and the `%p` extensions that read memory (`%ph`, `%pM`, `%pU`,
//!   ...) read an [`Arg::Str`] byte slice. Given anything else they treat
//!   it as an address they cannot read: `(null)` for 0 and `(efault)`
//!   otherwise, which is what C prints for NULL and invalid pointers. An
//!   extension that needs more bytes than the slice has prints `(einval)`.
//! - `%p` and `%px` print the argument's address.
//!
//! When the arguments run out, formatting stops there, as it does in C at
//! an unknown conversion.
//!
//! # `%p` extensions
//!
//! Ported: `%p` (hashed with SipHash once [`fill_ptr_key`] has been
//! called), `%px`, `%pK`, `%pe`, `%ph[CDN]`, `%pb[l]`, `%p[mM][FR][U]`,
//! `%p[iI][46][hnbl]`, `%pI6c`, `%pU[bBlL]`, `%pV` ([`Arg::Fmt`]), `%pA`
//! ([`Arg::Args`]), `%pNF`, `%p4c[chRlb]`, `%pa[pd]`, `%p[ku]s`, and
//! `%p[sSB]`, which prints the address as hex like C without kallsyms.
//!
//! Not yet ported, waiting for the structures they print: `%pR`/`%pr`
//! (resources and ranges), `%pE` (needs `string_escape_mem()`), `%pd`/`%pD`
//! (dentries), `%pt` (time), `%pC` (clocks), `%pg` (block devices), `%pG`
//! (page, VMA and GFP flags), `%pOF` and `%pfw` (firmware nodes) and the
//! sockaddr forms `%p[iI]S`. They print `(null)` or `(efault)` for bad
//! pointers like C, and `(%p<c>?)` otherwise; `%p[iI]S` prints `(%p[iI]?)`.
//!
//! `%pK` with `kptr_restrict == 1` prints the real pointer: rinux has no
//! tasks yet, so every caller is a kernel thread with all capabilities.
//! `%pe` names errors with the [`crate::errno`] table, which stands in for
//! `lib/errname.c`.
//!
//! # Not ported
//!
//! `kvasprintf()` and `kasprintf()` need the allocator; `vbin_printf()` and
//! `bstr_printf()` (`CONFIG_BINARY_PRINTF`) come with tracing; the
//! `hash_pointers=`, `no_hash_pointers` and `debug_boot_weak_hash` boot
//! parameters come with early_param support (the setters are here).
//!
//! The decimal conversion that emits two digits at a time is a performance
//! detail and is replaced by a plain loop with the same output.

use core::fmt;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};

use crate::bitops::{bits_to_longs, clear_bit, set_bit, test_bit};
use crate::ctype::{_tolower, isalnum, isascii, isdigit, isodigit, isprint, isspace, isxdigit};
use crate::errno::{EBUSY, Errno, Result, is_err_value};
use crate::find_bit::for_each_set_bitrange;
use crate::hash::hash_long;
use crate::hexdump::{HEX_ASC_UPPER, hex_asc_hi, hex_asc_lo, hex_byte_pack, hex_byte_pack_upper};
use crate::kstrtox::{KSTRTOX_OVERFLOW, parse_integer_fixup_radix, parse_integer_limit};
use crate::siphash::{SiphashKey, siphash_1u64};
use crate::string::skip_spaces;
use crate::uuid::{GUID_INDEX, UUID_INDEX, UUID_STRING_LEN};
use crate::warn_on_once;

#[inline]
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

#[inline]
fn sub(s: &[u8], i: usize) -> &[u8] {
    s.get(i..).unwrap_or(&[])
}

/// `PAGE_SIZE`: pointers below it are invalid.
const PAGE_SIZE: u64 = 4096;

// ---- simple_strto*() ----

/// `simple_strntoull()`: like [`simple_strtoull`] but reads at most
/// `max_chars` bytes, prefix included.
fn simple_strntoull(startp: &[u8], mut base: u32, max_chars: usize) -> (u64, usize) {
    let prefix_chars = parse_integer_fixup_radix(startp, &mut base);
    if prefix_chars < max_chars {
        let (rv, result) =
            parse_integer_limit(sub(startp, prefix_chars), base, max_chars - prefix_chars, 0);
        (result, prefix_chars + (rv & !KSTRTOX_OVERFLOW) as usize)
    } else {
        // Field too short for prefix + digit, skip over without converting.
        (0, max_chars)
    }
}

/// `simple_strtoull()`: converts the integer at the start of `cp`.
///
/// Returns the value and the index of the first byte not converted. Base 0
/// autodetects `0x` (hex) and `0` (octal). Overflow is not reported; the
/// value saturates at `u64::MAX`. Prefer [`crate::kstrtox::kstrtoull`].
pub fn simple_strtoull(cp: impl AsRef<[u8]>, base: u32) -> (u64, usize) {
    simple_strntoull(cp.as_ref(), base, i32::MAX as usize)
}

/// `simple_strtoul()`: [`simple_strtoull`]; `unsigned long` is 64 bits.
pub fn simple_strtoul(cp: impl AsRef<[u8]>, base: u32) -> (u64, usize) {
    simple_strtoull(cp, base)
}

/// `simple_strtol()`: [`simple_strtoul`] that accepts a leading `-`.
pub fn simple_strtol(cp: impl AsRef<[u8]>, base: u32) -> (i64, usize) {
    let cp = cp.as_ref();
    if at(cp, 0) == b'-' {
        let (v, end) = simple_strtoul(&cp[1..], base);
        return ((v as i64).wrapping_neg(), end + 1);
    }
    let (v, end) = simple_strtoul(cp, base);
    (v as i64, end)
}

/// `simple_strntoll()`.
fn simple_strntoll(cp: &[u8], base: u32, max_chars: usize) -> (i64, usize) {
    // simple_strntoull() safely handles receiving max_chars == 0 in the
    // case cp[0] == '-' && max_chars == 1.
    if at(cp, 0) == b'-' && max_chars > 0 {
        let (v, end) = simple_strntoull(&cp[1..], base, max_chars - 1);
        return ((v as i64).wrapping_neg(), end + 1);
    }
    let (v, end) = simple_strntoull(cp, base, max_chars);
    (v as i64, end)
}

/// `simple_strtoll()`: [`simple_strtol`].
pub fn simple_strtoll(cp: impl AsRef<[u8]>, base: u32) -> (i64, usize) {
    let (v, end) = simple_strntoll(cp.as_ref(), base, i32::MAX as usize);
    (v, end)
}

/// `skip_atoi()`: reads decimal digits at `*pos`. Wraps like x86 on
/// overflow (UB in C).
fn skip_atoi(s: &[u8], pos: &mut usize) -> i32 {
    let mut i: i32 = 0;
    loop {
        i = i
            .wrapping_mul(10)
            .wrapping_add(at(s, *pos) as i32 - b'0' as i32);
        *pos += 1;
        if !isdigit(at(s, *pos)) {
            break;
        }
    }
    i
}

/// Writes the decimal digits of `n` into `tmp` in reverse order; returns
/// their number. `put_dec()` in C, minus the two-digits-at-a-time tables.
fn put_dec(tmp: &mut [u8], mut n: u64) -> usize {
    let mut i = 0;
    loop {
        tmp[i] = b'0' + (n % 10) as u8;
        i += 1;
        n /= 10;
        if n == 0 {
            return i;
        }
    }
}

/// `num_to_str()`: `num` in decimal, right-aligned in `width` columns.
/// Returns the length, or 0 if it does not fit in `buf`.
pub fn num_to_str(buf: &mut [u8], num: u64, width: u32) -> usize {
    let size = buf.len();
    let mut tmp = [0u8; 24];
    let len = put_dec(&mut tmp, num);

    if len > size || width as usize > size {
        return 0;
    }

    let width = if width as usize > len {
        let w = width as usize - len;
        buf[..w].fill(b' ');
        w
    } else {
        0
    };
    for idx in 0..len {
        buf[idx + width] = tmp[len - idx - 1];
    }
    len + width
}

// ---- Output ----

/// Unsigned/signed.
const SIGN: u8 = 1;
/// Left justified.
const LEFT: u8 = 2;
/// Show plus.
const PLUS: u8 = 4;
/// Space if plus.
const SPACE: u8 = 8;
/// Pad with zero; must be `'0' - ' '`.
const ZEROPAD: u8 = 16;
/// Use lowercase in hex; must be `'a' ^ 'A'`.
const SMALL: u8 = 32;
/// Prefix hex with "0x", octal with "0".
const SPECIAL: u8 = 64;

const _: () = assert!(ZEROPAD == b'0' - b' ');
const _: () = assert!(SMALL == b'a' ^ b'A');

const FIELD_WIDTH_MAX: i32 = (1 << 23) - 1;
const PRECISION_MAX: i32 = (1 << 15) - 1;

/// `struct printf_spec`.
#[derive(Clone, Copy, Debug, Default)]
struct Spec {
    /// Flags to [`number`].
    flags: u8,
    /// Number base: 8, 10 or 16 only.
    base: u8,
    /// Number of digits or characters.
    precision: i16,
    /// Width of the output field.
    field_width: i32,
}

const DEFAULT_DEC_SPEC: Spec = Spec {
    flags: 0,
    base: 10,
    precision: -1,
    field_width: 0,
};

/// The output buffer. `pos` counts every byte produced, written or not, like
/// the C `buf` pointer running past `end`.
pub(crate) struct Out<'a> {
    buf: &'a mut [u8],
    pos: usize,
    /// Number of leading output bytes to drop. Only the bitmap printers
    /// set it; nothing that pads with [`Out::move_right`] runs then.
    skip: usize,
}

impl<'a> Out<'a> {
    fn new(buf: &'a mut [u8]) -> Out<'a> {
        Out {
            buf,
            pos: 0,
            skip: 0,
        }
    }

    #[inline]
    fn put(&mut self, c: u8) {
        if let Some(b) = self
            .pos
            .checked_sub(self.skip)
            .and_then(|i| self.buf.get_mut(i))
        {
            *b = c;
        }
        self.pos += 1;
    }

    fn puts(&mut self, s: &[u8]) {
        for &c in s {
            self.put(c);
        }
    }

    /// `move_right()`: shifts the `len` bytes at `start` right by `spaces`
    /// and fills the gap with spaces, as far as the buffer goes.
    fn move_right(&mut self, start: usize, len: usize, spaces: usize) {
        debug_assert_eq!(self.skip, 0);
        let end = self.buf.len();
        if start >= end {
            // Nowhere to put anything.
            return;
        }
        let size = end - start;
        if size <= spaces {
            self.buf[start..].fill(b' ');
            return;
        }
        if len != 0 {
            let len = len.min(size - spaces);
            self.buf.copy_within(start..start + len, start + spaces);
        }
        self.buf[start..start + spaces].fill(b' ');
    }
}

impl fmt::Write for Out<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.puts(s.as_bytes());
        Ok(())
    }
}

/// `number()`: formats `num` according to `spec`.
fn number(out: &mut Out, mut num: u64, mut spec: Spec) {
    let mut tmp = [0u8; 24];
    let need_pfx = spec.flags & SPECIAL != 0 && spec.base != 10;
    let is_zero = num == 0;
    let mut field_width = spec.field_width;
    let mut precision = spec.precision as i32;

    // locase = 0 or 0x20. ORing digits or letters with 'locase' produces
    // same digits or (maybe lowercased) letters.
    let locase = spec.flags & SMALL;
    if spec.flags & LEFT != 0 {
        spec.flags &= !ZEROPAD;
    }
    let mut sign = 0;
    if spec.flags & SIGN != 0 {
        if (num as i64) < 0 {
            sign = b'-';
            num = (num as i64).wrapping_neg() as u64;
            field_width -= 1;
        } else if spec.flags & PLUS != 0 {
            sign = b'+';
            field_width -= 1;
        } else if spec.flags & SPACE != 0 {
            sign = b' ';
            field_width -= 1;
        }
    }
    if need_pfx {
        if spec.base == 16 {
            field_width -= 2;
        } else if !is_zero {
            field_width -= 1;
        }
    }

    // Generate full string in tmp[], in reverse order.
    let base = spec.base as u64;
    let len = if num < base {
        tmp[0] = HEX_ASC_UPPER[num as usize] | locase;
        1
    } else if base != 10 {
        // 8 or 16
        let mask = (base - 1) as u8;
        let shift = if base == 16 { 4 } else { 3 };
        let mut i = 0;
        loop {
            tmp[i] = HEX_ASC_UPPER[(num as u8 & mask) as usize] | locase;
            i += 1;
            num >>= shift;
            if num == 0 {
                break i;
            }
        }
    } else {
        put_dec(&mut tmp, num)
    };
    let mut i = len as i32;

    // Printing 100 using %2d gives "100", not "00".
    if i > precision {
        precision = i;
    }
    // Leading space padding.
    field_width -= precision;
    if spec.flags & (ZEROPAD | LEFT) == 0 {
        loop {
            field_width -= 1;
            if field_width < 0 {
                break;
            }
            out.put(b' ');
        }
    }
    // Sign.
    if sign != 0 {
        out.put(sign);
    }
    // "0x" / "0" prefix.
    if need_pfx {
        if spec.base == 16 || !is_zero {
            out.put(b'0');
        }
        if spec.base == 16 {
            out.put(b'X' | locase);
        }
    }
    // Zero or space padding.
    if spec.flags & LEFT == 0 {
        let c = b' ' + (spec.flags & ZEROPAD);
        loop {
            field_width -= 1;
            if field_width < 0 {
                break;
            }
            out.put(c);
        }
    }
    // Even more zero padding?
    loop {
        precision -= 1;
        if i > precision {
            break;
        }
        out.put(b'0');
    }
    // Actual digits of result.
    while i > 0 {
        i -= 1;
        out.put(tmp[i as usize]);
    }
    // Trailing space padding.
    loop {
        field_width -= 1;
        if field_width < 0 {
            break;
        }
        out.put(b' ');
    }
}

/// `special_hex_number()`: `num` as `0x` and `2 * size` hex digits.
fn special_hex_number(out: &mut Out, num: u64, size: i32) {
    let spec = Spec {
        field_width: 2 + 2 * size,
        flags: SPECIAL | SMALL | ZEROPAD,
        base: 16,
        precision: -1,
    };
    number(out, num, spec);
}

/// `widen_string()`: pads the `n` bytes just written to the field width.
fn widen_string(out: &mut Out, n: usize, spec: Spec) {
    if n as i64 >= spec.field_width as i64 {
        return;
    }
    // We want to pad the sucker.
    let spaces = (spec.field_width as i64 - n as i64) as usize;
    if spec.flags & LEFT == 0 {
        out.move_right(out.pos - n, n, spaces);
        out.pos += spaces;
        return;
    }
    for _ in 0..spaces {
        out.put(b' ');
    }
}

/// `string_nocheck()`: a C string, cut at the precision, then padded.
fn string_nocheck(out: &mut Out, s: &[u8], spec: Spec) {
    let mut len = 0;
    let mut lim = spec.precision as i32;
    while lim != 0 {
        lim = lim.wrapping_sub(1);
        let c = at(s, len);
        if c == 0 {
            break;
        }
        out.put(c);
        len += 1;
    }
    widen_string(out, len, spec);
}

/// `error_string()`: an error message, cut at 16 bytes unless the format
/// gives a precision.
fn error_string(out: &mut Out, s: &[u8], mut spec: Spec) {
    // Hard limit to avoid completely insane messages.
    if spec.precision == -1 {
        spec.precision = 16;
    }
    string_nocheck(out, s, spec);
}

/// `check_pointer_msg()`: the message for an address that cannot be read.
fn check_pointer_msg(ptr: u64) -> Option<&'static [u8]> {
    if ptr == 0 {
        return Some(b"(null)");
    }
    if ptr < PAGE_SIZE || is_err_value(ptr) {
        return Some(b"(efault)");
    }
    None
}

/// The bytes a dereferencing conversion reads, or the message to print.
fn deref<'a>(arg: &Arg<'a>) -> core::result::Result<&'a [u8], &'static [u8]> {
    match *arg {
        Arg::Str(s) => Ok(s),
        _ => Err(check_pointer_msg(arg.word()).unwrap_or(b"(efault)")),
    }
}

/// [`deref`] for a conversion that needs at least `n` bytes.
fn deref_n<'a>(arg: &Arg<'a>, n: usize) -> core::result::Result<&'a [u8], &'static [u8]> {
    let s = deref(arg)?;
    if s.len() < n { Err(b"(einval)") } else { Ok(s) }
}

/// `string()`: `%s`.
fn string(out: &mut Out, arg: &Arg, spec: Spec) {
    match deref(arg) {
        Ok(s) => string_nocheck(out, s, spec),
        Err(msg) => error_string(out, msg, spec),
    }
}

/// `pointer_string()`: an address in hex, 16 digits unless a width is given.
fn pointer_string(out: &mut Out, ptr: u64, mut spec: Spec) {
    spec.base = 16;
    spec.flags |= SMALL;
    if spec.field_width == -1 {
        spec.field_width = 16;
        spec.flags |= ZEROPAD;
    }
    number(out, ptr, spec);
}

// ---- Pointer hashing ----

static NO_HASH_POINTERS: AtomicBool = AtomicBool::new(false);
static KPTR_RESTRICT: AtomicI32 = AtomicI32::new(0);
static DEBUG_BOOT_WEAK_HASH: AtomicBool = AtomicBool::new(false);
static FILLED_RANDOM_PTR_KEY: AtomicBool = AtomicBool::new(false);
static PTR_KEY: [AtomicU64; 2] = [AtomicU64::new(0), AtomicU64::new(0)];

/// `no_hash_pointers`: whether `%p` prints raw addresses.
pub fn no_hash_pointers() -> bool {
    NO_HASH_POINTERS.load(Ordering::Relaxed)
}

/// Sets `no_hash_pointers`, as the `no_hash_pointers` boot option does.
pub fn set_no_hash_pointers(on: bool) {
    NO_HASH_POINTERS.store(on, Ordering::Relaxed);
}

/// `kptr_restrict`: 0 hashes `%pK`, 1 prints it for privileged callers,
/// 2 prints zeros.
pub fn kptr_restrict() -> i32 {
    KPTR_RESTRICT.load(Ordering::Relaxed)
}

/// Sets `kptr_restrict` (the `kernel.kptr_restrict` sysctl).
pub fn set_kptr_restrict(val: i32) {
    KPTR_RESTRICT.store(val, Ordering::Relaxed);
}

/// Sets `debug_boot_weak_hash`: hash `%p` with [`hash_long`] instead of
/// SipHash, so pointers can be told apart before the RNG is up.
pub fn set_debug_boot_weak_hash(on: bool) {
    DEBUG_BOOT_WEAK_HASH.store(on, Ordering::Relaxed);
}

/// `fill_ptr_key()`: installs the random key that hashes `%p` pointers.
/// Until it is called, `%p` prints `(____ptrval____)`.
pub fn fill_ptr_key(key: SiphashKey) {
    PTR_KEY[0].store(key.key[0], Ordering::Relaxed);
    PTR_KEY[1].store(key.key[1], Ordering::Relaxed);
    // Pairs with the acquire in ptr_to_hashval() (smp_wmb()/smp_rmb() in C).
    FILLED_RANDOM_PTR_KEY.store(true, Ordering::Release);
}

/// `ptr_to_hashval()`: maps a pointer to a 32-bit unique identifier.
/// Returns `EBUSY` before [`fill_ptr_key`].
pub fn ptr_to_hashval(ptr: u64) -> Result<u64> {
    if !FILLED_RANDOM_PTR_KEY.load(Ordering::Acquire) {
        return Err(EBUSY);
    }
    let key = SiphashKey {
        key: [
            PTR_KEY[0].load(Ordering::Relaxed),
            PTR_KEY[1].load(Ordering::Relaxed),
        ],
    };
    // Mask off the first 32 bits, this makes explicit that we have modified
    // the address (and 32 bits is plenty for a unique ID).
    Ok(siphash_1u64(ptr, &key) & 0xffff_ffff)
}

/// `ptr_to_id()`: the hashed form of a pointer.
fn ptr_to_id(out: &mut Out, ptr: u64, mut spec: Spec) {
    // Print the real pointer value for NULL and error pointers, as they are
    // not actual addresses.
    if ptr == 0 || is_err_value(ptr) {
        return pointer_string(out, ptr, spec);
    }

    // When debugging early boot use non-cryptographically secure hash.
    if DEBUG_BOOT_WEAK_HASH.load(Ordering::Relaxed) {
        return pointer_string(out, hash_long(ptr, 32) as u64, spec);
    }

    match ptr_to_hashval(ptr) {
        Ok(hashval) => pointer_string(out, hashval, spec),
        Err(_) => {
            spec.field_width = 16;
            // String length must be less than default_width.
            error_string(out, b"(____ptrval____)", spec);
        }
    }
}

/// `default_pointer()`: `%p`.
fn default_pointer(out: &mut Out, ptr: u64, spec: Spec) {
    // Default is to _not_ leak addresses, so hash before printing, unless
    // no_hash_pointers is specified on the command line.
    if no_hash_pointers() {
        return pointer_string(out, ptr, spec);
    }
    ptr_to_id(out, ptr, spec);
}

/// `restricted_pointer()`: `%pK`.
fn restricted_pointer(out: &mut Out, ptr: u64, spec: Spec) {
    let ptr = match kptr_restrict() {
        // Handle as %p, hash and do _not_ leak addresses.
        0 => return default_pointer(out, ptr, spec),
        // C prints the pointer if the current task has CAP_SYSLOG and its
        // original credentials. Every rinux caller is a kernel thread.
        1 => ptr,
        // Always print 0's for %pK.
        _ => 0,
    };
    pointer_string(out, ptr, spec);
}

/// `err_ptr()`: `%pe` of an error pointer.
fn err_ptr(out: &mut Out, ptr: u64, mut spec: Spec) {
    let err = ptr as i64 as i32;
    if let Some(name) = Errno::from_errno(err).and_then(Errno::name) {
        let mut sym = [0u8; 40];
        sym[0] = b'-';
        let n = name.len().min(sym.len() - 1);
        sym[1..1 + n].copy_from_slice(&name.as_bytes()[..n]);
        return string_nocheck(out, &sym[..1 + n], spec);
    }
    // ERR_PTR(-1234) or some other non-existing Efoo: print the number.
    spec.flags |= SIGN;
    spec.base = 10;
    number(out, err as i64 as u64, spec);
}

// ---- %p extensions ----

/// `hex_string()`: `%*ph[CDN]`, at most 64 bytes.
fn hex_string(out: &mut Out, arg: &Arg, spec: Spec, fmt: &[u8]) {
    if spec.field_width == 0 {
        // Nothing to print.
        return;
    }
    // If we pass '%ph[CDN]', field width remains negative value, fallback
    // to the default.
    let len = if spec.field_width > 0 {
        spec.field_width.min(64) as usize
    } else {
        1
    };
    let addr = match deref_n(arg, len) {
        Ok(a) => a,
        Err(msg) => return error_string(out, msg, spec),
    };
    let separator = match at(fmt, 1) {
        b'C' => b':',
        b'D' => b'-',
        b'N' => 0,
        _ => b' ',
    };
    for (i, &byte) in addr[..len].iter().enumerate() {
        out.put(hex_asc_hi(byte));
        out.put(hex_asc_lo(byte));
        if separator != 0 && i != len - 1 {
            out.put(separator);
        }
    }
}

/// `bitmap_string()`: `%*pb`, comma-separated 32-bit hex chunks.
fn bitmap_string(out: &mut Out, bitmap: &[u64], nr_bits: usize) {
    const CHUNKSZ: usize = 32;
    // Reused to print numbers.
    let mut spec = Spec {
        flags: SMALL | ZEROPAD,
        base: 16,
        precision: 0,
        field_width: 0,
    };
    let mut chunksz = nr_bits & (CHUNKSZ - 1);
    if chunksz == 0 {
        chunksz = CHUNKSZ;
    }
    let mut first = true;
    let mut i = nr_bits.next_multiple_of(CHUNKSZ) as isize - CHUNKSZ as isize;
    while i >= 0 {
        let chunkmask = ((1u64 << chunksz) - 1) as u32;
        let word = i as usize / 64;
        let bit = i as usize % 64;
        let val = (bitmap[word] >> bit) as u32 & chunkmask;

        if !first {
            out.put(b',');
        }
        first = false;

        spec.field_width = chunksz.div_ceil(4) as i32;
        number(out, val as u64, spec);

        chunksz = CHUNKSZ;
        i -= CHUNKSZ as isize;
    }
}

/// `bitmap_list_string()`: `%*pbl`, ranges such as `0-3,8`.
fn bitmap_list_string(out: &mut Out, bitmap: &[u64], nr_bits: usize) {
    let mut first = true;
    for (rbot, rtop) in for_each_set_bitrange(bitmap, nr_bits) {
        if !first {
            out.put(b',');
        }
        first = false;

        number(out, rbot as u64, DEFAULT_DEC_SPEC);
        if rtop == rbot + 1 {
            continue;
        }
        out.put(b'-');
        number(out, (rtop - 1) as u64, DEFAULT_DEC_SPEC);
    }
}

/// `%*pb` and `%*pbl`: the bitmap argument and its size in bits.
fn bitmap_arg(out: &mut Out, arg: &Arg, spec: Spec, fmt: &[u8]) {
    let nr_bits = spec.field_width.max(0) as usize;
    let bitmap = match *arg {
        Arg::Bitmap(b) if b.len() >= bits_to_longs(nr_bits) => b,
        Arg::Bitmap(_) => return error_string(out, b"(einval)", spec),
        _ => {
            return error_string(
                out,
                check_pointer_msg(arg.word()).unwrap_or(b"(efault)"),
                spec,
            );
        }
    };
    if at(fmt, 1) == b'l' {
        bitmap_list_string(out, bitmap, nr_bits);
    } else {
        bitmap_string(out, bitmap, nr_bits);
    }
}

/// Prints `"%*pb\n"` or `"%*pbl\n"` and its NUL, keeping only the bytes from
/// `off` on that fit in `buf`, for `bitmap_print_*_to_buf()`. Returns the
/// number of bytes kept, like `memory_read_from_buffer()`.
pub(crate) fn bitmap_print_window(
    list: bool,
    buf: &mut [u8],
    maskp: &[u64],
    nmaskbits: usize,
    off: usize,
) -> usize {
    let count = buf.len();
    let mut out = Out {
        buf,
        pos: 0,
        skip: off,
    };
    if list {
        bitmap_list_string(&mut out, maskp, nmaskbits);
    } else {
        bitmap_string(&mut out, maskp, nmaskbits);
    }
    out.put(b'\n');
    out.put(0);
    let available = out.pos;
    if off >= available || count == 0 {
        return 0;
    }
    count.min(available - off)
}

/// `mac_address_string()`: `%p[mM][FR][U]`.
fn mac_address_string(out: &mut Out, arg: &Arg, spec: Spec, fmt: &[u8]) {
    let addr = match deref_n(arg, 6) {
        Ok(a) => a,
        Err(msg) => return error_string(out, msg, spec),
    };
    let mut separator = b':';
    let mut reversed = false;
    let mut uc = false;
    match at(fmt, 1) {
        b'F' => {
            uc = at(fmt, 2) == b'U';
            separator = b'-';
        }
        b'R' => {
            uc = at(fmt, 2) == b'U';
            reversed = true;
        }
        b'U' => uc = true,
        _ => {}
    }

    let mut mac_addr = [0u8; 17];
    let mut p = 0;
    for i in 0..6 {
        let byte = if reversed { addr[5 - i] } else { addr[i] };
        p += if uc {
            hex_byte_pack_upper(&mut mac_addr[p..], byte)
        } else {
            hex_byte_pack(&mut mac_addr[p..], byte)
        };
        if at(fmt, 0) == b'M' && i != 5 {
            mac_addr[p] = separator;
            p += 1;
        }
    }
    string_nocheck(out, &mac_addr[..p], spec);
}

/// `ip4_string()`: dotted quad of `addr` into `p`; returns the length.
/// `fmt[0] == b'i'` adds leading zeros; `fmt[2]` picks the byte order.
fn ip4_string(p: &mut [u8], addr: &[u8], fmt: [u8; 3]) -> usize {
    let leading_zeros = fmt[0] == b'i';
    let (mut index, step): (isize, isize) = match fmt[2] {
        // Host order: rinux targets are little-endian.
        b'h' => (3, -1),
        b'l' => (3, -1),
        _ => (0, 1),
    };
    let mut n = 0;
    for i in 0..4 {
        // Hold each IP quad in reverse order.
        let mut temp = [0u8; 4];
        let mut digits = put_dec(&mut temp, addr[index as usize] as u64);
        if leading_zeros {
            if digits < 3 {
                p[n] = b'0';
                n += 1;
            }
            if digits < 2 {
                p[n] = b'0';
                n += 1;
            }
        }
        // Reverse the digits in the quad.
        while digits > 0 {
            digits -= 1;
            p[n] = temp[digits];
            n += 1;
        }
        if i < 3 {
            p[n] = b'.';
            n += 1;
        }
        index += step;
    }
    n
}

/// `ipv6_addr_v4mapped()`: is `addr` `::ffff:a.b.c.d`?
fn ipv6_addr_v4mapped(addr: &[u8]) -> bool {
    addr[..10].iter().all(|&b| b == 0) && addr[10] == 0xff && addr[11] == 0xff
}

/// `ipv6_addr_is_isatap()`.
fn ipv6_addr_is_isatap(addr: &[u8]) -> bool {
    [addr[8] | 0x02, addr[9], addr[10], addr[11]] == [0x02, 0x00, 0x5e, 0xfe]
}

/// `ip6_compressed_string()`: RFC 5952 form of `addr`, such as `1::708`.
fn ip6_compressed_string(p: &mut [u8], addr: &[u8]) -> usize {
    let word = |j: usize| u16::from_be_bytes([addr[2 * j], addr[2 * j + 1]]);
    let use_ipv4 = ipv6_addr_v4mapped(addr) || ipv6_addr_is_isatap(addr);
    let range = if use_ipv4 { 6 } else { 8 };

    // Find position of longest 0 run.
    let mut zerolength = [0usize; 8];
    for (i, len) in zerolength[..range].iter_mut().enumerate() {
        *len = (i..range).take_while(|&j| word(j) == 0).count();
    }
    let mut longest = 1;
    let mut colonpos = None;
    for (i, &len) in zerolength[..range].iter().enumerate() {
        if len > longest {
            longest = len;
            colonpos = Some(i);
        }
    }
    if longest == 1 {
        // Don't compress a single 0.
        colonpos = None;
    }

    // Emit address.
    let mut n = 0;
    let mut needcolon = false;
    let mut i = 0;
    while i < range {
        if Some(i) == colonpos {
            if needcolon || i == 0 {
                p[n] = b':';
                n += 1;
            }
            p[n] = b':';
            n += 1;
            needcolon = false;
            i += longest;
            continue;
        }
        if needcolon {
            p[n] = b':';
            n += 1;
        }
        // Hex u16 without leading 0s.
        let [hi, lo] = word(i).to_be_bytes();
        if hi != 0 {
            if hi > 0x0f {
                n += hex_byte_pack(&mut p[n..], hi);
            } else {
                p[n] = hex_asc_lo(hi);
                n += 1;
            }
            n += hex_byte_pack(&mut p[n..], lo);
        } else if lo > 0x0f {
            n += hex_byte_pack(&mut p[n..], lo);
        } else {
            p[n] = hex_asc_lo(lo);
            n += 1;
        }
        needcolon = true;
        i += 1;
    }

    if use_ipv4 {
        if needcolon {
            p[n] = b':';
            n += 1;
        }
        n += ip4_string(&mut p[n..], &addr[12..16], [b'I', b'4', 0]);
    }
    n
}

/// `ip6_string()`: eight groups of four hex digits, colon-separated for
/// `%pI6`, run together for `%pi6`.
fn ip6_string(p: &mut [u8], addr: &[u8], fmt: &[u8]) -> usize {
    let mut n = 0;
    for i in 0..8 {
        n += hex_byte_pack(&mut p[n..], addr[2 * i]);
        n += hex_byte_pack(&mut p[n..], addr[2 * i + 1]);
        if at(fmt, 0) == b'I' && i != 7 {
            p[n] = b':';
            n += 1;
        }
    }
    n
}

/// `ip_addr_string()`: `%p[iI][46]`.
fn ip_addr_string(out: &mut Out, arg: &Arg, spec: Spec, fmt: &[u8]) {
    let need = match at(fmt, 1) {
        b'6' => 16,
        b'4' => 4,
        _ => 0,
    };
    let addr = match deref_n(arg, need) {
        Ok(a) => a,
        Err(msg) => return error_string(out, msg, spec),
    };
    let mut buf = [0u8; 48];
    let n = match at(fmt, 1) {
        b'6' if at(fmt, 0) == b'I' && at(fmt, 2) == b'c' => ip6_compressed_string(&mut buf, addr),
        b'6' => ip6_string(&mut buf, addr, fmt),
        b'4' => ip4_string(&mut buf, addr, [at(fmt, 0), at(fmt, 1), at(fmt, 2)]),
        _ => {
            let msg: &[u8] = if at(fmt, 0) == b'i' {
                b"(%pi?)"
            } else {
                b"(%pI?)"
            };
            return error_string(out, msg, spec);
        }
    };
    string_nocheck(out, &buf[..n], spec);
}

/// `va_format()`: `%pV`, a nested format and its arguments.
fn va_format(out: &mut Out, arg: &Arg, spec: Spec) {
    let Arg::Fmt(fmt, args) = *arg else {
        return error_string(
            out,
            check_pointer_msg(arg.word()).unwrap_or(b"(efault)"),
            spec,
        );
    };
    debug_assert_eq!(out.skip, 0);
    let start = out.pos.min(out.buf.len());
    out.pos += vsnprintf(&mut out.buf[start..], fmt, args);
}

/// `rust_fmt_argument()`: `%pA`, Rust `core::fmt::Arguments`.
fn rust_fmt_argument(out: &mut Out, arg: &Arg, spec: Spec) {
    let Arg::Args(args) = *arg else {
        return error_string(
            out,
            check_pointer_msg(arg.word()).unwrap_or(b"(efault)"),
            spec,
        );
    };
    let _ = fmt::Write::write_fmt(out, *args);
}

/// `uuid_string()`: `%pU[bBlL]`.
fn uuid_string(out: &mut Out, arg: &Arg, spec: Spec, fmt: &[u8]) {
    let addr = match deref_n(arg, 16) {
        Ok(a) => a,
        Err(msg) => return error_string(out, msg, spec),
    };
    let mut index = &UUID_INDEX;
    let mut uc = false;
    match at(fmt, 1) {
        b'L' => {
            uc = true;
            index = &GUID_INDEX;
        }
        b'l' => index = &GUID_INDEX,
        b'B' => uc = true,
        _ => {}
    }

    let mut uuid = [0u8; UUID_STRING_LEN];
    let mut p = 0;
    for (i, &idx) in index.iter().enumerate() {
        let byte = addr[idx as usize];
        p += if uc {
            hex_byte_pack_upper(&mut uuid[p..], byte)
        } else {
            hex_byte_pack(&mut uuid[p..], byte)
        };
        if matches!(i, 3 | 5 | 7 | 9) {
            uuid[p] = b'-';
            p += 1;
        }
    }
    string_nocheck(out, &uuid, spec);
}

/// `netdev_bits()`: `%pNF`, a `netdev_features_t`.
fn netdev_bits(out: &mut Out, arg: &Arg, spec: Spec, fmt: &[u8]) {
    let addr = match deref(arg) {
        Ok(a) => a,
        Err(msg) => return error_string(out, msg, spec),
    };
    if at(fmt, 1) != b'F' {
        return error_string(out, b"(%pN?)", spec);
    }
    let Some(bytes) = addr.first_chunk::<8>() else {
        return error_string(out, b"(einval)", spec);
    };
    special_hex_number(out, u64::from_ne_bytes(*bytes), 8);
}

/// Formats into a local buffer; returns the length.
fn number_into(buf: &mut [u8], num: u64, spec: Spec) -> usize {
    let mut out = Out::new(buf);
    number(&mut out, num, spec);
    out.pos
}

/// `fourcc_string()`: `%p4c[chRlb]`, a FourCC code.
fn fourcc_string(out: &mut Out, arg: &Arg, spec: Spec, fmt: &[u8]) {
    if at(fmt, 1) != b'c' {
        return error_string(out, b"(%p4?)", spec);
    }
    let addr = match deref_n(arg, 4) {
        Ok(a) => a,
        Err(msg) => return error_string(out, msg, spec),
    };

    let mut orig = u32::from_ne_bytes([addr[0], addr[1], addr[2], addr[3]]);
    let mut pixel_fmt = false;
    match at(fmt, 2) {
        b'h' => {
            if at(fmt, 3) == b'R' {
                orig = orig.swap_bytes();
            }
        }
        b'l' => orig = orig.to_le(),
        b'b' => orig = orig.to_be(),
        // Pixel formats are printed LSB-first.
        b'c' => pixel_fmt = true,
        _ => return error_string(out, b"(%p4?)", spec),
    }

    let val = if pixel_fmt {
        (orig & !(1 << 31)).swap_bytes()
    } else {
        orig
    };

    let mut output = [0u8; 32]; // sizeof("0123 little-endian (0x01234567)")
    let mut p = 0;
    for i in 0..4 {
        let c = (val >> ((3 - i) * 8)) as u8;
        // Print non-control ASCII characters as-is, dot otherwise.
        output[p] = if isascii(c) && isprint(c) { c } else { b'.' };
        p += 1;
    }

    if pixel_fmt {
        output[p] = b' ';
        p += 1;
        let s: &[u8] = if orig & (1 << 31) != 0 {
            b"big-endian"
        } else {
            b"little-endian"
        };
        output[p..p + s.len()].copy_from_slice(s);
        p += s.len();
    }

    output[p] = b' ';
    output[p + 1] = b'(';
    p += 2;
    let spec_hex = Spec {
        field_width: 10,
        flags: SPECIAL | SMALL | ZEROPAD,
        base: 16,
        precision: -1,
    };
    let len = output.len();
    p += number_into(&mut output[p..len - 2], orig as u64, spec_hex);
    output[p] = b')';
    p += 1;

    string_nocheck(out, &output[..p], spec);
}

/// `address_val()`: `%pa[pd]`, a `phys_addr_t` or `dma_addr_t`, both 64
/// bits.
fn address_val(out: &mut Out, arg: &Arg, spec: Spec) {
    let addr = match deref_n(arg, 8) {
        Ok(a) => a,
        Err(msg) => return error_string(out, msg, spec),
    };
    let num = u64::from_ne_bytes([
        addr[0], addr[1], addr[2], addr[3], addr[4], addr[5], addr[6], addr[7],
    ]);
    special_hex_number(out, num, 8);
}

/// An extension that is not ported yet: `(null)`/`(efault)` for bad
/// pointers like C, `(%p<c>?)` otherwise.
fn unsupported(out: &mut Out, arg: &Arg, c: u8, spec: Spec) {
    match deref(arg) {
        Err(msg) => error_string(out, msg, spec),
        Ok(_) => error_string(out, &[b'(', b'%', b'p', c, b'?', b')'], spec),
    }
}

/// `pointer()`: `%p` and its extensions. `fmt` starts after the `p`.
fn pointer(fmt: &[u8], out: &mut Out, arg: &Arg, spec: Spec) {
    let ptr = arg.word();
    match at(fmt, 0) {
        // symbol_string() without kallsyms.
        b'S' | b's' | b'B' => special_hex_number(out, ptr, 8),
        b'h' => hex_string(out, arg, spec, fmt),
        b'b' => bitmap_arg(out, arg, spec, fmt),
        // Colon separated: 00:01:02:03:04:05, contiguous: 000102030405,
        // [mM]F (FDDI), [mM]R (reverse order; Bluetooth), [mM][FR][U]
        // (one of the above in upper case).
        b'M' | b'm' => mac_address_string(out, arg, spec, fmt),
        // Formatted IP: 4: 1.2.3.4, 6: 0001:0203:...:0708, 6c: 1::708 or
        // 1::1.2.3.4. Contiguous: 4: 001.002.003.004, 6: 000102...0f.
        b'I' | b'i' => ip_addr_string(out, arg, spec, fmt),
        b'U' => uuid_string(out, arg, spec, fmt),
        b'V' => va_format(out, arg, spec),
        b'K' => restricted_pointer(out, ptr, spec),
        b'N' => netdev_bits(out, arg, spec, fmt),
        b'4' => fourcc_string(out, arg, spec, fmt),
        b'a' => address_val(out, arg, spec),
        b'A' => rust_fmt_argument(out, arg, spec),
        b'x' => pointer_string(out, ptr, spec),
        // %pe with a non-ERR_PTR gets treated as plain %p.
        b'e' if !is_err_value(ptr) => default_pointer(out, ptr, spec),
        b'e' => err_ptr(out, ptr, spec),
        b'u' | b'k' if at(fmt, 1) == b's' => string(out, arg, spec),
        b'u' | b'k' => error_string(out, b"(einval)", spec),
        c @ (b'R' | b'r' | b'E' | b'd' | b'D' | b't' | b'C' | b'g' | b'G' | b'O' | b'f') => {
            unsupported(out, arg, c, spec)
        }
        _ => default_pointer(out, ptr, spec),
    }
}

// ---- Format decoding ----

/// `enum format_state`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    /// Just a string part.
    None,
    Num,
    Width,
    Precision,
    Char,
    Str,
    Ptr,
    PercentChar,
    Invalid,
}

/// `struct fmt`: where decoding is, what it found, and the size of numbers.
#[derive(Clone, Copy, Debug)]
struct Fmt {
    pos: usize,
    state: State,
    size: u8,
}

/// `spec_flag()`.
fn spec_flag(c: u8) -> u8 {
    match c {
        b' ' => SPACE,
        b'#' => SPECIAL,
        b'+' => PLUS,
        b'-' => LEFT,
        b'0' => ZEROPAD,
        _ => 0,
    }
}

fn set_field_width(spec: &mut Spec, width: i32) {
    spec.field_width = width.clamp(-FIELD_WIDTH_MAX, FIELD_WIDTH_MAX);
    warn_on_once!(spec.field_width != width);
}

fn set_precision(spec: &mut Spec, prec: i32) {
    spec.precision = prec.clamp(0, PRECISION_MAX) as i16;
    warn_on_once!((spec.precision as i32) < prec);
}

/// `format_decode()`: decodes the next token of `fmt`: a run of literal
/// text, a conversion, or a `*` width or precision that needs an argument.
fn format_decode(fmt: &[u8], mut f: Fmt, spec: &mut Spec) -> Fmt {
    // We finished early by reading the field width.
    if f.state == State::Width {
        if spec.field_width < 0 {
            spec.field_width = -spec.field_width;
            spec.flags |= LEFT;
        }
        f.state = State::None;
        return decode_precision(fmt, f, spec);
    }

    // We finished early by reading the precision.
    if f.state == State::Precision {
        if spec.precision < 0 {
            spec.precision = 0;
        }
        f.state = State::None;
        return decode_qualifier(fmt, f, spec);
    }

    // By default.
    f.state = State::None;

    let start = f.pos;
    while at(fmt, f.pos) != 0 && at(fmt, f.pos) != b'%' {
        f.pos += 1;
    }

    // Return the current non-format string.
    if f.pos != start || at(fmt, f.pos) == 0 {
        return f;
    }

    // Process flags. This also skips the first '%'.
    spec.flags = 0;
    loop {
        f.pos += 1;
        let flag = spec_flag(at(fmt, f.pos));
        spec.flags |= flag;
        if flag == 0 {
            break;
        }
    }

    // Get field width.
    spec.field_width = -1;
    if isdigit(at(fmt, f.pos)) {
        set_field_width(spec, skip_atoi(fmt, &mut f.pos));
    } else if at(fmt, f.pos) == b'*' {
        // It's the next argument.
        f.state = State::Width;
        f.pos += 1;
        return f;
    }

    decode_precision(fmt, f, spec)
}

fn decode_precision(fmt: &[u8], mut f: Fmt, spec: &mut Spec) -> Fmt {
    spec.precision = -1;
    if at(fmt, f.pos) == b'.' {
        f.pos += 1;
        if isdigit(at(fmt, f.pos)) {
            set_precision(spec, skip_atoi(fmt, &mut f.pos));
        } else if at(fmt, f.pos) == b'*' {
            // It's the next argument.
            f.state = State::Precision;
            f.pos += 1;
            return f;
        }
    }
    decode_qualifier(fmt, f, spec)
}

fn decode_qualifier(fmt: &[u8], mut f: Fmt, spec: &mut Spec) -> Fmt {
    // Set up default numeric format.
    spec.base = 10;
    f.state = State::Num;
    f.size = 4;

    // Length: size and, if the letter is doubled, the double size.
    let (size, double_size) = match at(fmt, f.pos) {
        b'l' => (8, 8),
        b'L' => (8, 0),
        b'h' => (2, 1),
        b'H' => (1, 0), // Questionable historical
        b'z' => (8, 0),
        b't' => (8, 0),
        _ => (0, 0),
    };
    if size != 0 {
        f.size = size;
        if double_size != 0 && at(fmt, f.pos) == at(fmt, f.pos + 1) {
            f.size = double_size;
            f.pos += 1;
        }
        f.pos += 1;
    }

    let (state, flags, base) = match at(fmt, f.pos) {
        // Non-numeric formats.
        b'c' => (State::Char, 0, 0),
        b's' => (State::Str, 0, 0),
        b'p' => (State::Ptr, 0, 0),
        b'%' => (State::PercentChar, 0, 0),
        // Numerics.
        b'o' => (State::Num, 0, 8),
        b'x' => (State::Num, SMALL, 16),
        b'X' => (State::Num, 0, 16),
        b'd' | b'i' => (State::Num, SIGN, 10),
        b'u' => (State::Num, 0, 10),
        // Since %n poses a greater security risk than utility, treat it as
        // any other invalid or unsupported format specifier.
        _ => {
            warn_on_once!(true);
            f.state = State::Invalid;
            return f;
        }
    };
    if base != 0 {
        spec.base = base;
    }
    spec.flags |= flags;
    f.state = state;
    f.pos += 1;
    f
}

/// `convert_num_spec()`: truncates a 1-, 2- or 4-byte value and extends it
/// to 64 bits according to its signedness.
fn convert_num_spec(val: u32, size: u8, spec: Spec) -> u64 {
    let shift = 32 - size as u32 * 8;
    let val = val << shift;
    if spec.flags & SIGN == 0 {
        return (val >> shift) as u64;
    }
    ((val as i32) >> shift) as i64 as u64
}

// ---- Arguments and entry points ----

/// A `vsnprintf()` argument. See the [module documentation](self) for how
/// conversions read each kind.
#[derive(Clone, Copy, Debug)]
pub enum Arg<'a> {
    /// An integer, `char` or `bool`. Signed values are sign-extended.
    Num(u64),
    /// Memory for `%s` (a C string ending at NUL or the end of the slice)
    /// and for the `%p` extensions that read bytes.
    Str(&'a [u8]),
    /// A bare address, which `vsnprintf()` cannot read through.
    Ptr(usize),
    /// A bitmap for `%*pb` and `%*pbl`.
    Bitmap(&'a [u64]),
    /// `struct va_format` for `%pV`: a format and its arguments.
    Fmt(&'a [u8], &'a [Arg<'a>]),
    /// Rust format arguments for `%pA`.
    Args(&'a fmt::Arguments<'a>),
}

impl Arg<'_> {
    /// `ERR_PTR(err)`, for `%pe`.
    pub const fn err_ptr(err: i32) -> Arg<'static> {
        Arg::Ptr(err as isize as usize)
    }

    /// The value an integer conversion or `%p` sees: the number itself, or
    /// the address of what the argument points to.
    pub fn word(&self) -> u64 {
        match *self {
            Arg::Num(n) => n,
            Arg::Str(s) => s.as_ptr() as usize as u64,
            Arg::Ptr(p) => p as u64,
            Arg::Bitmap(b) => b.as_ptr() as usize as u64,
            Arg::Fmt(f, _) => f.as_ptr() as usize as u64,
            Arg::Args(a) => a as *const fmt::Arguments as usize as u64,
        }
    }
}

macro_rules! arg_from_int {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Arg<'_> {
                #[inline]
                fn from(v: $t) -> Self {
                    Arg::Num(v as u64)
                }
            }
        )*
    };
}

arg_from_int!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl From<bool> for Arg<'_> {
    fn from(v: bool) -> Self {
        Arg::Num(v as u64)
    }
}

impl From<char> for Arg<'_> {
    fn from(v: char) -> Self {
        Arg::Num(v as u64)
    }
}

impl<'a> From<&'a [u8]> for Arg<'a> {
    fn from(s: &'a [u8]) -> Self {
        Arg::Str(s)
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for Arg<'a> {
    fn from(s: &'a [u8; N]) -> Self {
        Arg::Str(s)
    }
}

impl<'a> From<&'a str> for Arg<'a> {
    fn from(s: &'a str) -> Self {
        Arg::Str(s.as_bytes())
    }
}

impl<'a> From<&'a [u64]> for Arg<'a> {
    fn from(b: &'a [u64]) -> Self {
        Arg::Bitmap(b)
    }
}

impl<'a, const N: usize> From<&'a [u64; N]> for Arg<'a> {
    fn from(b: &'a [u64; N]) -> Self {
        Arg::Bitmap(b)
    }
}

impl<T: ?Sized> From<*const T> for Arg<'_> {
    fn from(p: *const T) -> Self {
        Arg::Ptr(p.cast::<()>() as usize)
    }
}

impl<T: ?Sized> From<*mut T> for Arg<'_> {
    fn from(p: *mut T) -> Self {
        Arg::Ptr(p.cast::<()>() as usize)
    }
}

impl<'a> From<&'a fmt::Arguments<'a>> for Arg<'a> {
    fn from(a: &'a fmt::Arguments<'a>) -> Self {
        Arg::Args(a)
    }
}

/// `vsnprintf()`: formats into `buf`, writing at most `buf.len()` bytes
/// including the NUL terminator.
///
/// Returns the length the whole output would have, not counting the NUL,
/// as C99 does: a return value `>= buf.len()` means the output was cut.
///
/// This follows C99 with the kernel's extensions and limits: `%n` is not
/// supported, a precision with no digits (`%.s`) means no precision, a
/// negative `*` precision means 0, and `%.0d` of 0 prints `0`.
pub fn vsnprintf(buf: &mut [u8], fmt: impl AsRef<[u8]>, args: &[Arg]) -> usize {
    let fmt = fmt.as_ref();
    let size = buf.len();

    // Reject out-of-range values early.
    if warn_on_once!(size > i32::MAX as usize) {
        return 0;
    }

    let mut out = Out::new(buf);
    let mut args = args.iter();
    let mut spec = Spec::default();
    let mut f = Fmt {
        pos: 0,
        state: State::None,
        size: 0,
    };

    while at(fmt, f.pos) != 0 {
        let old = f.pos;
        f = format_decode(fmt, f, &mut spec);

        if f.state == State::None {
            out.puts(&fmt[old..f.pos]);
            continue;
        }
        if matches!(f.state, State::PercentChar) {
            out.put(b'%');
            continue;
        }
        // There is no safe or sane way to continue after an unsupported
        // specifier; the remaining specifiers and arguments would be out of
        // sync. Running out of arguments ends formatting too.
        let Some(arg) = args.next().filter(|_| f.state != State::Invalid) else {
            break;
        };

        match f.state {
            State::Num => {
                let num = if f.size > 4 {
                    arg.word()
                } else {
                    convert_num_spec(arg.word() as u32, f.size, spec)
                };
                number(&mut out, num, spec);
            }
            State::Width => set_field_width(&mut spec, arg.word() as i32),
            State::Precision => set_precision(&mut spec, arg.word() as i32),
            State::Char => {
                if spec.flags & LEFT == 0 {
                    loop {
                        spec.field_width -= 1;
                        if spec.field_width <= 0 {
                            break;
                        }
                        out.put(b' ');
                    }
                }
                out.put(arg.word() as u8);
                loop {
                    spec.field_width -= 1;
                    if spec.field_width <= 0 {
                        break;
                    }
                    out.put(b' ');
                }
            }
            State::Str => string(&mut out, arg, spec),
            State::Ptr => {
                pointer(sub(fmt, f.pos), &mut out, arg, spec);
                while isalnum(at(fmt, f.pos)) {
                    f.pos += 1;
                }
            }
            State::None | State::PercentChar | State::Invalid => unreachable!(),
        }
    }

    if size > 0 {
        if out.pos < size {
            out.buf[out.pos] = 0;
        } else {
            out.buf[size - 1] = 0;
        }
    }

    // The trailing null byte doesn't count towards the total.
    let ret = out.pos;
    if warn_on_once!(ret > i32::MAX as usize) {
        return i32::MAX as usize;
    }
    ret
}

/// `vscnprintf()`: like [`vsnprintf`] but returns the number of bytes
/// written, not counting the NUL; 0 for an empty buffer.
pub fn vscnprintf(buf: &mut [u8], fmt: impl AsRef<[u8]>, args: &[Arg]) -> usize {
    let size = buf.len();
    if size == 0 {
        return 0;
    }
    let i = vsnprintf(buf, fmt, args);
    if i < size { i } else { size - 1 }
}

/// `vsprintf()`: [`vsnprintf`] for callers that know the output fits.
/// Where C would overflow the buffer, this panics.
pub fn vsprintf(buf: &mut [u8], fmt: impl AsRef<[u8]>, args: &[Arg]) -> usize {
    let size = buf.len();
    let ret = vsnprintf(buf, fmt, args);
    assert!(ret < size, "sprintf() output does not fit in its buffer");
    ret
}

/// `snprintf(buf, fmt, args...)`: [`vsnprintf`] with the arguments
/// converted by `Arg::from`.
#[macro_export]
macro_rules! snprintf {
    ($buf:expr, $fmt:expr $(, $arg:expr)* $(,)?) => {
        $crate::vsprintf::vsnprintf($buf, $fmt, &[$($crate::vsprintf::Arg::from($arg)),*])
    };
}

/// `scnprintf(buf, fmt, args...)`: [`vscnprintf`] with the arguments
/// converted by `Arg::from`.
#[macro_export]
macro_rules! scnprintf {
    ($buf:expr, $fmt:expr $(, $arg:expr)* $(,)?) => {
        $crate::vsprintf::vscnprintf($buf, $fmt, &[$($crate::vsprintf::Arg::from($arg)),*])
    };
}

/// `sprintf(buf, fmt, args...)`: [`vsprintf`] with the arguments converted
/// by `Arg::from`.
#[macro_export]
macro_rules! sprintf {
    ($buf:expr, $fmt:expr $(, $arg:expr)* $(,)?) => {
        $crate::vsprintf::vsprintf($buf, $fmt, &[$($crate::vsprintf::Arg::from($arg)),*])
    };
}

// ---- sscanf() ----

/// A `vsscanf()` destination. Integer conversions store their value,
/// truncated to the length modifier as C does, into any integer variant;
/// `%s`, `%c` and `%[` copy into [`ScanArg::Bytes`].
#[derive(Debug)]
pub enum ScanArg<'a> {
    I8(&'a mut i8),
    U8(&'a mut u8),
    I16(&'a mut i16),
    U16(&'a mut u16),
    I32(&'a mut i32),
    U32(&'a mut u32),
    I64(&'a mut i64),
    U64(&'a mut u64),
    Isize(&'a mut isize),
    Usize(&'a mut usize),
    /// A character buffer. Like C, the conversions panic (overflow in C) if
    /// it is too small.
    Bytes(&'a mut [u8]),
}

macro_rules! scan_arg_from {
    ($($t:ty => $v:ident),*) => {
        $(
            impl<'a> From<&'a mut $t> for ScanArg<'a> {
                #[inline]
                fn from(p: &'a mut $t) -> Self {
                    ScanArg::$v(p)
                }
            }
        )*
    };
}

scan_arg_from!(i8 => I8, u8 => U8, i16 => I16, u16 => U16, i32 => I32, u32 => U32, i64 => I64, u64 => U64,
               isize => Isize, usize => Usize, [u8] => Bytes);

impl<'a, const N: usize> From<&'a mut [u8; N]> for ScanArg<'a> {
    fn from(p: &'a mut [u8; N]) -> Self {
        ScanArg::Bytes(p)
    }
}

impl ScanArg<'_> {
    /// Stores an integer; false if this is not an integer destination.
    fn store(&mut self, v: u64) -> bool {
        match self {
            ScanArg::I8(p) => **p = v as i8,
            ScanArg::U8(p) => **p = v as u8,
            ScanArg::I16(p) => **p = v as i16,
            ScanArg::U16(p) => **p = v as u16,
            ScanArg::I32(p) => **p = v as i32,
            ScanArg::U32(p) => **p = v as u32,
            ScanArg::I64(p) => **p = v as i64,
            ScanArg::U64(p) => **p = v,
            ScanArg::Isize(p) => **p = v as isize,
            ScanArg::Usize(p) => **p = v as usize,
            ScanArg::Bytes(_) => return false,
        }
        true
    }
}

/// `vsscanf()`: parses `buf` according to `fmt` into `args`. Returns the
/// number of arguments assigned.
///
/// White space in the format matches any amount of white space, including
/// none. `%*` skips a field up to the next white space. Supported
/// conversions are `%d %i %u %o %x %X` with `hh h l ll L z` modifiers,
/// `%c`, `%s`, `%n`, `%%` and `%[`, which unlike glibc has no ranges,
/// cannot match `]`, needs a field width and cannot be skipped with `%*[`.
pub fn vsscanf(buf: impl AsRef<[u8]>, fmt: impl AsRef<[u8]>, args: &mut [ScanArg]) -> usize {
    let buf = buf.as_ref();
    let fmt = fmt.as_ref();
    let mut s = 0;
    let mut f = 0;
    let mut num = 0;
    let mut args = args.iter_mut();

    while at(fmt, f) != 0 {
        // White space in format matches any amount of white space,
        // including none, in the input.
        if isspace(at(fmt, f)) {
            f += 1;
            f += skip_spaces(sub(fmt, f));
            s += skip_spaces(sub(buf, s));
        }

        // Anything that is not a conversion must match exactly.
        if at(fmt, f) != b'%' && at(fmt, f) != 0 {
            let (fc, sc) = (at(fmt, f), at(buf, s));
            f += 1;
            s += 1;
            if fc != sc {
                break;
            }
            continue;
        }

        if at(fmt, f) == 0 {
            break;
        }
        f += 1;

        // Skip this conversion: advance both strings to next white space.
        if at(fmt, f) == b'*' {
            if at(buf, s) == 0 {
                break;
            }
            while !isspace(at(fmt, f)) && at(fmt, f) != b'%' && at(fmt, f) != 0 {
                // '%*[' not yet supported, invalid format.
                if at(fmt, f) == b'[' {
                    return num;
                }
                f += 1;
            }
            while !isspace(at(buf, s)) && at(buf, s) != 0 {
                s += 1;
            }
            continue;
        }

        // Get field width.
        let mut field_width: i16 = -1;
        if isdigit(at(fmt, f)) {
            field_width = skip_atoi(fmt, &mut f) as i16;
            if field_width <= 0 {
                break;
            }
        }

        // Get conversion qualifier.
        let mut qualifier = 0xffu8;
        let c = at(fmt, f);
        if c == b'h' || _tolower(c) == b'l' || c == b'z' {
            qualifier = c;
            f += 1;
            if qualifier == at(fmt, f) {
                if qualifier == b'h' {
                    qualifier = b'H';
                    f += 1;
                } else if qualifier == b'l' {
                    qualifier = b'L';
                    f += 1;
                }
            }
        }

        if at(fmt, f) == 0 {
            break;
        }

        if at(fmt, f) == b'n' {
            // Return number of characters read so far.
            if !args
                .next()
                .is_some_and(|arg| arg.store(s as i32 as i64 as u64))
            {
                return num;
            }
            f += 1;
            continue;
        }

        if at(buf, s) == 0 {
            break;
        }

        let mut base = 10;
        let mut is_sign = false;

        let conv = at(fmt, f);
        f += 1;
        match conv {
            b'c' => {
                let Some(ScanArg::Bytes(d)) = args.next() else {
                    return num;
                };
                if field_width == -1 {
                    field_width = 1;
                }
                let mut i = 0;
                loop {
                    d[i] = at(buf, s);
                    i += 1;
                    s += 1;
                    field_width -= 1;
                    if field_width <= 0 || at(buf, s) == 0 {
                        break;
                    }
                }
                num += 1;
                continue;
            }
            b's' => {
                let Some(ScanArg::Bytes(d)) = args.next() else {
                    return num;
                };
                if field_width == -1 {
                    field_width = i16::MAX;
                }
                // First, skip leading white space in buffer.
                s += skip_spaces(sub(buf, s));
                // Now copy until next white space.
                let mut i = 0;
                while at(buf, s) != 0 && !isspace(at(buf, s)) && field_width != 0 {
                    field_width -= 1;
                    d[i] = at(buf, s);
                    i += 1;
                    s += 1;
                }
                d[i] = 0;
                num += 1;
                continue;
            }
            b'[' => {
                let Some(ScanArg::Bytes(d)) = args.next() else {
                    return num;
                };
                let mut set = [0u64; 4];
                let mut len = 0;
                let negate = at(fmt, f) == b'^';

                // Field width is required.
                if field_width == -1 {
                    return num;
                }
                if negate {
                    f += 1;
                }
                while at(fmt, f) != 0 && at(fmt, f) != b']' {
                    set_bit(at(fmt, f) as usize, &mut set);
                    f += 1;
                    len += 1;
                }
                // No ']' or no character set found.
                if at(fmt, f) == 0 || len == 0 {
                    return num;
                }
                f += 1;

                if negate {
                    for w in &mut set {
                        *w = !*w;
                    }
                    // Exclude null '\0' byte.
                    clear_bit(0, &mut set);
                }

                // Match must be non-empty.
                if !test_bit(at(buf, s) as usize, &set) {
                    return num;
                }

                let mut i = 0;
                while test_bit(at(buf, s) as usize, &set) && field_width != 0 {
                    field_width -= 1;
                    d[i] = at(buf, s);
                    i += 1;
                    s += 1;
                }
                d[i] = 0;
                num += 1;
                continue;
            }
            b'o' => base = 8,
            b'x' | b'X' => base = 16,
            b'i' => {
                base = 0;
                is_sign = true;
            }
            b'd' => is_sign = true,
            b'u' => {}
            b'%' => {
                // Looking for '%' in str.
                if at(buf, s) != b'%' {
                    return num;
                }
                s += 1;
                continue;
            }
            // Invalid format; stop here.
            _ => return num,
        }

        // Have some sort of integer conversion. First, skip white space in
        // buffer.
        s += skip_spaces(sub(buf, s));

        let mut digit = at(buf, s);
        if is_sign && digit == b'-' {
            if field_width == 1 {
                break;
            }
            digit = at(buf, s + 1);
        }

        if digit == 0
            || (base == 16 && !isxdigit(digit))
            || (base == 10 && !isdigit(digit))
            || (base == 8 && !isodigit(digit))
            || (base == 0 && !isdigit(digit))
        {
            break;
        }

        let max_chars = if field_width >= 0 {
            field_width as usize
        } else {
            i32::MAX as usize
        };
        let (val, next) = if is_sign {
            let (v, next) = simple_strntoll(sub(buf, s), base, max_chars);
            (v as u64, next)
        } else {
            simple_strntoull(sub(buf, s), base, max_chars)
        };

        let v = match qualifier {
            // That's 'hh' in format.
            b'H' if is_sign => val as i8 as u64,
            b'H' => val as u8 as u64,
            b'h' if is_sign => val as i16 as u64,
            b'h' => val as u16 as u64,
            b'l' | b'L' | b'z' => val,
            _ if is_sign => val as i32 as u64,
            _ => val as u32 as u64,
        };
        if !args.next().is_some_and(|arg| arg.store(v)) {
            return num;
        }
        num += 1;
        s += next;
    }
    num
}

/// `sscanf(buf, fmt, args...)`: [`vsscanf`] with the destinations
/// converted by `ScanArg::from`, so `&mut x` works for integers and byte
/// arrays.
#[macro_export]
macro_rules! sscanf {
    ($buf:expr, $fmt:expr $(, $arg:expr)* $(,)?) => {
        $crate::vsprintf::vsscanf($buf, $fmt, &mut [$($crate::vsprintf::ScanArg::from($arg)),*])
    };
}
