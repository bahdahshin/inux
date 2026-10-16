// SPDX-License-Identifier: GPL-2.0
//! Device drivers, as in Linux's `drivers/`.

pub mod block;
pub mod char;
pub mod clocksource;
pub mod input;
pub mod net;
pub mod pci;
pub mod rtc;
pub mod tty;
pub mod virtio;
