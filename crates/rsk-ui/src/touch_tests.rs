// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

use super::*;

#[test]
fn reconstructs_finger_coordinate() {
    // x=100 (0x064): X high 8 = 0x06, X low 4 = 0x4. y=200 (0x0C8): Y high 8 =
    // 0x0C, Y low 4 = 0x8 — so the packed low-nibble byte is 0x48.
    let r = [FINGER_DOWN, 0x06, 0x0C, 0x48, 0, 0, 0];
    assert_eq!(parse_cst328(&r), Some(Point::new(100, 200)));
}

#[test]
fn no_finger_down_is_none() {
    assert_eq!(parse_cst328(&[0x00, 0x06, 0x0C, 0x48]), None);
}

#[test]
fn short_report_is_none() {
    assert_eq!(parse_cst328(&[FINGER_DOWN, 0x06]), None);
    assert_eq!(parse_cst328(&[]), None);
}

#[test]
fn full_scale_is_twelve_bits_each() {
    let r = [FINGER_DOWN, 0xFF, 0xFF, 0xFF];
    assert_eq!(parse_cst328(&r), Some(Point::new(0xFFF, 0xFFF)));
}

#[test]
fn cst3530_reconstructs_finger_coordinate() {
    // x=100 (0x064): low byte 0x64, high nibble 0. y=200 (0x0C8): low byte 0xC8,
    // high nibble 0 — so the packed nibble byte is 0x00, and 0x12 is finger 2
    // reporting a press.
    let r = [0, 0, 0, 0x01, 0x64, 0xC8, 0x40, 0x00, 0x12];
    assert_eq!(parse_cst3530(&r), Some(Point::new(100, 200)));
}

#[test]
fn cst3530_high_nibbles_are_the_upper_four_bits() {
    // x=0x123, y=0x2C8: the packed byte is X high nibble (1) | Y high nibble (2)
    // in its upper half, so 0x21.
    let r = [0, 0, 0, 0x01, 0x23, 0xC8, 0x40, 0x21, 0x11];
    assert_eq!(parse_cst3530(&r), Some(Point::new(0x123, 0x2C8)));
}

#[test]
fn cst3530_lift_frame_is_none() {
    // The frame that ends a touch still carries its coordinate; the event nibble is
    // 0, so it must not become a tap.
    let r = [0, 0, 0, 0x01, 0x64, 0xC8, 0x40, 0xC0, 0x02];
    assert_eq!(parse_cst3530(&r), None);
}

#[test]
fn cst3530_no_points_is_none() {
    let r = [0, 0, 0, 0x00, 0x64, 0xC8, 0x40, 0xC0, 0x12];
    assert_eq!(parse_cst3530(&r), None);
}

#[test]
fn cst3530_short_report_is_none() {
    assert_eq!(parse_cst3530(&[0, 0, 0, 0x01, 0x64]), None);
    assert_eq!(parse_cst3530(&[]), None);
}


#[test]
fn ft3168_reconstructs_coordinates() {
    // x=0x123, y=0x1B8. The high nibbles contain controller metadata and
    // must be masked before the 12-bit coordinate is assembled.
    let r = [0x01, 0xA1, 0x23, 0xB1, 0xB8];
    assert_eq!(parse_ft3168(&r, 368, 448), Some(Point::new(0x123, 0x1B8)));
}

#[test]
fn ft3168_accepts_last_physical_pixel() {
    let r = [1, 0x01, 0x6F, 0x01, 0xBF]; // x=367, y=447
    assert_eq!(parse_ft3168(&r, 368, 448), Some(Point::new(367, 447)));
}

#[test]
fn ft3168_no_finger_is_none() {
    assert_eq!(parse_ft3168(&[0, 0, 10, 0, 10], 368, 448), None);
}

#[test]
fn ft3168_out_of_panel_coordinate_is_none() {
    assert_eq!(parse_ft3168(&[1, 0x01, 0x70, 0, 10], 368, 448), None);
    assert_eq!(parse_ft3168(&[1, 0, 10, 0x01, 0xC0], 368, 448), None);
}

#[test]
fn ft3168_short_report_or_zero_panel_is_none() {
    assert_eq!(parse_ft3168(&[1, 0, 10, 0], 368, 448), None);
    assert_eq!(parse_ft3168(&[1, 0, 10, 0, 10], 0, 448), None);
    assert_eq!(parse_ft3168(&[1, 0, 10, 0, 10], 368, 0), None);
}


#[test]
fn amoled_transform_maps_the_content_origin_and_last_pixel() {
    assert_eq!(map_rp2350_amoled_to_ui(Point::new(16, 0)), Some(Point::new(0, 0)));
    assert_eq!(
        map_rp2350_amoled_to_ui(Point::new(351, 447)),
        Some(Point::new(239, 319))
    );
}

#[test]
fn amoled_transform_rejects_side_margins_and_outside_panel() {
    assert_eq!(map_rp2350_amoled_to_ui(Point::new(15, 100)), None);
    assert_eq!(map_rp2350_amoled_to_ui(Point::new(352, 100)), None);
    assert_eq!(map_rp2350_amoled_to_ui(Point::new(100, 448)), None);
}


#[test]
fn amoled_rect_transform_covers_full_panel_content() {
    assert_eq!(
        map_ui_rect_to_rp2350_amoled(Rect::new(0, 0, PANEL_W, PANEL_H)),
        Some(Rect::new(16, 0, 336, 448))
    );
}

#[test]
fn amoled_rect_transform_matches_touch_pixel_mapping() {
    // The first logical pixel occupies 2x2 physical pixels under the inverse
    // nearest-neighbour map; the final logical pixel occupies one physical pixel.
    assert_eq!(
        map_ui_rect_to_rp2350_amoled(Rect::new(0, 0, 1, 1)),
        Some(Rect::new(16, 0, 2, 2))
    );
    assert_eq!(
        map_ui_rect_to_rp2350_amoled(Rect::new(239, 319, 1, 1)),
        Some(Rect::new(351, 447, 1, 1))
    );
}

#[test]
fn amoled_rect_transform_rejects_empty_or_out_of_bounds_rectangles() {
    assert_eq!(map_ui_rect_to_rp2350_amoled(Rect::new(0, 0, 0, 1)), None);
    assert_eq!(map_ui_rect_to_rp2350_amoled(Rect::new(239, 0, 2, 1)), None);
    assert_eq!(map_ui_rect_to_rp2350_amoled(Rect::new(0, 319, 1, 2)), None);
}
