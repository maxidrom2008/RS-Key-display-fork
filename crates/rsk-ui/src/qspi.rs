// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! SH8601 QSPI wire encoding helpers for the Waveshare RP2350-Touch-AMOLED-1.8.
//!
//! The board's vendor PIO program always shifts four output pins. SH8601
//! command/address bytes are therefore expanded into eight four-bit transfers
//! with only DIO0 asserted; pixel payload bytes are sent unmodified so each
//! high/low nibble drives DIO3..DIO0. Keeping the command encoder pure makes
//! bit order testable without a panel or RP2350.
//!
//! The command-byte expansion is adapted from Waveshare Team's
//! `rp2350/vendor/qspi/qspi_pio.c` (2025-03-20), distributed under the MIT
//! License. Copyright (c) Waveshare Team. The Waveshare permission notice and
//! disclaimer are reproduced below; this file also remains under the repository's
//! AGPL-3.0-only terms for its original portions.
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in
//! all copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
//! THE SOFTWARE.

/// Convert one SH8601 command/address byte to the four FIFO words expected by
/// Waveshare's four-output PIO program.
///
/// The PIO program uses left-shift, autopull threshold 8 and `out pins, 4`.
/// Each FIFO word carries one encoded byte in bits 31..24. The high nibble is
/// clocked first, then the low nibble. In each nibble only DIO0 can be set,
/// reproducing the byte MSB-first over the command lane.
pub const fn encode_sh8601_command_byte(byte: u8) -> [u32; 4] {
    let mut words = [0u32; 4];
    let mut pair = 0usize;
    while pair < 4 {
        let even_bit = (byte >> (pair * 2)) & 1;
        let odd_bit = (byte >> (pair * 2 + 1)) & 1;
        let encoded = even_bit | (odd_bit << 4);
        words[3 - pair] = (encoded as u32) << 24;
        pair += 1;
    }
    words
}

/// Split an RGB565 payload byte into the two four-lane values emitted by the
/// QSPI PIO program. The high nibble is transferred first, then the low nibble.
pub const fn qspi_pixel_nibbles(byte: u8) -> [u8; 2] {
    [byte >> 4, byte & 0x0F]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_encoder_sends_msb_first_on_dio0() {
        // 0xA5 = 1,0,1,0,0,1,0,1. Each pair becomes one PIO FIFO word
        // with its odd bit in the high nibble and even bit in the low nibble.
        assert_eq!(
            encode_sh8601_command_byte(0xA5),
            [0x1000_0000, 0x1000_0000, 0x0100_0000, 0x0100_0000]
        );
    }

    #[test]
    fn command_encoder_handles_zero_and_all_ones() {
        assert_eq!(encode_sh8601_command_byte(0x00), [0; 4]);
        assert_eq!(
            encode_sh8601_command_byte(0xFF),
            [0x1100_0000; 4]
        );
    }

    #[test]
    fn pixel_payload_is_high_nibble_first() {
        assert_eq!(qspi_pixel_nibbles(0xA5), [0x0A, 0x05]);
        assert_eq!(qspi_pixel_nibbles(0x00), [0x00, 0x00]);
        assert_eq!(qspi_pixel_nibbles(0xFF), [0x0F, 0x0F]);
    }
}
