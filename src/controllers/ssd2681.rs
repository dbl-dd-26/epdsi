//! SSD2681 E-Paper Display Controller implementation.
//!
//! The SSD2681 is Solomon Systech's 4-color (black, white, yellow, red) controller for small
//! E Ink Spectra 3100 panels. It drives `GDEM0154F61H`.
//!
//! ## Provenance
//!
//! Every byte below comes from Good Display's own `GDEM0154F61H` package
//! (`AU-GDEM0154F61H-F-20260513`, `Display_EPD_W21.cpp`), checked against `SSD2681.pdf` (rev 1.0,
//! Oct 2025), and bench verified on 17 Oct 2026 (see [`GDEM0154F61H`](crate::panels::GDEM0154F61H)).
//! The datasheet documents `0xE0` (CCSET, `TSFIX`), `0xE6` (TSSET), `0x12` (DRF),
//! `0x02` (POF), `0x07` (deep sleep) and `0x10` (DTM). It does not document `0xE9`, `0xEF` or
//! `0xF6`, which the demo sends, so those three are vendor registers sourced from the demo only.
//!
//! ## Why this is not a [`Jd7966xVariant`](super::jd7966x::Jd7966xVariant)
//!
//! The plain init is the same two writes the JD79676 profile uses (`0xE9 0x01`, then `PON`),
//! and refresh and sleep match too. The ICs come from different vendors, though, and the fast
//! init here adds `0xEF`/`0xF6` writes and a different `0xE6` value. A separate small controller
//! keeps the bench-verified JD7966x core untouched.
//!
//! ## Things to know
//!
//! - **BUSY is active-LOW** (idle high). Pass a pull-up input.
//! - **One RAM plane, 2 bits per pixel**: four pixels per byte, the leftmost pixel in bits 7:6,
//!   written through `0x10` only. Codes are black `00`, white `01`, yellow `10`, red `11`. The
//!   `ColorChannel` argument is ignored.
//! - **Full refresh only**: the demo has no partial mode and re-initialises before every update.
//! - **Windows are not used**: `set_window` and `set_cursor` do nothing, and the whole frame is
//!   streamed.

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(feature = "blocking")]
use embedded_hal::spi::SpiDevice;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::spi::SpiDevice;

#[cfg(feature = "blocking")]
use embedded_hal::digital::InputPin as Wait;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::digital::Wait;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// SSD2681 command definitions.
pub mod cmd {
    /// Power OFF
    pub const POWER_OFF: u8 = 0x02;
    /// Power ON
    pub const POWER_ON: u8 = 0x04;
    /// Deep Sleep
    pub const DEEP_SLEEP: u8 = 0x07;
    /// Data Start Transmission (the single 2 bpp RAM plane)
    pub const DATA_START_TRANSMISSION: u8 = 0x10;
    /// Display Refresh
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// Vendor register. Not in the SSD2681 datasheet; the demo sends `0x01` after reset.
    pub const VENDOR_E9: u8 = 0xE9;
    /// Vendor register. Not in the datasheet; used by the demo's fast init only.
    pub const VENDOR_EF: u8 = 0xEF;
    /// Vendor register. Not in the datasheet; used by the demo's fast init only.
    pub const VENDOR_F6: u8 = 0xF6;
    /// CCSET, temperature input selection (`TSFIX` is bit 1)
    pub const CCSET: u8 = 0xE0;
    /// TSSET, manual input temperature value
    pub const TSSET: u8 = 0xE6;
    /// Auto sequence command. The datasheet lists `0x17` with `A5h`/`A7h`; the demo sends a bare
    /// `0xA5` as a command, which is not documented.
    pub const BARE_A5: u8 = 0xA5;
}

/// Init profile for [`Ssd2681Controller`], read by `init_sequence`.
///
/// This enum is `#[non_exhaustive]`: a new mode is added in a patch release, so a `match` outside
/// this crate needs a wildcard arm. Constructing a variant is unaffected.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Ssd2681RefreshMode {
    /// Good Display's `EPD_init()`: reset, `0xE9 0x01`, `PON`.
    #[default]
    Full,
    /// Good Display's `EPD_init_Fast()`: `0xE9 0x01`, `0xEF 0x01`, `0xF6 0x24`, `0xEF 0x00`,
    /// `0xE0 0x02` (`TSFIX`), `0xE6 0x5C`, a bare `0xA5`, then `PON`.
    ///
    /// `0xE9`, `0xEF`, `0xF6` and the bare `0xA5` are not in the datasheet. `TSFIX` plus `0xE6`
    /// forces the temperature to 92 degC, which most likely selects a shorter waveform. That is
    /// an inference from the datasheet's description of `TSSET`, not a measurement.
    Fast,
}

/// SSD2681 controller (`GDEM0154F61H`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Ssd2681Controller {
    refresh_mode: Ssd2681RefreshMode,
}

impl Ssd2681Controller {
    /// Creates a new SSD2681 controller using the plain init.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the refresh mode (builder method). See [`Ssd2681RefreshMode`].
    pub fn with_refresh_mode(mut self, mode: Ssd2681RefreshMode) -> Self {
        self.refresh_mode = mode;
        self
    }

    /// Sets the refresh mode. Takes effect at the next `init`. See [`Ssd2681RefreshMode`].
    pub fn set_refresh_mode(&mut self, mode: Ssd2681RefreshMode) {
        self.refresh_mode = mode;
    }

    /// Returns the configured refresh mode.
    pub fn refresh_mode(&self) -> Ssd2681RefreshMode {
        self.refresh_mode
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Ssd2681Controller
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
        let fast = self.refresh_mode == Ssd2681RefreshMode::Fast;
        // Demo: 100 ms (plain) or 20 ms (fast) before reset, then 50 ms low and 50 ms high.
        // `hard_reset` uses one duration for both phases.
        delay.delay_ms(if fast { 20 } else { 100 }).await;
        bus.hard_reset(delay, 50).await?;

        bus.send_command_with_data(cmd::VENDOR_E9, &[0x01]).await?;
        if fast {
            bus.send_command_with_data(cmd::VENDOR_EF, &[0x01]).await?;
            bus.send_command_with_data(cmd::VENDOR_F6, &[0x24]).await?;
            bus.send_command_with_data(cmd::VENDOR_EF, &[0x00]).await?;
            bus.send_command_with_data(cmd::CCSET, &[0x02]).await?;
            bus.send_command_with_data(cmd::TSSET, &[0x5C]).await?;
            bus.send_command(cmd::BARE_A5).await?;
            bus.wait_busy_with_delay(delay, false).await?;
        }

        bus.send_command(cmd::POWER_ON).await?;
        bus.wait_busy_with_delay(delay, false).await
    }

    async fn set_window(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x_start: u32,
        _y_start: u32,
        _x_end: u32,
        _y_end: u32,
    ) -> Result<(), Self::Error> {
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
        bus.send_command_with_data(cmd::DISPLAY_REFRESH, &[0x00])
            .await?;
        bus.wait_busy_with_delay(delay, false).await
    }

    async fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::POWER_OFF, &[0x00]).await?;
        bus.wait_busy_with_delay(delay, false).await?;
        bus.send_command_with_data(cmd::DEEP_SLEEP, &[0xA5]).await
    }
}
