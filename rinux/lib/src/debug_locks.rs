// SPDX-License-Identifier: GPL-2.0-only
//! The global switch that turns all lock debugging off, as in `lib/debug_locks.c`.
//!
//! The C code keeps two globals: `debug_locks`, which is 1 while lock
//! debugging is on, and `debug_locks_silent`, which suppresses the console
//! message. Here both live in a [`DebugLocks`] value; [`DEBUG_LOCKS`] is the
//! instance the kernel would use. The first detected bug turns debugging off
//! and reports once, so later bugs do not bury it in the log.
//!
//! `debug_locks_off()` in C calls `console_verbose()` itself. Here the caller
//! passes that action in as a closure, so the console stays out of this module.
//! The atomic `xchg` of the header becomes an `AtomicI32::swap`.

use core::sync::atomic::{AtomicI32, Ordering};

/// The two `debug_locks` globals of `lib/debug_locks.c`.
pub struct DebugLocks {
    /// `debug_locks`: non-zero while lock debugging is on. Starts at 1.
    locks: AtomicI32,
    /// `debug_locks_silent`: non-zero to report a bug without console output.
    silent: AtomicI32,
}

/// The instance the kernel uses for `debug_locks` and `debug_locks_silent`.
pub static DEBUG_LOCKS: DebugLocks = DebugLocks::new();

impl DebugLocks {
    /// Lock debugging on, not silent: the boot-time state in C.
    pub const fn new() -> Self {
        DebugLocks {
            locks: AtomicI32::new(1),
            silent: AtomicI32::new(0),
        }
    }

    /// `debug_locks != 0`.
    pub fn enabled(&self) -> bool {
        self.locks.load(Ordering::Relaxed) != 0
    }

    /// `debug_locks_silent != 0`.
    pub fn is_silent(&self) -> bool {
        self.silent.load(Ordering::Relaxed) != 0
    }

    /// Set `debug_locks_silent`. The locking test suite uses this to fail
    /// silently.
    pub fn set_silent(&self, silent: bool) {
        self.silent.store(silent as i32, Ordering::Relaxed);
    }

    /// `debug_locks_off()`: turn lock debugging off.
    ///
    /// Returns `true` only for the call that switched debugging off while it
    /// was on, and only when not silent. In that case `console_verbose` runs
    /// first, as `console_verbose()` does in C. A silent switch-off returns
    /// `false`, as it does in C, but still turns debugging off.
    pub fn off(&self, console_verbose: impl FnOnce()) -> bool {
        if self.enabled() && self.locks.swap(0, Ordering::Relaxed) != 0 && !self.is_silent() {
            console_verbose();
            return true;
        }
        false
    }
}

impl Default for DebugLocks {
    fn default() -> Self {
        Self::new()
    }
}
