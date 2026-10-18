// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `stmp_reset_block`, over a simulated register block that
//! clears and sets bits the way the SET/CLR aliases do. Linux has no KUnit file
//! for this driver helper.

use rinux_lib::errno::ETIMEDOUT;
use rinux_lib::stmp_device::*;

/// A block whose value register (offset 0) responds to SET and CLR writes.
#[derive(Default)]
struct Block {
    value: u32,
    writes: Vec<(usize, u32)>,
    /// When set, a clear of this mask never completes.
    stuck: u32,
}

impl StmpRegs for Block {
    fn read(&mut self, offset: usize) -> u32 {
        assert_eq!(offset, 0);
        self.value
    }
    fn write(&mut self, offset: usize, v: u32) {
        self.writes.push((offset, v));
        match offset {
            STMP_OFFSET_REG_SET => {
                self.value |= v;
                // Soft reset makes the block gate its clock, as the hardware does.
                if v & STMP_MODULE_SFTRST != 0 {
                    self.value |= STMP_MODULE_CLKGATE;
                }
            }
            STMP_OFFSET_REG_CLR
                if v & self.stuck == 0 => {
                    self.value &= !v;
                }
            _ => {}
        }
    }
    fn delay_us_1(&mut self) {}
}

#[test]
fn reset_sequence_ends_with_both_bits_clear() {
    let mut b = Block::default();
    assert_eq!(stmp_reset_block(&mut b), Ok(()));
    assert_eq!(b.value & (STMP_MODULE_SFTRST | STMP_MODULE_CLKGATE), 0);
    // The first write clears soft reset, as C does.
    assert_eq!(b.writes[0], (STMP_OFFSET_REG_CLR, STMP_MODULE_SFTRST));
}

#[test]
fn a_stuck_soft_reset_times_out() {
    let mut b = Block {
        value: STMP_MODULE_SFTRST,
        stuck: STMP_MODULE_SFTRST,
        ..Default::default()
    };
    assert_eq!(stmp_reset_block(&mut b), Err(ETIMEDOUT));
}
