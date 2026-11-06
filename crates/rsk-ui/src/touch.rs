// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Parsing a capacitive-touch report into a panel coordinate. The reference
//! display board uses CST328 (V1) or CST3530 (V2); the RP2350 AMOLED port adds
//! FT3168. Pure and host-tested: the firmware's display backend owns I2C reads
//! and hands raw report bytes here, so bit reconstruction and bounds checks
//! stay independent of the hardware driver.

use crate::{PANEL_H, PANEL_W, Point};

/// Finger-1 status nibble (CST328 register 0xD000, low 4 bits) meaning a finger
/// is down.
const FINGER_DOWN: u8 = 0x06;

/// Parse a CST328 report block read starting at register 0xD000, returning the
/// first active finger's coordinate or `None` (no finger down, or a short block).
///
/// Byte layout returned by the controller from 0xD000: `[0]` finger-1 status,
/// `[1]` X high 8 bits, `[2]` Y high 8 bits, `[3]` X low 4 bits (upper nibble) |
/// Y low 4 bits (lower nibble). Both axes are 12-bit. The controller is configured
/// at the panel resolution and reports the panel's own axes, so `display.rs` hands
/// this straight on — there is no rotation or flip step anywhere.
pub fn parse_cst328(report: &[u8]) -> Option<Point> {
    if report.len() < 4 || report[0] & 0x0F != FINGER_DOWN {
        return None;
    }
    let x = ((report[1] as u16) << 4) | ((report[3] as u16) >> 4);
    let y = ((report[2] as u16) << 4) | ((report[3] as u16) & 0x0F);
    Some(Point::new(x, y))
}

/// Finger-event nibble of a CST3530 report (`[8]` high 4 bits) for a lift: the
/// frame that ends a touch still carries the last coordinate, so a lift sample
/// must not be read as a tap.
const CST3530_LIFT: u8 = 0x00;

/// Parse a CST3530 report block read starting at register `0xD0070000` — the touch
/// controller Waveshare put on the **V2** revision of the reference board (the V1
/// carries a CST328). Returns the first finger's coordinate, or `None`.
///
/// Byte layout as read (nine bytes, no register prefix): `[3]` low nibble is the
/// number of points, `[4]` X low 8 bits, `[5]` Y low 8 bits, `[6]` strength,
/// `[7]` X high 4 bits in the low nibble | Y high 4 bits in the high nibble,
/// `[8]` finger id in the low nibble | finger event in the high nibble (1 press,
/// 2 hold, 0 lift). Both axes are 12-bit and already in the panel's own axes — as
/// with the CST328, no rotation or flip step follows.
pub fn parse_cst3530(report: &[u8]) -> Option<Point> {
    if report.len() < 9 || report[3] & 0x0F == 0 || report[8] & 0xF0 == CST3530_LIFT {
        return None;
    }
    let x = (report[4] as u16) | (((report[7] & 0x0F) as u16) << 8);
    let y = (report[5] as u16) | (((report[7] & 0xF0) as u16) << 4);
    Some(Point::new(x, y))
}

/// Parse the five-byte FT3168 report read starting at register 0x02:
/// finger count, X high, X low, Y high, Y low. Coordinates are 12-bit, with
/// the upper nibble of each high byte reserved for controller metadata.
///
/// `panel_width` and `panel_height` are the physical panel dimensions. A point
/// outside those bounds is rejected here, before it can be transformed into the
/// UI's logical coordinate space and used for security-sensitive hit testing.
pub fn parse_ft3168(report: &[u8], panel_width: u16, panel_height: u16) -> Option<Point> {
    if report.len() < 5 || report[0] & 0x0F == 0 || panel_width == 0 || panel_height == 0 {
        return None;
    }
    let x = (u16::from(report[1] & 0x0F) << 8) | u16::from(report[2]);
    let y = (u16::from(report[3] & 0x0F) << 8) | u16::from(report[4]);
    if x >= panel_width || y >= panel_height {
        return None;
    }
    Some(Point::new(x, y))
}


/// Map a physical FT3168 coordinate from the 368x448 RP2350 AMOLED panel to
/// the existing 240x320 logical UI. The logical surface is uniformly scaled
/// by 7/5 (1.4x), so its physical width is 336px and it is centered with
/// 16px side margins. The renderer must use the matching forward transform.
pub fn map_rp2350_amoled_to_ui(point: Point) -> Option<Point> {
    const X_OFFSET: u16 = 16;
    const CONTENT_WIDTH: u16 = 336;
    const PHYSICAL_HEIGHT: u16 = 448;
    const SCALE_NUMERATOR: u32 = 5;
    const SCALE_DENOMINATOR: u32 = 7;

    let physical_x = point.x.checked_sub(X_OFFSET)?;
    if physical_x >= CONTENT_WIDTH || point.y >= PHYSICAL_HEIGHT {
        return None;
    }

    let logical_x = u16::try_from(
        u32::from(physical_x) * SCALE_NUMERATOR / SCALE_DENOMINATOR,
    )
    .ok()?;
    let logical_y = u16::try_from(
        u32::from(point.y) * SCALE_NUMERATOR / SCALE_DENOMINATOR,
    )
    .ok()?;
    if logical_x >= PANEL_W || logical_y >= PANEL_H {
        return None;
    }

    Some(Point::new(logical_x, logical_y))
}

#[cfg(test)]
#[path = "touch_tests.rs"]
mod tests;
