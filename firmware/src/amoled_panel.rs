// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! SH8601 register and pixel transactions for the Waveshare RP2350 AMOLED panel.
//!
//! Rendering remains in rsk-ui; this is the hardware-facing command layer.
//! It deliberately exposes bounded window writes instead of a framebuffer.

use embassy_futures::block_on;
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, block_for};

use crate::amoled_qspi::PioQspiTx;

/// 368x448 SH8601 panel connection.
pub(crate) struct Sh8601Panel {
    qspi: PioQspiTx,
    cs: Output<'static>,
    // Keep reset and panel power pins claimed for the panel's lifetime.
    _reset: Output<'static>,
    _power: Output<'static>,
}

impl Sh8601Panel {
    pub(crate) fn new(
        qspi: PioQspiTx,
        cs: Output<'static>,
        mut reset: Output<'static>,
        mut power: Output<'static>,
    ) -> Self {
        // Power is enabled before reset; keep chip select inactive during bring-up.
        power.set_high();
        reset.set_high();
        block_for(Duration::from_millis(10));

        reset.set_high();
        block_for(Duration::from_millis(50));
        reset.set_low();
        block_for(Duration::from_millis(50));
        reset.set_high();
        block_for(Duration::from_millis(300));

        let mut panel = Self {
            qspi,
            cs,
            _reset: reset,
            _power: power,
        };
        panel.initialize();
        panel
    }

    /// Send one SH8601 register write. The command and 24-bit register address
    /// are serially encoded on DIO0; data bytes use the same command lane.
    fn write_register(&mut self, register: u8, parameters: &[u8]) {
        self.cs.set_low();
        for byte in [0x02, 0x00, register, 0x00].into_iter().chain(parameters.iter().copied()) {
            self.qspi.write_command_byte(byte);
        }
        self.qspi.flush();
        self.cs.set_high();
    }

    fn initialize(&mut self) {
        // Waveshare RP2350 SH8601 initialization sequence.
        self.write_register(0x11, &[]); // Sleep out.
        block_for(Duration::from_millis(120));
        self.write_register(0x44, &[0x01, 0xC5]); // TE scanline.
        self.write_register(0x35, &[0x00]); // TE output.
        self.write_register(0x3A, &[0x55]); // RGB565.
        self.write_register(0xC4, &[0x80]);
        self.write_register(0x53, &[0x20]);
        self.write_register(0x51, &[0xFF]); // Full brightness.
        self.write_register(0x29, &[]); // Display on.
        block_for(Duration::from_millis(10));
    }

    pub(crate) fn set_brightness(&mut self, brightness: u8) {
        self.write_register(0x51, &[brightness]);
    }

    /// Set an exclusive-end pixel window, matching the display driver's Rect API.
    fn set_window(&mut self, x: u16, y: u16, width: u16, height: u16) {
        assert!(width != 0 && height != 0);
        assert!(u32::from(x) + u32::from(width) <= 368);
        assert!(u32::from(y) + u32::from(height) <= 448);
        let x_end = x + width - 1;
        let y_end = y + height - 1;
        self.write_register(
            0x2A,
            &[
                (x >> 8) as u8,
                x as u8,
                (x_end >> 8) as u8,
                x_end as u8,
            ],
        );
        self.write_register(
            0x2B,
            &[
                (y >> 8) as u8,
                y as u8,
                (y_end >> 8) as u8,
                y_end as u8,
            ],
        );
        self.write_register(0x2C, &[]);
    }

    /// Synchronous bounded write for one already-rasterized RGB565 band.
    /// pixels must contain exactly width*height*2 big-endian RGB565 bytes.
    pub(crate) fn write_window(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        pixels: &[u8],
    ) {
        let expected = usize::from(width) * usize::from(height) * 2;
        assert_eq!(pixels.len(), expected);
        self.set_window(x, y, width, height);

        self.cs.set_low();
        for byte in [0x32, 0x00, 0x2C, 0x00] {
            self.qspi.write_command_byte(byte);
        }
        self.qspi.flush();
        block_on(self.qspi.write_pixels(pixels));
        self.qspi.flush();
        self.cs.set_high();
    }

    pub(crate) fn clear(&mut self, color: u16, row: &mut [u8; 368 * 2]) {
        let bytes = color.to_be_bytes();
        for pixel in row.chunks_exact_mut(2) {
            pixel.copy_from_slice(&bytes);
        }
        self.set_window(0, 0, 368, 448);
        self.cs.set_low();
        for byte in [0x32, 0x00, 0x2C, 0x00] {
            self.qspi.write_command_byte(byte);
        }
        self.qspi.flush();
        for _ in 0..448 {
            block_on(self.qspi.write_pixels(row));
        }
        self.qspi.flush();
        self.cs.set_high();
    }
}
