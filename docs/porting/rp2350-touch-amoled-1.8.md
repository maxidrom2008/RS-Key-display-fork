<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# Waveshare RP2350-Touch-AMOLED-1.8 port

**Status: bring-up work in progress. This board is not supported by a release image yet.**

This document records the hardware contract for a future `display-amoled` firmware
flavor. Do not use the existing `display` image for this board: that image targets
the ST7789/CST328 hardware on the RP2350-Touch-LCD-2.8.

## Board identity

- MCU: RP2350A
- External QSPI flash: W25Q128, 16 MiB
- Display: 1.8-inch AMOLED, 368 x 448, SH8601
- Display host interface: 4-lane QSPI
- Touch controller: FT3168, I2C, 7-bit address 0x38

Primary source: Waveshare's RP2350-Touch-AMOLED-1.8 product page and wiki.
Waveshare's Pico SDK example archive contains the corresponding SH8601 QSPI
and FT3168 example drivers. The upstream sample headers identify the panel
signals and touch register map; retain their permissive license notice if any
code is adapted.

## Pin assignment

| Signal | RP2350 GPIO | Notes |
|---|---:|---|
| LCD_CS | 9 | Active-low panel chip select |
| QSPI_SCLK | 10 | Panel serial clock |
| QSPI_SIO0..3 | 11..14 | Four data lanes, contiguous |
| LCD_RESET | 15 | Panel reset, active-low |
| LCD_TE | 16 | Tearing-effect signal; optional unless synchronization is added |
| DSI_PWR_EN | 17 | Must be driven high to enable panel power |
| TP_INT | 4 | Touch interrupt; polling can be used initially |
| TP_RESET | 5 | Touch reset, active-low |
| TP_SDA | 6 | I2C data |
| TP_SCL | 7 | I2C clock |

The panel has no separate PWM backlight pin. Brightness is a SH8601 command
(`0x51` in the vendor sample), so the existing LCD backlight-PWM path must not
be reused. Panel power enable (GPIO17) is a different function from brightness.

## Port architecture

1. Keep the trusted-display state machine and policy in `rsk-display`; it is
   hardware independent and already owns the security-sensitive approve/deny
   and PIN flows.
2. Add a distinct RP2350 AMOLED hardware backend. The existing
   `display_panel.rs` is a single-lane ST7789 transport and is not a QSPI
   implementation.
3. Implement SH8601 command/register writes and QSPI pixel streaming using
   RP2350 PIO + DMA. Commands/window setup and pixel payloads have different
   transfer phases; keep CS asserted over each command/data transaction.
4. Implement FT3168 I2C reads (address 0x38; finger count at 0x02, first
   contact coordinates at 0x03..0x06) and map physical touch coordinates to
   the UI coordinate space. Clamp invalid/out-of-range contacts.
5. Preserve the current 240 x 320 logical UI initially and scale/translate
   rendering and touch coordinates consistently, or undertake a separately
   tested responsive-layout change for 368 x 448. Never scale only the pixels:
   hit-testing must use the exact inverse transform.
6. Treat GPIO17 power-up, GPIO15 reset timing, SH8601 sleep-out/display-on,
   pixel format, orientation, brightness, and QSPI clock as explicit board
   initialization settings. Keep panel power off until the driver is ready.
7. Add a board-specific build flavor and compile-time pin-collision checks.
   The existing LCD defaults (GPIO13/14/15/16/17 and CST328/CST3530) must remain
   unchanged for the RP2350-Touch-LCD-2.8.

## Validation gates

- Host tests for FT3168 report parsing, coordinate transforms, clipping, and
  QSPI byte/nibble packing.
- Firmware compile for both the existing LCD flavor and the new AMOLED flavor.
- Hardware smoke test: panel reset/init, color bars, full-frame DMA transfer,
  partial-window transfer, FT3168 touch coordinate readout, and repeated wake/
  reset cycles.
- End-to-end trusted-display test: on-device PIN entry and approve/deny with a
  known relying-party string. A successful compile alone is not hardware
  validation.

Until those gates pass and a UF2 is built from the resulting branch, use only
the existing supported board/flavor combinations.
