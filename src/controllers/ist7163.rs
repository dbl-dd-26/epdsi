//! IST7163 (IST7163CA1-C) E-Paper Display Controller implementation.
//!
//! The IST7163 is Integrated Solutions Technology's 480 gate x 240 source driver for 4-colour
//! (black/white/yellow/red) E Ink panels. It drives `GDEM037F51`, sold by Waveshare as the
//! "3.7inch e-Paper (G)". Waveshare's `EPD_3in7g.cpp` and Good Display's
//! `A32-GDEM037F51-20230926` demo send the same init bytes, in the same order.
//!
//! Every init register except three was checked against `IST7163CA1-C-20231220.pdf` (Rev 004).
//! The **vendor-only** bytes, sourced from the two demos and absent from that specification, are
//! `0xE7`, `0xE3` and `0xE9`; the same trio appears in the JD79661 family's demos. The `0x03`
//! power-off sequence register has only its first parameter documented in this revision, so its
//! second and third bytes (`0x54`, `0x44`) are vendor-only as well.
//!
//! The register set is close to [`Jd7966xController`](crate::controllers::Jd7966xController)'s but
//! not the same: PWR takes six bytes here (two on the JD79661), BTST takes three (seven on the
//! JD79661), and the init runs two test-mode banks (`0xFF 0xA5` ... `0xFF 0xE3`). It is a
//! separate controller for that reason, and nothing here shows the two ICs are related.
//!
//! One spec inconsistency is carried as is. PSR `RES[1:0]` = `00` is listed as "240x120" while
//! TRES must not exceed the PSR line count, yet both vendor demos set PSR `0x0F` and TRES
//! 240 x 416. The vendor bytes are followed.
//!
//! Waveshare's fast init (`0xE0 0x02`, `0xE6 0x5B`, then `0xA5`) is not implemented. `0xE0` and
//! `0xE6` are the force-temperature registers, but `0x5B` (91) lies outside the documented
//! -25 to 60 C table, and register `0xA5` is not in the specification.
//!
//! The documented `AUTO` register (`0x17`, PON then DRF then POF) is unused, as in both demos.

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(feature = "blocking")]
use embedded_hal::spi::SpiDevice;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::spi::SpiDevice;

use embedded_hal::digital::{InputPin, OutputPin};
// `Wait` has no blocking equivalent, so it is aliased to `InputPin` under `blocking`, as in
// `jd7966x.rs`.
#[cfg(feature = "blocking")]
use embedded_hal::digital::InputPin as Wait;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::digital::Wait;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// IST7163 Command Definitions
pub mod cmd {
    /// Panel Setting (PSR)
    pub const PANEL_SETTING: u8 = 0x00;
    /// Power Setting (PWR), six parameters
    pub const POWER_SETTING: u8 = 0x01;
    /// Power OFF
    pub const POWER_OFF: u8 = 0x02;
    /// Power on/off Sequence Setting (POFS)
    pub const POWER_SEQUENCE: u8 = 0x03;
    /// Power ON
    pub const POWER_ON: u8 = 0x04;
    /// Booster Soft Start (BTST), three parameters
    pub const BOOSTER_SOFT_START: u8 = 0x06;
    /// Deep Sleep
    pub const DEEP_SLEEP: u8 = 0x07;
    /// Data Start Transmission, 2 bits per pixel, four pixels per byte
    pub const DATA_START_TRANSMISSION: u8 = 0x10;
    /// Display Refresh
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// PLL Control
    pub const PLL_CONTROL: u8 = 0x30;
    /// Temperature Sensor Enable
    pub const TEMPERATURE_SENSOR: u8 = 0x41;
    /// VCOM and Data Interval Setting (CDI)
    pub const CDI: u8 = 0x50;
    /// TCON Setting
    pub const TCON: u8 = 0x60;
    /// Resolution Setting (TRES)
    pub const RESOLUTION: u8 = 0x61;
    /// Gate/Source Start Setting (GSST)
    pub const GATE_SOURCE_START: u8 = 0x65;
    /// Test mode gate. `0xA5` enters, `0xE3` quits.
    pub const TEST_MODE: u8 = 0xFF;
    /// Test mode: charge pump timing (six parameters, 125 ns steps)
    pub const TEST_POWER_PWM: u8 = 0xEF;
}

/// Wait before the reset pulse, from Good Display's demo ("at least 100ms").
const PRE_RESET_DELAY_MS: u32 = 100;
/// Reset low and high time. The spec needs 1 ms low and 1 ms recovery. Good Display's demo uses
/// 50 ms; Waveshare's uses 2 ms low, which sits close to the limit.
const RESET_PULSE_MS: u32 = 50;
/// Wait before polling BUSY after PON and DRF, as in Waveshare's `ReadBusyH`. The IC drops
/// BUSY_N after the command, and polling too early can read the idle level.
const BUSY_SETTLE_MS: u32 = 100;
/// Wait between POF and DSLP, from Good Display's demo ("necessary, 100mS at least").
const SLEEP_SETTLE_MS: u32 = 100;

/// IST7163 Controller IC driver implementation.
#[derive(Debug, Clone, Copy)]
pub struct Ist7163Controller {
    width: u32,
    height: u32,
}

impl Ist7163Controller {
    /// Creates a new IST7163 controller for the given panel dimensions.
    ///
    /// `width` is rounded up to a multiple of 4 in the TRES register, which the specification
    /// requires (the low two bits of HRES are fixed at zero).
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Ist7163Controller
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin + Wait,
{
    type Error = EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>;

    async fn init_sequence<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // Hardware reset, then BUSY_N must be high before the first command (spec, reset
        // timing note 2). BUSY is active-low: busy while LOW.
        delay.delay_ms(PRE_RESET_DELAY_MS).await;
        bus.hard_reset(delay, RESET_PULSE_MS).await?;
        bus.wait_busy(false).await?;

        // PSR: param 1 is the reset default. Param 2 sets FOPT (no scan after the waveform) on
        // the `09` default.
        bus.send_command_with_data(cmd::PANEL_SETTING, &[0x0F, 0x29])
            .await?;

        // PWR: all three DC/DC internal, VGH/VGL +/-20 V, VDHR 6.4 V, VDH 15.0 V, VDL -4.0 V,
        // VDHR1 6.4 V.
        bus.send_command_with_data(cmd::POWER_SETTING, &[0x07, 0x00, 0x22, 0x78, 0x0A, 0x22])
            .await?;

        // POFS: only the first byte is documented (VGH/VGL off 40 ms, VDH/VDL off 20 ms).
        bus.send_command_with_data(cmd::POWER_SEQUENCE, &[0x10, 0x54, 0x44])
            .await?;

        // BTST: 40 ms soft start in each of the three phases.
        bus.send_command_with_data(cmd::BOOSTER_SOFT_START, &[0xC0, 0xC0, 0xC0])
            .await?;

        // PLL: Dyna set ("E Ink use only"), 12.5 Hz frame rate.
        bus.send_command_with_data(cmd::PLL_CONTROL, &[0x08])
            .await?;

        // TSE: internal temperature sensor on, no offset.
        bus.send_command_with_data(cmd::TEMPERATURE_SENSOR, &[0x00])
            .await?;

        // CDI: border follows Gray1 (white), 10 hsync interval. The reset default is `0x97`.
        bus.send_command_with_data(cmd::CDI, &[0x37]).await?;

        // TCON: reset default, 3 units (500 ns each) for both non-overlap periods.
        bus.send_command_with_data(cmd::TCON, &[0x02, 0x02]).await?;

        // TRES: HRES is a multiple of 4, VRES is the gate count.
        let hres = self.width.div_ceil(4) * 4;
        bus.send_command_with_data(
            cmd::RESOLUTION,
            &[
                ((hres >> 8) & 0xFF) as u8,
                (hres & 0xFF) as u8,
                ((self.height >> 8) & 0xFF) as u8,
                (self.height & 0xFF) as u8,
            ],
        )
        .await?;

        // GSST: first gate and first source active.
        bus.send_command_with_data(cmd::GATE_SOURCE_START, &[0x00, 0x00, 0x00, 0x00])
            .await?;

        // Vendor-only, not in the IST7163CA1-C specification (also in the JD79661 demos).
        bus.send_command_with_data(0xE7, &[0x1C]).await?;
        bus.send_command_with_data(0xE3, &[0x22]).await?;

        // Test-mode bank 1, all documented. EF are the charge pump timings used while powering
        // up, C3 makes the power PWM follow EF, DC/DD/DE set the clock pulse widths, FD/E8 the
        // VDH offset and VDL overshoot, DA the driving strength, C9 the GDR drive and A8 the
        // VDHR offset.
        bus.send_command_with_data(cmd::TEST_MODE, &[0xA5]).await?;
        bus.send_command_with_data(cmd::TEST_POWER_PWM, &[0x01, 0x1E, 0x0A, 0x1B, 0x0B, 0x17])
            .await?;
        bus.send_command_with_data(0xC3, &[0xFD]).await?;
        bus.send_command_with_data(0xDC, &[0x01]).await?;
        bus.send_command_with_data(0xDD, &[0x08]).await?;
        bus.send_command_with_data(0xDE, &[0x41]).await?;
        bus.send_command_with_data(0xFD, &[0x01]).await?;
        bus.send_command_with_data(0xE8, &[0x03]).await?;
        bus.send_command_with_data(0xDA, &[0x07]).await?;
        bus.send_command_with_data(0xC9, &[0x00]).await?;
        bus.send_command_with_data(0xA8, &[0x0F]).await?;
        bus.send_command_with_data(cmd::TEST_MODE, &[0xE3]).await?;

        // Vendor-only, not in the specification.
        bus.send_command_with_data(0xE9, &[0x01]).await?;

        // Power ON. The command is only accepted while BUSY_N is high, and BUSY_N returns high
        // when the power sequence is done.
        bus.send_command(cmd::POWER_ON).await?;
        delay.delay_ms(BUSY_SETTLE_MS).await;
        bus.wait_busy(false).await?;

        // Test-mode bank 2: the charge pump timings switch to the refresh values.
        bus.send_command_with_data(cmd::TEST_MODE, &[0xA5]).await?;
        bus.send_command_with_data(cmd::TEST_POWER_PWM, &[0x03, 0x1E, 0x0A, 0x1B, 0x0E, 0x15])
            .await?;
        bus.send_command_with_data(0xDC, &[0x01]).await?;
        bus.send_command_with_data(0xDD, &[0x08]).await?;
        bus.send_command_with_data(0xDE, &[0x41]).await?;
        bus.send_command_with_data(cmd::TEST_MODE, &[0xE3]).await?;

        Ok(())
    }

    async fn set_window(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x_start: u32,
        _y_start: u32,
        _x_end: u32,
        _y_end: u32,
    ) -> Result<(), Self::Error> {
        // Full-frame streaming only. Neither vendor demo has a windowed or partial update, and
        // the panel is full-refresh only.
        Ok(())
    }

    async fn set_cursor(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x: u32,
        _y: u32,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        // One 2 bpp plane on `0x10`; the channel is not used.
        bus.send_command_with_data(cmd::DATA_START_TRANSMISSION, data)
            .await
    }

    async fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::DATA_START_TRANSMISSION).await?;
        bus.send_data_repeated(byte, count).await
    }

    async fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // DRF takes one parameter (AC/DC VCOM select, 0 = follow the LUT). BUSY_N falls after
        // the command, so wait before polling, as the Waveshare demo does.
        bus.send_command_with_data(cmd::DISPLAY_REFRESH, &[0x00])
            .await?;
        delay.delay_ms(BUSY_SETTLE_MS).await;
        bus.wait_busy(false).await
    }

    async fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::POWER_OFF, &[0x00]).await?;
        bus.wait_busy(false).await?;
        // Good Display's demo marks this delay "necessary".
        delay.delay_ms(SLEEP_SETTLE_MS).await;
        // The IC leaves deep sleep only through a hardware reset.
        bus.send_command_with_data(cmd::DEEP_SLEEP, &[0xA5]).await
    }
}
