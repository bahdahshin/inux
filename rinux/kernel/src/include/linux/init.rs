// SPDX-License-Identifier: GPL-2.0
//! Boot-time parameter registration.
//!
//! Ported from the `__setup()`/`early_param()` part of
//! `include/linux/init.h`. As in Linux, each `early_param!()` puts an
//! `ObsKernelParam` into the `.init.setup` linker section, and the linker
//! script brackets the section with `__setup_start` and `__setup_end`, so
//! handlers register themselves from anywhere in the kernel without a
//! central list.

/// `struct obs_kernel_param`.
#[repr(C)]
pub struct ObsKernelParam {
    /// The parameter name, without the `=`.
    pub str: &'static str,
    /// Handles the value, which is `None` for a bare `param` with no `=`
    /// (C passes NULL). Returns 0 on success, as in C.
    pub setup_func: fn(Option<&[u8]>) -> i32,
    /// Handled by `parse_early_param()` rather than at the normal time.
    pub early: bool,
}

unsafe extern "C" {
    // Linker script symbols; only their addresses mean anything.
    static __setup_start: u8;
    static __setup_end: u8;
}

/// All registered `early_param()` and `__setup()` handlers.
pub fn setup_params() -> &'static [ObsKernelParam] {
    let start = (&raw const __setup_start).cast::<ObsKernelParam>();
    let end = (&raw const __setup_end).cast::<ObsKernelParam>();
    // SAFETY: the linker script places only ObsKernelParam values, each
    // 8-byte aligned and a multiple of 8 bytes long, between the two
    // symbols, and nothing writes to them.
    unsafe { core::slice::from_raw_parts(start, end.offset_from(start) as usize) }
}

/// `early_param(str, fn)`: registers `fn` to handle `str=` during
/// `parse_early_param()`.
#[macro_export]
macro_rules! early_param {
    ($str:literal, $func:path) => {
        const _: () = {
            #[used]
            #[unsafe(link_section = ".init.setup")]
            static PARAM: $crate::include::linux::init::ObsKernelParam =
                $crate::include::linux::init::ObsKernelParam {
                    str: $str,
                    setup_func: $func,
                    early: true,
                };
        };
    };
}
