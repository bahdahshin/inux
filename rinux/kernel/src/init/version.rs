// SPDX-License-Identifier: GPL-2.0-only
//! The version banner.
//!
//! Ported from `init/version.c` and `init/version-timestamp.c`. The
//! release is the Linux version rinux is ported from, with `-rinux` as
//! the local version.

macro_rules! uts_release {
    () => {
        "7.3.0-rc5-rinux"
    };
}

/// `UTS_RELEASE`.
#[allow(dead_code)]
pub const UTS_RELEASE: &str = uts_release!();

/// `linux_banner`.
pub const LINUX_BANNER: &str = concat!(
    "Linux version ",
    uts_release!(),
    " (rinux@rinux) (rustc) #1\n"
);
