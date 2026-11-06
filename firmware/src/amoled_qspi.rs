// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors

//! Four-lane QSPI transport for the Waveshare RP2350-Touch-AMOLED-1.8.
//!
//! This is intentionally separate from `display_panel.rs`: that file is a
//! one-data-lane ST7789 transport with a D/C pin. The SH8601 has no D/C pin;
//! register transactions are encoded on DIO0 and pixel payloads are pushed to
//! all four data lanes.

use embassy_rp::dma::{Channel, ChannelInstance};
use embassy_rp::interrupt::typelevel::Binding;
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{
    Common, Config as PioConfig, Direction, FifoJoin, LoadedProgram, PioPin,
    ShiftDirection, StateMachine,
};
use embassy_rp::{Peri, dma};

/// Clocked output for PIO0 SM0, the four-lane data program from Waveshare's
/// `qspi.pio`. GPIO pins must be contiguous: DIO0..DIO3.
pub(crate) struct PioQspiTx {
    sm: StateMachine<'static, PIO0, 0>,
    dma: Channel<'static>,
    _program: LoadedProgram<'static, PIO0>,
}

impl PioQspiTx {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        common: &mut Common<'static, PIO0>,
        mut sm: StateMachine<'static, PIO0, 0>,
        clk: Peri<'static, impl PioPin>,
        dio0: Peri<'static, impl PioPin>,
        dio1: Peri<'static, impl PioPin>,
        dio2: Peri<'static, impl PioPin>,
        dio3: Peri<'static, impl PioPin>,
        dma: Peri<'static, DMA_CH0>,
        irq: impl Binding<<DMA_CH0 as ChannelInstance>::Interrupt, dma::InterruptHandler<DMA_CH0>>
        + 'static,
    ) -> Self {
        let program = embassy_rp::pio::program::pio_asm!(
            ".side_set 1 opt",
            ".wrap_target",
            "out pins, 4 side 0",
            "nop side 1",
            ".wrap",
        );
        let program = common.load_program(&program.program);
        let mut clk = common.make_pio_pin(clk);
        let dio0 = common.make_pio_pin(dio0);
        let dio1 = common.make_pio_pin(dio1);
        let dio2 = common.make_pio_pin(dio2);
        let dio3 = common.make_pio_pin(dio3);
        clk.set_output_inversion(false);

        let mut config = PioConfig::default();
        config.use_program(&program, &[&clk]);
        config.set_out_pins(&[&dio0, &dio1, &dio2, &dio3]);
        config.shift_out.auto_fill = true;
        config.shift_out.direction = ShiftDirection::Left;
        config.shift_out.threshold = 8;
        config.fifo_join = FifoJoin::TxOnly;
        config.clock_divider = 1u8.into();

        // Match the vendor transport: one 4-bit transfer every two system-clock
        // cycles. At the standard 150 MHz RP2350 clock this produces a 75 MHz
        // QSPI clock; do not silently overclock the MCU to match the old LCD path.
        assert_eq!(
            embassy_rp::clocks::clk_sys_freq(),
            150_000_000,
            "the AMOLED QSPI transport expects a 150 MHz system clock"
        );

        sm.set_config(&config);
        sm.set_pins(
            embassy_rp::gpio::Level::Low,
            &[&clk, &dio0, &dio1, &dio2, &dio3],
        );
        sm.set_pin_dirs(
            Direction::Out,
            &[&clk, &dio0, &dio1, &dio2, &dio3],
        );
        sm.set_enable(true);

        Self {
            sm,
            dma: Channel::new(dma, irq),
            _program: program,
        }
    }

    /// Send one SH8601 command/address byte over DIO0 using the exact four-word
    /// expansion used by Waveshare's QSPI command writer.
    pub(crate) fn write_command_byte(&mut self, byte: u8) {
        for word in rsk_ui::qspi::encode_sh8601_command_byte(byte) {
            while !self.sm.tx().try_push(word) {}
            // Clear an earlier empty-FIFO latch before flush observes this word.
            self.sm.tx().stalled();
        }
    }

    /// Push raw RGB565 bytes to the four-lane output using DMA. Pixel byte order
    /// is supplied by the caller; SH8601 is initialized for RGB565.
    pub(crate) async fn write_pixels(&mut self, bytes: &[u8]) {
        self.sm.tx().stalled();
        let mut dma = self.dma.reborrow();
        self.sm.tx().dma_push(&mut dma, bytes, false).await;
    }

    /// Wait until the PIO FIFO is empty and the final clock edge has completed.
    pub(crate) fn flush(&mut self) {
        while !self.sm.tx().empty() {}
        while !self.sm.tx().stalled() {}
    }
}
