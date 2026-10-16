// SPDX-License-Identifier: GPL-2.0
//! `WARN_ON()` and `BUG_ON()`.
//!
//! Ported in spirit from `include/asm-generic/bug.h`. In C, `WARN_ON()` prints
//! a backtrace through printk. The library has no console, so it counts the
//! warning and calls a handler that the kernel installs once printk exists.

use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

/// Called with the source location of every warning.
pub type WarnHandler = fn(file: &'static str, line: u32);

static HANDLER: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());
static WARNINGS: AtomicUsize = AtomicUsize::new(0);

/// Installs the function that reports warnings (normally printk).
pub fn set_warn_handler(handler: WarnHandler) {
    HANDLER.store(handler as *mut (), Ordering::Release);
}

/// Number of warnings raised since boot.
pub fn warn_count() -> usize {
    WARNINGS.load(Ordering::Relaxed)
}

#[doc(hidden)]
#[cold]
pub fn __warn(file: &'static str, line: u32) {
    WARNINGS.fetch_add(1, Ordering::Relaxed);
    let handler = HANDLER.load(Ordering::Acquire);
    if !handler.is_null() {
        // SAFETY: HANDLER only ever holds null or a `WarnHandler` stored by
        // `set_warn_handler`.
        let handler = unsafe { core::mem::transmute::<*mut (), WarnHandler>(handler) };
        handler(file, line);
    }
}

/// `WARN_ON(cond)`: reports a warning if `cond` is true and evaluates to `cond`.
#[macro_export]
macro_rules! warn_on {
    ($cond:expr) => {{
        let cond: bool = $cond;
        if cond {
            $crate::bug::__warn(file!(), line!());
        }
        cond
    }};
}

/// `WARN_ON_ONCE(cond)`: like [`warn_on!`] but reports only the first time.
#[macro_export]
macro_rules! warn_on_once {
    ($cond:expr) => {{
        static WARNED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
        let cond: bool = $cond;
        if cond && !WARNED.swap(true, core::sync::atomic::Ordering::Relaxed) {
            $crate::bug::__warn(file!(), line!());
        }
        cond
    }};
}

/// `BUG_ON(cond)`: panics if `cond` is true.
#[macro_export]
macro_rules! bug_on {
    ($cond:expr) => {
        if $cond {
            panic!(concat!("BUG_ON(", stringify!($cond), ")"));
        }
    };
}
