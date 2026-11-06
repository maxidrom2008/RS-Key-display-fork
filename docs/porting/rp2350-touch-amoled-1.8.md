<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# Waveshare RP2350-Touch-AMOLED-1.8 port

**Status: bring-up work in progress. This board is not supported by a release image yet.**

This document records the hardware contract for a future `display-amoled` firmware
flavor. Do not use the existing `display` image for this board: that image targets
the ST7789/CST328 hardware on the RP2350-Touch-LCD-2.8.

## Board identity

- MCU: RP2350A
- External QSPI flash: W25Q128JVSIQ, 16 MiB
- Display: 1.8-inch AMOLED, 368 x 448, SH8601
- Display host interface: 4-lane QSPI
- Touch controller: FT3168, I2C1, 7-bit address 0x38

Sources:
- [Waveshare product page](https://www.waveshare.com/RP2350-Touch-AMOLED-1.8.htm)
- [Waveshare wiki](https://www.waveshare.com/wiki/RP2350-Touch-AMOLED-1.8)
- Waveshare's official Pico SDK demo archive, linked from the wiki.
  Its source headers retain the permissive Waveshare license; preserve that
  notice if any driver code is adapted.

## Pin assignment

The values below are from the Waveshare RP2350 Pico SDK sample's
`DEV_Config.h` and `qspi_pio.h`. Do not reuse the similarly named ESP32-S3
AMOLED board pin map: this is the RP2350 board and it wires the display
directly to RP2350 GPIOs.

| Signal | RP2350 GPIO | Notes |
|---|---:|---|
| LCD_CS | 9 | Active-low panel chip select |
| QSPI_SCLK | 10 | Panel serial clock |
| QSPI_SIO0 | 11 | QSPI data lane 0 |
| QSPI_SIO1 | 12 | QSPI data lane 1 |
| QSPI_SIO2 | 13 | QSPI data lane 2 |
| QSPI_SIO3 | 14 | QSPI data lane 3 |
| LCD_RESET | 15 | Active-low panel reset |
| DSI_PWR_EN | 17 | Drive high to enable panel power |
| TP_INT | 4 | FT3168 interrupt; polling is sufficient for first bring-up |
| TP_RESET | 5 | FT3168 reset, active-low pulse |
| I2C1_SDA | 6 | FT3168 data |
| I2C1_SCL | 7 | FT3168 clock |

The sample also labels GPIO8 as `DOF_INT1` and GPIO18 as `SYS_OUT`; they are
not display/touch signals and are not claimed by this port. GPIO16 is not listed
as an AMOLED signal in the RP2350 sample header.

The panel has no separate PWM backlight pin. Brightness is set with the SH8601
command `0x51` (the vendor demo calls `AMOLED_1IN8_SetBrightness`), so the
existing LCD backlight-PWM path must not be reused. GPIO17 is panel power enable,
not a brightness output.

## QSPI transport contract

The Waveshare Pico SDK sample uses PIO0 state machine 0 with a four-output
program (`out pins, 4` plus a clock side-set). It sends command/address bytes
over DIO0 by software-expanding each serial byte into four PIO words; pixel data
is sent as raw bytes, two nibbles per PIO word, across DIO0..DIO3. The sample
contains a separate one-output PIO program, but its initialization is commented
out and the demo does not use that state machine.

The sample's QSPI GPIO setup owns GPIO9..17 as shown above, and selects the
panel with GPIO9. The existing RS-Key ST7789 transport is a different protocol:
it has a D/C pin and emits one data bit at a time. It cannot be retargeted by
changing only pin numbers. The AMOLED backend must implement the SH8601 command,
address/window and quad-pixel phases explicitly, preserving CS and the PIO
transaction boundaries exactly as required by the controller.

The display is 368 x 448 RGB pixels. At RGB565, a full frame is 329,728 bytes.
The old LCD UI uses a 240 x 320 logical coordinate space; the port must apply one
consistent transform to both rendering and touch hit-testing. A reasonable first
layout is uniform 1.4x scaling (336 x 448) with 16-pixel horizontal margins.
Do not scale the output while leaving touch coordinates untransformed.

## FT3168 touch report

Waveshare's FT3168 header defines the 7-bit address as `0x38` and these
registers:
- `0x02`: finger count;
- `0x03..0x04`: first-contact X high/low;
- `0x05..0x06`: first-contact Y high/low;
- `0xA0`: device ID, expected to be `0x03`.

Coordinate reconstruction is `((high & 0x0f) << 8) | low` for each axis.
The first implementation should read the five-byte block beginning at 0x02,
treat a zero finger count as no contact, and reject coordinates outside the
368 x 448 physical panel. GPIO4 may later be used for interrupt-driven sampling;
a short polling loop is adequate for initial bring-up.

## Port architecture

1. Keep the trusted-display state machine and policy in `rsk-display`; it is
   hardware independent and already owns the security-sensitive approve/deny
   and PIN flows.
2. Add a distinct RP2350 AMOLED hardware backend. The existing
   `display_panel.rs` is an ST7789 single-lane transport and is not a QSPI
   implementation.
3. Implement SH8601 command/register writes and QSPI pixel streaming using
   RP2350 PIO + DMA. Commands/window setup and pixel payloads have different
   transfer phases; keep the panel selected only for complete transactions.
4. Implement FT3168 I2C reads and parse them in a pure, host-tested function in
   `rsk-ui`. Map physical touch coordinates to the exact inverse of the panel
   rendering transform. Clamp/reject invalid contacts before hit-testing.
5. Preserve the existing 240 x 320 logical UI initially and scale/translate
   rendering and touch coordinates consistently, or undertake a separately
   tested responsive-layout change for 368 x 448.
6. Bring GPIO17 power enable high, pulse GPIO15 reset with the vendor timing,
   initialize SH8601 sleep-out/display-on, pixel format, orientation and
   brightness. Keep panel power and reset sequencing explicit.
7. Add a board-specific build flavor and compile-time pin-collision checks.
   The existing LCD defaults (ST7789 GPIO13/14/15, PWM backlight GPIO16,
   CST328/CST3530 on I2C1) must remain unchanged.

## Validation gates

- Host tests for FT3168 report parsing, coordinate transforms, clipping, and
  QSPI byte/nibble packing.
- Firmware compile for both the existing LCD flavor and the new AMOLED flavor.
- Hardware smoke test: panel power/reset/init, color bars, full-frame DMA
  transfer, partial-window transfer, FT3168 touch coordinate readout, and
  repeated power/reset cycles.
- End-to-end trusted-display test: on-device PIN entry and approve/deny with a
  known relying-party string. A successful compile alone is not hardware
  validation.

Until those gates pass and a UF2 is built from the resulting branch, use only
the existing supported board/flavor combinations.
