//! UC8151D E-Paper Display Controller implementation.
//!
//! The UC8151D is UltraChip's small-panel monochrome controller. It shares most of its command
//! core with the UC8253 and the IL0373, but its panels need a different init profile, so it has
//! its own controller instead of a third [`Uc8253Variant`](super::uc8253::Uc8253Variant).
//!
//! ## Provenance
//!
//! Every byte below comes from Good Display's own `GDEW0215T12` package
//! (`AU-GDEW0215T12-P-20230926`, `Display_EPD_W21.cpp`), checked against `GDEW0215T12-new.pdf`
//! (rev 1.0, 2023/09/19) and the UltraChip `UC8151D` datasheet (rev B0.6). Bench verified on
//! 15 Oct 2026 with the blocking API on an XIAO ESP32-C3 on the Seeed ePaper Driver Board (full
//! refresh, whole-frame partial and windowed partial updates all rendered), and the same four
//! screens ran to completion on an Adafruit Feather RP2040 ThinkInk (blocking), and on a Pico 2
//! with the blocking and the async (Embassy) APIs.
//!
//! ## Two init profiles, selected before `init`
//!
//! The vendor demo uses a different init for full and partial updates: the full profile runs the
//! panel's OTP waveform, the partial profile uploads its own LUT through registers `0x20` to
//! `0x24`. [`Uc8151dRefreshMode`] selects the profile and is read by `init_sequence`. Changing it
//! afterwards has no effect until the display is initialised again, so set it with
//! [`Uc8151dController::with_refresh_mode`] before the first `init`, or call `init` again after
//! [`Uc8151dController::set_refresh_mode`].
//!
//! ## Things to know
//!
//! - **BUSY is active-LOW** (idle high). Pass a pull-up input.
//! - **RAM planes**: `ColorChannel::BlackWhite` writes `0x13` (new data), and
//!   `ColorChannel::RedYellow` writes `0x10` (old data, the differential base). White is `1`.
//!   Prime the old plane to `0xFF` before the first full refresh, as the vendor demo does.
//! - **Panel setting second byte `0x0D`** is not in any datasheet this crate was checked against.
//!   The vendor demo sends it (`1F 0D` full, `BF 0D` partial) and so does this driver, and the
//!   panel works with it. Two other sources send a one-byte form, so it is probably ignored, but
//!   the one-byte form has not been tried.
//! - **Partial window**: the vertical end line is sent inclusive, as the datasheet defines it. The
//!   vendor demo sends the end line minus one. The inclusive form rendered a complete band on the
//!   bench; the demo's form has not been tried.

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;
#[cfg(feature = "blocking")]
use embedded_hal::spi::SpiDevice;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::spi::SpiDevice;

use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "blocking")]
use embedded_hal::digital::InputPin as Wait;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::digital::Wait;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// UC8151D command definitions.
pub mod cmd {
    /// Panel Setting
    pub const PANEL_SETTING: u8 = 0x00;
    /// Power Setting
    pub const POWER_SETTING: u8 = 0x01;
    /// Power OFF
    pub const POWER_OFF: u8 = 0x02;
    /// Power ON
    pub const POWER_ON: u8 = 0x04;
    /// Booster Soft Start
    pub const BOOSTER_SOFT_START: u8 = 0x06;
    /// Deep Sleep
    pub const DEEP_SLEEP: u8 = 0x07;
    /// Write the "old" (previous image) data plane
    pub const WRITE_OLD_DATA: u8 = 0x10;
    /// Display Refresh
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// Write the "new" (current image) data plane
    pub const WRITE_NEW_DATA: u8 = 0x13;
    /// LUT for VCOM (44 bytes)
    pub const LUT_VCOM: u8 = 0x20;
    /// LUT for white to white (42 bytes)
    pub const LUT_WW: u8 = 0x21;
    /// LUT for black to white (42 bytes)
    pub const LUT_BW: u8 = 0x22;
    /// LUT for white to black (42 bytes)
    pub const LUT_WB: u8 = 0x23;
    /// LUT for black to black (42 bytes)
    pub const LUT_BB: u8 = 0x24;
    /// PLL control
    pub const PLL_CONTROL: u8 = 0x30;
    /// VCOM and Data Interval Setting (CDI)
    pub const CDI: u8 = 0x50;
    /// Resolution Setting (TRES): HRES byte, then VRES high and low bytes
    pub const RESOLUTION: u8 = 0x61;
    /// VCOM DC setting
    pub const VCOM_DC: u8 = 0x82;
    /// Partial Window
    pub const PARTIAL_WINDOW: u8 = 0x90;
    /// Partial In
    pub const PARTIAL_IN: u8 = 0x91;
    /// Partial Out
    pub const PARTIAL_OUT: u8 = 0x92;
}

/// Builds one 6-byte-per-row LUT table whose only non-zero row is the first, as the vendor demo
/// does: `[first, 0x19, 0x01, 0x00, 0x00, 0x01]` followed by zeros.
const fn lut<const N: usize>(first: u8) -> [u8; N] {
    let mut table = [0u8; N];
    table[0] = first;
    table[1] = 0x19;
    table[2] = 0x01;
    table[5] = 0x01;
    table
}

const LUT_VCOM_PARTIAL: [u8; 44] = lut::<44>(0x00);
const LUT_WW_PARTIAL: [u8; 42] = lut::<42>(0x00);
const LUT_BW_PARTIAL: [u8; 42] = lut::<42>(0x80);
const LUT_WB_PARTIAL: [u8; 42] = lut::<42>(0x40);
const LUT_BB_PARTIAL: [u8; 42] = lut::<42>(0x00);

/// Number of reset pulses the vendor demo issues before configuring the panel.
const RESET_PULSES: usize = 3;

/// How long to wait for BUSY to assert after `DISPLAY_REFRESH`, before giving up and waiting for
/// completion anyway.
///
/// BUSY goes active a short while after the command, so polling it straight away can read "idle"
/// while the refresh has only just begun. The refresh would then report done in a few
/// milliseconds, and the next command (a reset, in the partial profile) would cut it off.
const BUSY_ASSERT_TIMEOUT_MS: u32 = 500;

/// Display refresh profile for the UC8151D controller.
///
/// The profile is chosen at `init`, not at refresh time, because the two profiles program
/// different registers. See the module documentation.
///
/// This enum is `#[non_exhaustive]`: a new profile is added in a patch release, so a `match`
/// outside this crate needs a wildcard arm. Constructing a variant is unaffected.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Uc8151dRefreshMode {
    /// Full update on the panel's OTP waveform (PSR `0x1F`, CDI `0x97`).
    #[default]
    Full,
    /// Partial update on a register-uploaded LUT (PSR `0xBF`, PLL `0x3C`, VCOM DC `0x12`).
    Partial,
}

/// UC8151D controller IC driver configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uc8151dController {
    width: u32,
    height: u32,
    refresh_mode: Uc8151dRefreshMode,
    /// Window requested through `set_window`, applied lazily around RAM writes and refreshes.
    window: Option<(u32, u32, u32, u32)>,
    /// Window currently programmed into the chip, `Some` while it is in partial mode.
    open: Option<(u32, u32, u32, u32)>,
}

impl Uc8151dController {
    /// Creates a new UC8151D controller for a panel of `width` by `height` pixels.
    ///
    /// `width` is the source (horizontal RAM) axis and `height` the gate axis, as the panel's
    /// resolution setting expects them. The default profile is [`Uc8151dRefreshMode::Full`].
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            refresh_mode: Uc8151dRefreshMode::default(),
            window: None,
            open: None,
        }
    }

    /// Sets the refresh profile (builder method). Applies at the next `init`.
    pub fn with_refresh_mode(mut self, mode: Uc8151dRefreshMode) -> Self {
        self.refresh_mode = mode;
        self
    }

    /// Sets the refresh profile. Applies at the next `init`, so initialise the display again
    /// after changing it.
    pub fn set_refresh_mode(&mut self, mode: Uc8151dRefreshMode) {
        self.refresh_mode = mode;
    }

    /// Returns the configured refresh profile.
    pub fn refresh_mode(&self) -> Uc8151dRefreshMode {
        self.refresh_mode
    }

    /// Clears any recorded partial window, returning to full-frame addressing.
    ///
    /// The chip leaves partial mode (`PARTIAL_OUT`) the next time it is written to or refreshed.
    pub fn clear_window(&mut self) {
        self.window = None;
    }

    /// Maps a colour channel onto the RAM plane command.
    ///
    /// Black/White goes to the "new" plane `0x13`, and the second channel to the "old" plane
    /// `0x10`, which is the differential base for partial updates.
    fn plane_command(channel: ColorChannel) -> u8 {
        if matches!(channel, ColorChannel::BlackWhite) {
            cmd::WRITE_NEW_DATA
        } else {
            cmd::WRITE_OLD_DATA
        }
    }

    /// Encodes the 7-byte Partial Window payload for `(x_start, y_start, x_end, y_end)`.
    ///
    /// The horizontal bounds are source banks of 8 (`HRST[7:3]` and `HRED[7:3]`), so the start
    /// rounds down and the end rounds up to the bank's last pixel. The vertical bounds are
    /// inclusive 9-bit line numbers. The last byte is `PT_SCAN`: the vendor demo sends `0x28`,
    /// whose bit 0 is clear, so gates scan only inside the window.
    fn window_payload(window: (u32, u32, u32, u32)) -> [u8; 7] {
        let (x_start, y_start, x_end, y_end) = window;
        let y = y_start as u16;
        let ye = y_end as u16;
        [
            (x_start & 0xF8) as u8,
            (x_end | 0x07) as u8,
            (y >> 8) as u8,
            (y & 0xFF) as u8,
            (ye >> 8) as u8,
            (ye & 0xFF) as u8,
            0x28,
        ]
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl Uc8151dController {
    /// Brings the chip's partial-window state in line with the recorded window.
    ///
    /// A window covering the whole panel counts as no window. Sends `PARTIAL_IN` only when the chip is not already in partial mode, and re-sends
    /// `PARTIAL_WINDOW` when the window changed. Leaves partial mode with `PARTIAL_OUT` when the
    /// window has been cleared.
    #[allow(clippy::type_complexity)]
    async fn sync_window<SPI, DC, RST, BUSY>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
    {
        // A window that covers the whole panel is plain full-frame addressing. The vendor's full
        // update never enters partial mode, so do not send `PARTIAL_IN` for it: partial mode
        // changes how the chip scans and may not run the OTP waveform.
        let full = (0, 0, self.width - 1, self.height - 1);
        let wanted_window = self.window.filter(|w| *w != full);

        match (wanted_window, self.open) {
            (Some(wanted), Some(current)) if wanted == current => Ok(()),
            (Some(wanted), open) => {
                if open.is_none() {
                    bus.send_command(cmd::PARTIAL_IN).await?;
                }
                bus.send_command_with_data(cmd::PARTIAL_WINDOW, &Self::window_payload(wanted))
                    .await?;
                self.open = Some(wanted);
                Ok(())
            }
            (None, Some(_)) => {
                bus.send_command(cmd::PARTIAL_OUT).await?;
                self.open = None;
                Ok(())
            }
            (None, None) => Ok(()),
        }
    }

    /// Resets the controller the way the vendor demo does: three low and high pulses, then a
    /// wait for BUSY to read idle.
    #[allow(clippy::type_complexity)]
    async fn reset<SPI, DC, RST, BUSY, DELAY>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin + Wait,
        DELAY: DelayNs,
    {
        for _ in 0..RESET_PULSES {
            bus.hard_reset(delay, 10).await?;
        }
        // A reset leaves the chip outside partial mode.
        self.open = None;
        bus.wait_busy_with_delay(delay, false).await
    }

    /// Sends the resolution setting: HRES byte, then VRES high and low.
    #[allow(clippy::type_complexity)]
    async fn send_resolution<SPI, DC, RST, BUSY>(
        &self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
    {
        bus.send_command_with_data(
            cmd::RESOLUTION,
            &[
                self.width as u8,
                (self.height >> 8) as u8,
                (self.height & 0xFF) as u8,
            ],
        )
        .await
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Uc8151dController
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
        self.reset(bus, delay).await?;

        match self.refresh_mode {
            Uc8151dRefreshMode::Full => {
                // LUT from OTP, KW mode. The second byte is the vendor demo's, see module docs.
                bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1F, 0x0D])
                    .await?;
                self.send_resolution(bus).await?;
                bus.send_command(cmd::POWER_ON).await?;
                bus.wait_busy_with_delay(delay, false).await?;
                bus.send_command_with_data(cmd::CDI, &[0x97]).await?;
            }
            Uc8151dRefreshMode::Partial => {
                bus.send_command_with_data(cmd::POWER_SETTING, &[0x03, 0x00, 0x2B, 0x2B, 0x03])
                    .await?;
                bus.send_command_with_data(cmd::BOOSTER_SOFT_START, &[0x17, 0x17, 0x17])
                    .await?;
                // `0xBF` has REG set, so the LUT comes from the registers uploaded below.
                bus.send_command_with_data(cmd::PANEL_SETTING, &[0xBF, 0x0D])
                    .await?;
                bus.send_command_with_data(cmd::PLL_CONTROL, &[0x3C])
                    .await?;
                self.send_resolution(bus).await?;
                bus.send_command_with_data(cmd::VCOM_DC, &[0x12]).await?;
                bus.send_command_with_data(cmd::LUT_VCOM, &LUT_VCOM_PARTIAL)
                    .await?;
                bus.send_command_with_data(cmd::LUT_WW, &LUT_WW_PARTIAL)
                    .await?;
                bus.send_command_with_data(cmd::LUT_BW, &LUT_BW_PARTIAL)
                    .await?;
                bus.send_command_with_data(cmd::LUT_WB, &LUT_WB_PARTIAL)
                    .await?;
                bus.send_command_with_data(cmd::LUT_BB, &LUT_BB_PARTIAL)
                    .await?;
                bus.send_command(cmd::POWER_ON).await?;
                bus.wait_busy_with_delay(delay, false).await?;
            }
        }
        Ok(())
    }

    /// Records the window without emitting any command.
    ///
    /// The chip is put into partial mode lazily, right before the next RAM write or refresh, so
    /// the stream matches the vendor demo's: `PARTIAL_IN`, `PARTIAL_WINDOW`, old plane, new
    /// plane, refresh. Use [`Uc8151dController::clear_window`] to return to full-frame addressing.
    async fn set_window(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), Self::Error> {
        self.window = Some((x_start, y_start, x_end, y_end));
        Ok(())
    }

    async fn set_cursor(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x: u32,
        _y: u32,
    ) -> Result<(), Self::Error> {
        // The UC8151D has no separate RAM cursor; the window sets where data lands.
        Ok(())
    }

    async fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        self.sync_window(bus).await?;
        bus.send_command_with_data(Self::plane_command(channel), data)
            .await
    }

    async fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        self.sync_window(bus).await?;
        bus.send_command(Self::plane_command(channel)).await?;
        bus.send_data_repeated(byte, count).await
    }

    async fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.sync_window(bus).await?;
        bus.send_command(cmd::DISPLAY_REFRESH).await?;
        // The vendor demo says a gap before polling BUSY is necessary, 200 us at least.
        delay.delay_ms(1).await;
        // Wait for BUSY to actually assert before waiting for it to clear. The result is not
        // checked: a panel that never asserts reads idle below and returns at once, the same
        // behaviour as the UC8253 controller.
        bus.wait_busy_assert(delay, false, BUSY_ASSERT_TIMEOUT_MS)
            .await?;
        bus.wait_busy_with_delay(delay, false).await?;

        if self.open.is_some() {
            bus.send_command(cmd::PARTIAL_OUT).await?;
            self.open = None;
        }
        Ok(())
    }

    async fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::CDI, &[0xF7]).await?;
        bus.send_command(cmd::POWER_OFF).await?;
        bus.wait_busy_with_delay(delay, false).await?;
        // The vendor demo marks this gap as necessary, 100 ms at least, before deep sleep.
        delay.delay_ms(100).await;
        bus.send_command_with_data(cmd::DEEP_SLEEP, &[0xA5]).await
    }
}
