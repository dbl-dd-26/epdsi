//! UC8253 E-Paper Display Controller implementation.

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

use crate::bus::{EpdBusError, SpiBusWrapper, ValidationError};
use crate::traits::{ColorChannel, EpdController};

const LUT_R20_DU: [u8; 42] = // VCOM 10×8=80bytes
    [
        0x01, 0x0f, 0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
const LUT_R21_DU: [u8; 42] = // W2W 7×8=56bytes
    [
        0x01, 0x0f, 0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
const LUT_R22_DU: [u8; 42] = // B2W 10×8=80bytes
    [
        0x01, 0x8f, 0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
const LUT_R23_DU: [u8; 42] = // W2B 10×8=80bytes
    [
        0x01, 0x4f, 0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
const LUT_R24_DU: [u8; 42] = // B2B 10×8=80bytes
    [
        0x01, 0x0f, 0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
const LUT_R20_GC: [u8; 42] = // VCOM 10×8=80bytes
    [
        0x01, 0x0f, 0x0f, 0x0f, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
const LUT_R21_GC: [u8; 42] = // W2W 7×8=56bytes
    [
        0x01, 0x4f, 0x8f, 0x0f, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
const LUT_R22_GC: [u8; 42] = // B2W 10×8=80bytes
    [
        0x01, 0x0f, 0x8f, 0x0f, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
const LUT_R23_GC: [u8; 42] = // W2B 10×8=80bytes
    [
        0x01, 0x4f, 0x8f, 0x4f, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
const LUT_R24_GC: [u8; 42] = // B2B 10×8=80bytes
    [
        0x01, 0x0f, 0x8f, 0x4f, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

const LUTS_DU: Luts<'static> = Luts {
    vcom: &LUT_R20_DU,
    ww: &LUT_R21_DU,
    kw: &LUT_R22_DU,
    wk: &LUT_R23_DU,
    kk: &LUT_R24_DU,
};

const LUTS_GC: Luts<'static> = Luts {
    vcom: &LUT_R20_GC,
    ww: &LUT_R21_GC,
    kw: &LUT_R22_GC,
    wk: &LUT_R23_GC,
    kk: &LUT_R24_GC,
};

/// Bytes in one `Sfe213zhbw` LUT register load: 6 groups of 7 bytes, the KW-mode group count.
const LUT_LEN: usize = 42;

/// LUT phase byte level selections (bits 7:6). On the `SFE213ZHBW` glass VSH drives a pixel
/// towards black and VSL towards white, as in the vendor DU/GC tables.
const LEVEL_GND: u8 = 0b00 << 6;
const LEVEL_VSH: u8 = 0b01 << 6;
const LEVEL_VSL: u8 = 0b10 << 6;

/// A phase's frame count is 6 bits wide.
const MAX_PHASE_FRAMES: u8 = 0x3F;

/// PLL (`0x30`) frame rate codes: the panel's 50 Hz from init, and the 100 Hz the grayscale
/// refreshes run at for finer pulse steps.
const PLL_50HZ: u8 = 0x09;
const PLL_100HZ: u8 = 0x13;

/// Frame length while grayscale refreshes run at [`PLL_100HZ`].
const GRAYSCALE_FRAME_MS: u16 = 10;

/// Default drive time to the darkest gray level, for both [`Uc8253RefreshMode::Gray4`] and
/// [`Uc8253Controller::draw_grayscale`]. Measured with one-way pulses: on `SFE213ZHBW` glass a
/// white pixel reaches full black within about 120–180 ms of VSH. The balanced pulses' white
/// half makes the black half less effective, so the best value for them may be higher.
const DEFAULT_GRAY_BLACK_MS: u16 = 160;

/// Default push time each way for [`Uc8253RefreshMode::Balanced`]: a little past the measured
/// 120–180 ms to full black, since a BW pixel must fully switch rather than land on a level.
const DEFAULT_BALANCED_MS: u16 = 200;

/// Most gray levels [`Uc8253Controller::draw_grayscale`] accepts.
pub const MAX_GRAY_LEVELS: u8 = 16;

/// Longest raster row the UC8253 can address: 240 sources.
const MAX_ROW_BYTES: usize = 240 / 8;

/// One full set of `Sfe213zhbw` waveform LUTs, named by the transition each one drives
/// (`ww` = white→white, `kw` = black→white, `wk` = white→black, `kk` = black→black).
struct Luts<'a> {
    vcom: &'a [u8; LUT_LEN],
    ww: &'a [u8; LUT_LEN],
    kw: &'a [u8; LUT_LEN],
    wk: &'a [u8; LUT_LEN],
    kk: &'a [u8; LUT_LEN],
}

/// Builds a LUT whose first group runs `phases` (level, frames) once, with every other group empty.
fn single_group_lut(phases: [(u8, u8); 4]) -> [u8; LUT_LEN] {
    let mut lut = [0u8; LUT_LEN];
    // Group and state repeat counts as in the vendor tables.
    lut[0] = 0x01;
    for (slot, (level, frames)) in lut[1..5].iter_mut().zip(phases) {
        *slot = if frames == 0 {
            0
        } else {
            level | frames.min(MAX_PHASE_FRAMES)
        };
    }
    lut[5] = 0x01;
    lut[6] = 0x01;
    lut
}

/// VCOM LUT holding VCOM_DC for the longest of `luts`, phase by phase.
fn vcom_lut_for(luts: [&[u8; LUT_LEN]; 4]) -> [u8; LUT_LEN] {
    let mut phases = [(LEVEL_GND, 0u8); 4];
    for (index, phase) in phases.iter_mut().enumerate() {
        phase.1 = luts
            .iter()
            .map(|lut| lut[1 + index] & MAX_PHASE_FRAMES)
            .max()
            .unwrap_or(0);
    }
    single_group_lut(phases)
}

/// VSH frames for gray `level` out of `max_level`, scaled so `max_level` gets `frames_to_black`.
/// Any non-white level gets at least one frame.
fn gray_frames(level: u8, max_level: u8, frames_to_black: u8) -> u8 {
    if level == 0 || max_level == 0 {
        return 0;
    }
    let scaled = (u16::from(frames_to_black) * u16::from(level) + u16::from(max_level) / 2)
        / u16::from(max_level);
    (scaled as u8).max(1)
}

/// Gray drive time in ms as 100 Hz frames, rounded and clamped to one LUT phase (`1..=63`).
fn gray_ms_to_frames(ms: u16) -> u8 {
    let frames = (ms.saturating_add(GRAYSCALE_FRAME_MS / 2)) / GRAYSCALE_FRAME_MS;
    frames.clamp(1, u16::from(MAX_PHASE_FRAMES)) as u8
}

/// Gray level waveform for a pixel that starts white: `frames` at VSL, then `frames` at VSH.
///
/// The VSL half only pushes the white pixel further white, which looks the same; the VSH half
/// darkens it. Equal time each way leaves no net charge, so a gray image is as balanced as a
/// white one. Zero frames gives an empty table, leaving the pixel untouched.
fn balanced_gray_lut(frames: u8) -> [u8; LUT_LEN] {
    if frames == 0 {
        return [0; LUT_LEN];
    }
    single_group_lut([
        (LEVEL_VSL, frames),
        (LEVEL_VSH, frames),
        (LEVEL_GND, 0),
        (LEVEL_GND, 0),
    ])
}

/// Owned LUT storage for the computed grayscale waveforms.
struct LutBank {
    vcom: [u8; LUT_LEN],
    ww: [u8; LUT_LEN],
    kw: [u8; LUT_LEN],
    wk: [u8; LUT_LEN],
    kk: [u8; LUT_LEN],
}

impl LutBank {
    fn new(ww: [u8; LUT_LEN], kw: [u8; LUT_LEN], wk: [u8; LUT_LEN], kk: [u8; LUT_LEN]) -> Self {
        Self {
            vcom: vcom_lut_for([&ww, &kw, &wk, &kk]),
            ww,
            kw,
            wk,
            kk,
        }
    }

    fn as_luts(&self) -> Luts<'_> {
        Luts {
            vcom: &self.vcom,
            ww: &self.ww,
            kw: &self.kw,
            wk: &self.wk,
            kk: &self.kk,
        }
    }

    /// Single-refresh 4-level waveforms for [`Uc8253RefreshMode::Gray4`], for a white screen.
    ///
    /// The plane code picks the level per pixel, so the four transition LUTs carry White (WW,
    /// empty), Light (WK), Dark (KW) and Black (KK), each a [`balanced_gray_lut`].
    fn gray4(frames_to_black: u8) -> Self {
        let level = |gray: u8| balanced_gray_lut(gray_frames(gray, 3, frames_to_black));
        Self::new(level(0), level(2), level(1), level(3))
    }

    /// Waveforms for one [`Uc8253Controller::draw_grayscale`] pass, darkening the three levels
    /// starting at `base` from white: `base` via WW, `base + 1` via KK, `base + 2` via WK, each a
    /// [`balanced_gray_lut`]. KW is left empty so every other pixel is untouched.
    fn gray_pass(base: u8, max_level: u8, frames_to_black: u8) -> Self {
        let level = |gray: u8| {
            if gray > max_level {
                [0; LUT_LEN]
            } else {
                balanced_gray_lut(gray_frames(gray, max_level, frames_to_black))
            }
        };
        Self::new(level(base), [0; LUT_LEN], level(base + 2), level(base + 1))
    }

    /// Waveforms for [`Uc8253RefreshMode::Balanced`]: white→black pushes white then black for
    /// `frames` each, black→white pushes black then white, and unchanged pixels get nothing.
    fn balanced_bw(frames: u8) -> Self {
        let wk = balanced_gray_lut(frames);
        let kw = single_group_lut([
            (LEVEL_VSH, frames),
            (LEVEL_VSL, frames),
            (LEVEL_GND, 0),
            (LEVEL_GND, 0),
        ]);
        Self::new([0; LUT_LEN], kw, wk, [0; LUT_LEN])
    }
}

/// Darkness index (`0` = white … `levels - 1` = black) for a luminance (`0` = black, `255` =
/// white), rounded to the nearest level.
fn gray_level(luma: u8, levels: u8) -> u8 {
    let max_level = u16::from(levels.saturating_sub(1));
    ((u16::from(255 - luma) * max_level + 127) / 255) as u8
}

/// The (OLD, NEW) RAM bits that route a pixel of darkness `level` to its transition LUT in the
/// [`Uc8253Controller::draw_grayscale`] pass starting at `base`. A set bit is white.
fn gray_pass_bits(level: u8, base: u8) -> (bool, bool) {
    if level == base {
        (true, true) // WW
    } else if level == base + 1 {
        (false, false) // KK
    } else if level == base + 2 {
        (true, false) // WK
    } else {
        (false, true) // KW: untouched this pass
    }
}

/// UC8253 Command Definitions
pub mod cmd {
    /// Panel Setting command
    pub const PANEL_SETTING: u8 = 0x00;
    /// Power OFF command
    pub const POWER_OFF: u8 = 0x02;
    /// Power ON command
    pub const POWER_ON: u8 = 0x04;
    /// Booster Soft Start command
    pub const BOOSTER_SOFT_START: u8 = 0x06;
    /// Deep Sleep command
    pub const DEEP_SLEEP: u8 = 0x07;
    /// Write "old"/previous image data plane
    pub const WRITE_OLD_DATA: u8 = 0x10;
    /// Display Refresh command
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// Write "new"/current image data plane
    pub const WRITE_NEW_DATA: u8 = 0x13;
    /// VCOM and Data Interval Setting (CDI)
    pub const CDI: u8 = 0x50;
    /// PLL Control (PLL): sets the frame rate, and with it the length of every LUT frame
    pub const PLL: u8 = 0x30;
    /// Resolution Setting (TRES): HRES byte, then VRES high/low bytes
    pub const RESOLUTION: u8 = 0x61;
    /// Cascade Setting (CCSET), used to force a temperature-compensated fast waveform speed
    pub const CASCADE_SETTING: u8 = 0xE0;
    /// Force Temperature (TSSET), used together with `CASCADE_SETTING` for fast update modes
    pub const FORCE_TEMP: u8 = 0xE5;
    /// Partial Window In command
    pub const PARTIAL_IN: u8 = 0x91;
    /// Partial Window command
    pub const PARTIAL_WINDOW: u8 = 0x90;
    /// Partial Window Out command
    pub const PARTIAL_OUT: u8 = 0x92;
    /// Auto Sequence (AUTO): with check code `0xA5`, runs power on, display refresh, power off
    pub const AUTO_SEQUENCE: u8 = 0x17;
    /// VCOM LUT (LUTC)
    pub const LUT_VCOM: u8 = 0x20;
    /// White-to-white LUT (LUTWW)
    pub const LUT_WW: u8 = 0x21;
    /// Black-to-white LUT (LUTKW)
    pub const LUT_KW: u8 = 0x22;
    /// White-to-black LUT (LUTWK)
    pub const LUT_WK: u8 = 0x23;
    /// Black-to-black LUT (LUTKK)
    pub const LUT_KK: u8 = 0x24;
}

/// How long to wait for BUSY to assert after a command, before giving up and waiting for
/// completion anyway.
///
/// Generous relative to the ~17 s refresh it guards. A panel that has not asserted within half a
/// second is not going to, and falling through leaves the "missing panel reads idle" behaviour
/// intact rather than hanging.
const BUSY_ASSERT_TIMEOUT_MS: u32 = 500;

/// Panel register profile for the UC8253 controller.
///
/// The UC8253 drives panels whose init sequences, RAM plane assignment and refresh handling
/// differ enough that one profile cannot serve both. Selecting the wrong variant does not
/// error — it renders inverted or blank — so the variant must match the panel type the
/// [`crate::driver::EpdDriver`] is built with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Uc8253Variant {
    /// Good Display `GDEY037T03` profile (GxEPD2 lineage).
    ///
    /// Monochrome; `PANEL_SETTING` soft-reset init; Black/White plane on
    /// [`cmd::WRITE_NEW_DATA`]; every refresh powers the panel up and back down, and re-issues
    /// `CDI`. Supports all four [`Uc8253RefreshMode`] modes.
    #[default]
    Gdey037t03,
    /// Waveshare `SE0352N14-TNG-A0` (3.52" e-Paper HAT (B)) profile.
    ///
    /// Tri-Color; explicit `POWER_ON`/`CDI`/`RESOLUTION`/`BOOSTER_SOFT_START` init; Black/White
    /// plane on [`cmd::WRITE_OLD_DATA`] — swapped relative to [`Uc8253Variant::Gdey037t03`];
    /// refresh is `POWER_ON` then `DISPLAY_REFRESH`, without re-issuing `CDI`.
    ///
    /// Full refresh only (~16–20 s): the red pigment has no differential waveform, so
    /// [`Uc8253RefreshMode`] is ignored for this variant.
    ///
    /// The controller drops its charge pump after an update, so each refresh powers it back on.
    /// Waveshare's reference driver does this by re-running its entire init before every display
    /// operation — one refresh per init. Skipping it does not error: the `DISPLAY_REFRESH` is
    /// silently ignored, BUSY never asserts, and the refresh appears to complete instantly having
    /// drawn nothing.
    Se0352n14,
    /// 2.13" `SFE213ZHBW` profile.
    ///
    /// Monochrome. `init_sequence` performs a 30 ms hardware reset, then writes the panel's
    /// register init (`PANEL_SETTING`, `0x01`, `0x03`, `BOOSTER_SOFT_START`, `0x30`, `0x60`,
    /// `0x82`, `0xE3`, `0x41`, `RESOLUTION` 122 × 250, `0x65`), clears both RAM planes to white,
    /// then loads the GC LUTs twice, each followed by an Auto Sequence trigger (`0x17`, `0xA5`).
    /// `trigger_refresh` loads the DU LUTs and triggers `0x17` the same way. The LUT2/LUT3
    /// register order alternates on every load.
    ///
    /// [`Uc8253RefreshMode::Full`], `FastFull`, `Partial` and `FastPartial` all refresh with the
    /// DU LUTs. [`Uc8253RefreshMode::GlobalClear`], [`Uc8253RefreshMode::Gray4`] and
    /// [`Uc8253RefreshMode::Balanced`] are specific to this profile, as are
    /// [`Uc8253Controller::draw_grayscale`] and [`Uc8253Controller::clear_to_white`].
    Sfe213zhbw,
}

/// Display refresh operating mode for the UC8253 controller.
///
/// Ignored by [`Uc8253Variant::Se0352n14`], which is full-refresh only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Uc8253RefreshMode {
    /// Full display update using the panel's default waveform speed (CDI `0x97`).
    #[default]
    Full,
    /// Full display update forced to a faster temperature-compensated waveform (CCSET/TSSET `0x5A`, CDI `0x97`).
    FastFull,
    /// Partial display update (CDI `0xD7`).
    Partial,
    /// Partial display update forced to a faster temperature-compensated waveform (CCSET/TSSET `0x6E`, CDI `0xD7`).
    FastPartial,
    /// [`Uc8253Variant::Sfe213zhbw`] only: refresh with the vendor GC (global clear) LUTs used
    /// during init.
    ///
    /// Flashes, and drives every pixel fully to its NEW-plane colour. Like a DU refresh, it
    /// picks each pixel's waveform from its OLD → NEW transition, so the OLD plane must match
    /// the glass for the charge to balance; to clear to white,
    /// [`Uc8253Controller::clear_to_white`] picks the right planes for what the screen shows.
    /// Waits for the refresh to finish. Other variants treat this as [`Self::Full`].
    GlobalClear,
    /// [`Uc8253Variant::Sfe213zhbw`] only: one-refresh, 4-level grayscale, drawn onto a white
    /// screen.
    ///
    /// The two RAM planes carry a 2-bit gray code per pixel instead of old/new images: draw with
    /// a [`GrayBufferPair`](crate::graphics::GrayBufferPair) using
    /// [`Gray4Polarity::UC8253_SFE213ZHBW`](crate::graphics::Gray4Polarity::UC8253_SFE213ZHBW),
    /// writing plane A to [`ColorChannel::BlackWhite`] and plane B to
    /// [`ColorChannel::RedYellow`].
    ///
    /// Each gray level pushes its pixels towards white, then towards black for the same time, so
    /// the image carries no net charge; White pixels are untouched. The waveform has no cleaning
    /// flash of its own, so first clear to white with [`Uc8253Controller::clear_to_white`]. Runs
    /// at 100 Hz, restoring the panel's 50 Hz afterwards, and spreads the levels over
    /// [`Uc8253Controller::with_gray4_black_ms`]. Waits for the refresh to finish.
    ///
    /// The planes no longer describe the image on the glass afterwards, so clear it with
    /// [`Uc8253WhiteClear::BalancedFlash`]. Other variants treat this as [`Self::Full`].
    ///
    /// The waveform is computed by `epdsi`, not supplied by the panel vendor.
    Gray4,
    /// [`Uc8253Variant::Sfe213zhbw`] only: a monochrome update that leaves no net charge.
    ///
    /// Like the default DU refresh, only pixels that change are driven, so the rest of the
    /// screen does not flash. White→black pushes towards white and then towards black for
    /// [`Uc8253Controller::with_balanced_ms`] each, black→white the reverse. The vendor DU
    /// refresh instead leaves each black pixel owing a push back towards white until it is
    /// turned white again, which a power loss can stop from ever happening. Runs at 100 Hz,
    /// restoring the panel's 50 Hz afterwards, and waits for the refresh to finish.
    ///
    /// As with DU, the OLD plane picks each pixel's transition. Writing only the NEW plane
    /// keeps it matched between `Balanced` refreshes. After a grayscale refresh, write the OLD
    /// plane too: equal to the NEW plane wherever the gray image should stay (those pixels are
    /// then left alone), and the previous monochrome content where it changes. Clear a screen
    /// drawn this way with [`Uc8253WhiteClear::BalancedFlash`]. Other variants treat this as
    /// [`Self::Full`].
    ///
    /// The waveform is computed by `epdsi`, not supplied by the panel vendor.
    Balanced,
}

/// How [`Uc8253Controller::clear_to_white`] (and the clear that opens
/// [`Uc8253Controller::draw_grayscale`]) returns an `Sfe213zhbw` screen to white, chosen by how
/// the screen was drawn.
///
/// The vendor DU and GC refreshes leave each black pixel owing a push back towards white, paid
/// only by a black→white transition, which needs the OLD plane to show which pixels are black.
/// `epdsi`'s balanced refreshes ([`Uc8253RefreshMode::Gray4`], [`Uc8253RefreshMode::Balanced`]
/// and [`Uc8253Controller::draw_grayscale`]) leave nothing owing. Picking the wrong variant does
/// not error, but leaves net charge on pixels that builds up over repeated cycles, or a ghost of
/// the old image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Uc8253WhiteClear {
    /// The screen was drawn with the default DU refresh (or GC): a DU refresh to white (about
    /// 0.4 s), with no flash. Black pixels get the vendor black→white drive that repays them,
    /// white ones are left alone. Relies on the OLD plane matching the glass, as normal DU
    /// refreshes keep it.
    Mono,
    /// As [`Self::Mono`], but with the vendor GC flash (about 1 s) for an even white.
    MonoFlash,
    /// The screen was drawn with balanced refreshes only, or nothing is known about it (e.g.
    /// at power-up): both planes are rewritten white and the GC flash is applied evenly to every
    /// pixel (about 1 s). It adds no charge whatever the screen shows, but cannot repay a DU
    /// image's black pixels, and cannot tell gray levels apart, so a faint ghost may remain;
    /// repeat it to fade one.
    BalancedFlash,
    /// The screen is already white, e.g. straight after another clear: nothing is sent.
    AlreadyWhite,
}

/// UC8253 Controller IC driver configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uc8253Controller {
    width: u32,
    height: u32,
    variant: Uc8253Variant,
    refresh_mode: Uc8253RefreshMode,
    /// Last window set via `set_window`, tracked so `trigger_refresh` knows whether to close
    /// partial-window mode (`PARTIAL_OUT`) before refreshing.
    window: Option<(u32, u32, u32, u32)>,
    /// `Sfe213zhbw` LUT2/LUT3 swap parity, flipped on every LUT load (one load per refresh).
    lut_flag: bool,
    /// `Sfe213zhbw` [`Uc8253RefreshMode::Gray4`]: frames (at 100 Hz) for the darkest level.
    gray4_black_frames: u8,
    /// `Sfe213zhbw` [`Self::draw_grayscale`]: frames (at 100 Hz) for the darkest level.
    grayscale_black_frames: u8,
    /// `Sfe213zhbw` [`Uc8253RefreshMode::Balanced`]: frames (at 100 Hz) pushed each way.
    balanced_frames: u8,
}

impl Uc8253Controller {
    /// Creates a new UC8253 controller configured for target dimensions.
    ///
    /// Defaults to [`Uc8253Variant::Gdey037t03`]; use [`Self::with_variant`] for other panels.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            variant: Uc8253Variant::default(),
            refresh_mode: Uc8253RefreshMode::default(),
            window: None,
            lut_flag: false,
            gray4_black_frames: gray_ms_to_frames(DEFAULT_GRAY_BLACK_MS),
            grayscale_black_frames: gray_ms_to_frames(DEFAULT_GRAY_BLACK_MS),
            balanced_frames: gray_ms_to_frames(DEFAULT_BALANCED_MS),
        }
    }

    /// Sets the panel register profile (builder method).
    pub fn with_variant(mut self, variant: Uc8253Variant) -> Self {
        self.variant = variant;
        self
    }

    /// Returns the configured panel register profile.
    pub fn variant(&self) -> Uc8253Variant {
        self.variant
    }

    /// Sets display refresh operating mode (builder method).
    pub fn with_refresh_mode(mut self, mode: Uc8253RefreshMode) -> Self {
        self.refresh_mode = mode;
        self
    }

    /// Sets display refresh operating mode.
    pub fn set_refresh_mode(&mut self, mode: Uc8253RefreshMode) {
        self.refresh_mode = mode;
    }

    /// Returns current display refresh mode.
    pub fn refresh_mode(&self) -> Uc8253RefreshMode {
        self.refresh_mode
    }

    /// Sets how long [`Uc8253RefreshMode::Gray4`] drives its darkest level, Black (builder
    /// method), rounded to 10 ms frames and clamped to `10..=630` ms.
    ///
    /// Light and Dark sit at a third and two thirds of it. Tune it the same way as
    /// [`Self::with_grayscale_black_ms`]: lower it if Dark and Black look alike, raise it if
    /// Black is not black. Defaults to 160 ms.
    pub fn with_gray4_black_ms(mut self, ms: u16) -> Self {
        self.gray4_black_frames = gray_ms_to_frames(ms);
        self
    }

    /// Returns the [`Uc8253RefreshMode::Gray4`] darkest-level drive time, in ms.
    pub fn gray4_black_ms(&self) -> u16 {
        u16::from(self.gray4_black_frames) * GRAYSCALE_FRAME_MS
    }

    /// Sets how long [`Self::draw_grayscale`] drives its darkest level (builder method), rounded
    /// to 10 ms frames and clamped to `10..=630` ms.
    ///
    /// Each level pushes its pixels towards white and then towards black, for a time spaced
    /// linearly between white and this value. Past the point where the glass is fully black,
    /// extra time only merges the darkest levels together, so this should sit just at
    /// saturation: if the darkest bands look alike, lower it; if the darkest band is not black,
    /// raise it. Defaults to 160 ms.
    pub fn with_grayscale_black_ms(mut self, ms: u16) -> Self {
        self.grayscale_black_frames = gray_ms_to_frames(ms);
        self
    }

    /// Returns the [`Self::draw_grayscale`] darkest-level drive time, in ms.
    pub fn grayscale_black_ms(&self) -> u16 {
        u16::from(self.grayscale_black_frames) * GRAYSCALE_FRAME_MS
    }

    /// Sets how long a [`Uc8253RefreshMode::Balanced`] update pushes each way (builder method),
    /// rounded to 10 ms frames and clamped to `10..=630` ms.
    ///
    /// Each changed pixel is pushed for this long away from its target colour, then this long
    /// towards it, so an update takes about twice this. It must be long enough to switch a pixel
    /// fully: raise it if new black text looks gray or erased text leaves a trace, lower it for
    /// faster updates. Defaults to 200 ms.
    pub fn with_balanced_ms(mut self, ms: u16) -> Self {
        self.balanced_frames = gray_ms_to_frames(ms);
        self
    }

    /// Returns the [`Uc8253RefreshMode::Balanced`] push time each way, in ms.
    pub fn balanced_ms(&self) -> u16 {
        u16::from(self.balanced_frames) * GRAYSCALE_FRAME_MS
    }

    /// Clears any recorded partial window, returning the controller to full-frame addressing.
    ///
    /// Full-frame writes must not be wrapped in a partial-window session, so call this before a
    /// full-screen write/refresh that follows partial updates.
    pub fn clear_window(&mut self) {
        self.window = None;
    }

    /// Maps a colour channel onto the RAM plane command for the configured variant.
    ///
    /// The two planes are swapped between variants: the `GDEY037T03` profile treats
    /// `WRITE_NEW_DATA` as the Black/White plane and `WRITE_OLD_DATA` as the previous-frame plane
    /// used for differential updates, while the tri-colour `SE0352N14-TNG-A0` puts Black/White on
    /// `WRITE_OLD_DATA` and the red pigment on `WRITE_NEW_DATA`. Crossing them routes black pixels
    /// into the red plane.
    fn plane_command(&self, channel: ColorChannel) -> u8 {
        let is_black_white = matches!(channel, ColorChannel::BlackWhite);
        match self.variant {
            Uc8253Variant::Gdey037t03 | Uc8253Variant::Sfe213zhbw => {
                if is_black_white {
                    cmd::WRITE_NEW_DATA
                } else {
                    cmd::WRITE_OLD_DATA
                }
            }
            Uc8253Variant::Se0352n14 => {
                if is_black_white {
                    cmd::WRITE_OLD_DATA
                } else {
                    cmd::WRITE_NEW_DATA
                }
            }
        }
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl Uc8253Controller {
    /// Opens the recorded partial window (`PARTIAL_IN` + `PARTIAL_WINDOW`), if one is set.
    ///
    /// The UC8253 requires the window to be re-opened around *each* RAM write and again around
    /// the refresh, so this is emitted per operation rather than once by `set_window`.
    #[allow(clippy::type_complexity)]
    async fn open_window<SPI, DC, RST, BUSY>(
        &self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
    {
        let Some((x_start, y_start, x_end, y_end)) = self.window else {
            return Ok(());
        };

        let x = (x_start & 0xFFF8) as u8;
        let xe = (x_end | 0x0007) as u8;
        let y = y_start as u16;
        let ye = y_end as u16;

        bus.send_command(cmd::PARTIAL_IN).await?;
        bus.send_command_with_data(
            cmd::PARTIAL_WINDOW,
            &[
                x,
                xe,
                (y / 256) as u8,
                (y % 256) as u8,
                (ye / 256) as u8,
                (ye % 256) as u8,
                0x01,
            ],
        )
        .await
    }

    /// Closes the partial window (`PARTIAL_OUT`), if one is set.
    #[allow(clippy::type_complexity)]
    async fn close_window<SPI, DC, RST, BUSY>(
        &self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
    {
        if self.window.is_some() {
            bus.send_command(cmd::PARTIAL_OUT).await?;
        }
        Ok(())
    }

    /// Re-sends the two Panel Setting register writes performed during `init_sequence`, used to
    /// undo the CCSET/TSSET "TSFIX" temperature override after a fast-update refresh.
    #[allow(clippy::type_complexity)]
    async fn reset_panel_setting<SPI, DC, RST, BUSY, DELAY>(
        &self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
        DELAY: DelayNs,
    {
        // `0x1E` clears RST_N, which is a soft reset; the controller needs time to settle before
        // `0x1F` releases it. Without the wait the reset's power-on defaults win and the scan
        // direction flips, rotating every subsequent frame by 180 degrees.
        bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1E, 0x0D])
            .await?;
        delay.delay_ms(1).await;
        bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1F, 0x0D])
            .await
    }

    /// Loads a full `Sfe213zhbw` LUT set, applying the vendor driver's LUT2/LUT3 swap.
    ///
    /// The vendor code trades the black→white and white→black tables between registers `0x22`
    /// and `0x23` on every other load, and refreshes that only ever write the NEW plane render
    /// correctly on hardware that way. That fits a controller which ping-pongs its two RAM
    /// buffers on each refresh but compares them in a fixed order, reversing every other
    /// transition. Only `kw`/`wk` need the swap: reversing a white→white or black→black pixel
    /// leaves it unchanged. Every refresh must therefore go through here exactly once, so the
    /// tables written stay in step with the controller.
    #[allow(clippy::type_complexity)]
    async fn load_luts<SPI, DC, RST, BUSY>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        luts: Luts<'_>,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
    {
        bus.send_command_with_data(cmd::LUT_VCOM, luts.vcom).await?;
        bus.send_command_with_data(cmd::LUT_WW, luts.ww).await?;
        bus.send_command_with_data(cmd::LUT_KK, luts.kk).await?;
        if !self.lut_flag {
            bus.send_command_with_data(cmd::LUT_KW, luts.kw).await?;
            bus.send_command_with_data(cmd::LUT_WK, luts.wk).await?;
        } else {
            bus.send_command_with_data(cmd::LUT_WK, luts.kw).await?;
            bus.send_command_with_data(cmd::LUT_KW, luts.wk).await?;
        }
        self.lut_flag = !self.lut_flag;
        Ok(())
    }

    /// Triggers the Auto Sequence (power on, refresh, power off) and waits for it to finish.
    #[allow(clippy::type_complexity)]
    async fn auto_refresh_and_wait<SPI, DC, RST, BUSY, DELAY>(
        &self,
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
        bus.send_command_with_data(cmd::AUTO_SEQUENCE, &[0xA5])
            .await?;
        bus.wait_busy_assert(delay, false, BUSY_ASSERT_TIMEOUT_MS)
            .await?;
        bus.wait_busy_with_delay(delay, false).await
    }

    /// Streams one `draw_grayscale` pass's OLD and NEW planes, returning whether any pixel
    /// falls into the pass's three levels.
    #[allow(clippy::type_complexity)]
    async fn write_gray_pass<SPI, DC, RST, BUSY, F>(
        &self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        levels: u8,
        base: u8,
        shade: &mut F,
    ) -> Result<bool, EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
        F: FnMut(u32, u32) -> u8,
    {
        let stride = (self.width.div_ceil(8) as usize).min(MAX_ROW_BYTES);
        let mut any = false;
        for (command, is_new_plane) in [(cmd::WRITE_OLD_DATA, false), (cmd::WRITE_NEW_DATA, true)] {
            bus.send_command(command).await?;
            for y in 0..self.height {
                // Padding bits past the panel width use the untouched (KW) code.
                let mut row = [if is_new_plane { 0xFF } else { 0x00 }; MAX_ROW_BYTES];
                for x in 0..self.width.min((MAX_ROW_BYTES * 8) as u32) {
                    let level = gray_level(shade(x, y), levels);
                    let (old, new) = gray_pass_bits(level, base);
                    any |= old || !new;
                    let mask = 0x80u8 >> (x % 8);
                    let byte = &mut row[(x / 8) as usize];
                    if if is_new_plane { new } else { old } {
                        *byte |= mask;
                    } else {
                        *byte &= !mask;
                    }
                }
                bus.send_data(&row[..stride]).await?;
            }
        }
        Ok(any)
    }

    /// Returns an `Sfe213zhbw` screen to white, choosing the refresh by what it currently shows
    /// (see [`Uc8253WhiteClear`]). Waits for the refresh to finish. Uses full-frame addressing
    /// and clears any recorded partial window.
    ///
    /// Afterwards both RAM planes describe a white screen, so normal refreshes, a
    /// [`Uc8253RefreshMode::Gray4`] image or [`Self::draw_grayscale`] with
    /// [`Uc8253WhiteClear::AlreadyWhite`] can follow.
    ///
    /// # Errors
    ///
    /// [`EpdBusError::Unsupported`] for any variant other than [`Uc8253Variant::Sfe213zhbw`],
    /// before anything is sent.
    #[allow(clippy::type_complexity)]
    pub async fn clear_to_white<SPI, DC, RST, BUSY, DELAY>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
        clear: Uc8253WhiteClear,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin + Wait,
        DELAY: DelayNs,
    {
        if self.variant != Uc8253Variant::Sfe213zhbw {
            return Err(ValidationError::Unsupported.into());
        }
        self.window = None;
        let plane_bytes = (self.width.div_ceil(8) * self.height) as usize;
        let luts = match clear {
            Uc8253WhiteClear::AlreadyWhite => return Ok(()),
            Uc8253WhiteClear::Mono => LUTS_DU,
            Uc8253WhiteClear::MonoFlash => LUTS_GC,
            Uc8253WhiteClear::BalancedFlash => {
                // A white OLD plane routes every pixel to the white→white flash.
                bus.send_command(cmd::WRITE_OLD_DATA).await?;
                bus.send_data_repeated(0xFF, plane_bytes).await?;
                LUTS_GC
            }
        };
        bus.send_command(cmd::WRITE_NEW_DATA).await?;
        bus.send_data_repeated(0xFF, plane_bytes).await?;
        self.load_luts(bus, luts).await?;
        self.auto_refresh_and_wait(bus, delay).await
    }

    /// Draws a multi-level grayscale image over several refreshes (`Sfe213zhbw` only).
    ///
    /// `shade(x, y)` returns each pixel's luminance, `0` black to `255` white, in raw panel
    /// coordinates (`0..width`, `0..height`, unrotated); it is called twice per pixel per pass,
    /// so nothing is buffered. Luminance is quantised to `levels` evenly spaced grays, clamped to
    /// `2..=`[`MAX_GRAY_LEVELS`].
    ///
    /// `clear` first returns the screen to white, as [`Self::clear_to_white`]; pick it by what
    /// the screen shows now. Then, after antirez's `uc8151_micropython` greyscale mode, each pass
    /// darkens three levels at once from white — the OLD/NEW planes route those levels to the
    /// WW, KK and WK tables and park every other pixel on an empty KW table. A pass whose levels
    /// no pixel uses is skipped, so `levels = 16` costs at most five pass refreshes.
    ///
    /// Each level pushes its pixels towards white, then towards black for the same time, so the
    /// image carries no net charge and needs nothing from the image to clear it: use
    /// [`Uc8253WhiteClear::BalancedFlash`]. The passes run at 100 Hz rather than the panel's 50 Hz,
    /// for 10 ms steps, and spread the levels linearly over [`Self::with_grayscale_black_ms`].
    /// The frame rate is restored afterwards. Uses full-frame addressing and clears any recorded
    /// partial window.
    ///
    /// # Errors
    ///
    /// [`EpdBusError::Unsupported`] for any variant other than [`Uc8253Variant::Sfe213zhbw`],
    /// before anything is sent.
    #[allow(clippy::type_complexity)]
    pub async fn draw_grayscale<SPI, DC, RST, BUSY, DELAY, F>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
        clear: Uc8253WhiteClear,
        levels: u8,
        mut shade: F,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin + Wait,
        DELAY: DelayNs,
        F: FnMut(u32, u32) -> u8,
    {
        self.clear_to_white(bus, delay, clear).await?;
        let levels = levels.clamp(2, MAX_GRAY_LEVELS);

        bus.send_command_with_data(cmd::PLL, &[PLL_100HZ]).await?;
        let passes = self.gray_passes(bus, delay, levels, &mut shade).await;
        // Restore the panel's 50 Hz even if a pass failed.
        let restore = bus.send_command_with_data(cmd::PLL, &[PLL_50HZ]).await;
        passes.and(restore)
    }

    /// Loads `luts` and refreshes once at 100 Hz, as every `epdsi`-computed waveform runs.
    #[allow(clippy::type_complexity)]
    async fn fast_refresh<SPI, DC, RST, BUSY, DELAY>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
        luts: LutBank,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin + Wait,
        DELAY: DelayNs,
    {
        bus.send_command_with_data(cmd::PLL, &[PLL_100HZ]).await?;
        let mut refresh = self.load_luts(bus, luts.as_luts()).await;
        if refresh.is_ok() {
            refresh = self.auto_refresh_and_wait(bus, delay).await;
        }
        // Restore the panel's 50 Hz even if the refresh failed.
        let restore = bus.send_command_with_data(cmd::PLL, &[PLL_50HZ]).await;
        refresh.and(restore)
    }

    /// Runs every `draw_grayscale` pass, skipping those whose levels no pixel uses.
    #[allow(clippy::type_complexity)]
    async fn gray_passes<SPI, DC, RST, BUSY, DELAY, F>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
        levels: u8,
        shade: &mut F,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin + Wait,
        DELAY: DelayNs,
        F: FnMut(u32, u32) -> u8,
    {
        let max_level = levels - 1;
        for base in (1..=max_level).step_by(3) {
            if self.write_gray_pass(bus, levels, base, shade).await? {
                let luts = LutBank::gray_pass(base, max_level, self.grayscale_black_frames);
                self.load_luts(bus, luts.as_luts()).await?;
                self.auto_refresh_and_wait(bus, delay).await?;
            }
        }
        Ok(())
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Uc8253Controller
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
        match self.variant {
            Uc8253Variant::Gdey037t03 => {
                bus.hard_reset(delay, 10).await?;
                // `0x1E` clears RST_N (soft reset); the 1 ms wait lets it settle before `0x1F`
                // releases it.
                bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1E, 0x0D])
                    .await?;
                delay.delay_ms(1).await;
                bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1F, 0x0D])
                    .await?;
            }
            Uc8253Variant::Se0352n14 => {
                // A 30 ms RST low pulse, matching Pervasive Displays' reference driver for this
                // panel family. Waveshare's uses 2 ms, which proved marginal: on a XIAO ESP32-C3
                // it latched intermittently, and a reset that does not take leaves the controller
                // ignoring POWER_ON and DISPLAY_REFRESH — BUSY never asserts and the refresh
                // returns having drawn nothing, at a random frame each run.
                //
                // `hard_reset` does not cover the settle that must follow, so that is added here.
                bus.hard_reset(delay, 30).await?;
                delay.delay_ms(200).await;

                // Power comes up here rather than per-refresh: this panel's refresh is a bare
                // `DISPLAY_REFRESH`, so the rails stay up between frames.
                bus.send_command(cmd::POWER_ON).await?;
                delay.delay_ms(100).await;
                bus.wait_busy_with_delay(delay, false).await?;

                // `0x87` — not the `0x97` the GDEY037T03 profile uses. The two differ in the DDX
                // polarity bits, and `0x87` is what makes `0x00` mean white in both RAM planes on
                // this panel. Re-issuing `0x97` later would invert black and white.
                bus.send_command_with_data(cmd::CDI, &[0x87]).await?;
                bus.send_command_with_data(cmd::PANEL_SETTING, &[0x03, 0x0D])
                    .await?;
                // Unlike the GDEY037T03, which encodes its size in the Panel Setting bits, this
                // panel needs an explicit resolution: HRES ignores the low 3 bits, VRES is 16-bit.
                bus.send_command_with_data(
                    cmd::RESOLUTION,
                    &[
                        (self.width & 0xF8) as u8,
                        (self.height >> 8) as u8,
                        (self.height & 0xFF) as u8,
                    ],
                )
                .await?;
                bus.send_command_with_data(cmd::BOOSTER_SOFT_START, &[0x2F, 0x2F, 0x2E])
                    .await?;
                bus.wait_busy_with_delay(delay, false).await?;
            }
            Uc8253Variant::Sfe213zhbw => {
                // init
                bus.hard_reset(delay, 30).await?;

                bus.send_command_with_data(cmd::PANEL_SETTING, &[0xFF, 0x0A])
                    .await?;
                bus.send_command_with_data(0x01, &[0x03, 0x00, 0x3F, 0x3F, 0x03])
                    .await?;
                bus.send_command_with_data(0x03, &[0x10]).await?;
                bus.send_command_with_data(cmd::BOOSTER_SOFT_START, &[0x27, 0x27, 0x2F])
                    .await?;

                bus.send_command_with_data(cmd::PLL, &[PLL_50HZ]).await?;
                bus.send_command_with_data(0x60, &[0x22]).await?;
                bus.send_command_with_data(0x82, &[0x09]).await?;

                bus.send_command_with_data(0xE3, &[0x00]).await?;
                bus.send_command_with_data(0x41, &[0x00]).await?;

                bus.send_command_with_data(0x61, &[0x80, 0x00, 0xFA])
                    .await?;
                bus.send_command_with_data(0x65, &[0x00, 0x00, 0x00])
                    .await?;

                // clear frame
                self.write_frame_pattern(
                    bus,
                    ColorChannel::RedYellow,
                    0xFF,
                    (self.width.div_ceil(8) * self.height) as usize,
                )
                .await?;

                self.write_frame_pattern(
                    bus,
                    ColorChannel::BlackWhite,
                    0xFF,
                    (self.width.div_ceil(8) * self.height) as usize,
                )
                .await?;

                // lut gc
                self.load_luts(bus, LUTS_GC).await?;
                bus.send_command_with_data(cmd::AUTO_SEQUENCE, &[0xA5])
                    .await?;
                bus.wait_busy_with_delay(delay, false).await?;
                self.load_luts(bus, LUTS_GC).await?;
                bus.send_command_with_data(cmd::AUTO_SEQUENCE, &[0xA5])
                    .await?;
                bus.wait_busy_with_delay(delay, false).await?;
            }
        }
        Ok(())
    }

    /// Records the partial window without emitting any commands.
    ///
    /// The UC8253 needs `PARTIAL_IN` + `PARTIAL_WINDOW` re-issued around every RAM write and
    /// again around the refresh, so the commands are emitted by `write_frame`,
    /// `write_frame_pattern` and `trigger_refresh`. Use [`Uc8253Controller::clear_window`] to
    /// return to full-frame addressing.
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
        // UC8253 has no separate RAM cursor register; window position is set by `set_window`.
        Ok(())
    }

    async fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        let cmd = self.plane_command(channel);
        self.open_window(bus).await?;
        bus.send_command_with_data(cmd, data).await?;
        self.close_window(bus).await
    }

    async fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        let cmd = self.plane_command(channel);
        self.open_window(bus).await?;
        bus.send_command(cmd).await?;
        bus.send_data_repeated(byte, count).await?;
        self.close_window(bus).await
    }

    async fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // The refresh gets its own partial-window session, re-opened here and closed after the
        // update. The window is deliberately kept for subsequent operations; call
        // `clear_window` to return to full-frame addressing.
        self.open_window(bus).await?;

        if self.variant == Uc8253Variant::Se0352n14 {
            // `CDI` is deliberately not re-issued — `0x97` would move the DDX polarity bits away
            // from the `0x87` set at init and invert black and white. `refresh_mode` has no
            // effect here either: tri-colour ink has no differential waveform.
            //
            // Power on before every refresh. Waveshare's reference driver re-runs its whole
            // init — which begins with POWER_ON — before each display operation, exactly one
            // refresh per init. The controller drops its charge pump after an update, and a bare
            // DISPLAY_REFRESH on an unpowered controller is silently ignored: BUSY never asserts,
            // the poll reads idle, and the refresh "finishes" instantly having done nothing.
            //
            // The fixed 100 ms settle matches both the reference driver and this controller's own
            // `init_sequence`. Watching for a BUSY edge here instead is worse: `POWER_ON` often
            // does not assert BUSY at all on this panel, so the edge wait just burns its timeout,
            // and when BUSY *does* blip low during a ramp it can release the wait before the
            // booster has stabilised.
            bus.send_command(cmd::POWER_ON).await?;
            delay.delay_ms(100).await;
            bus.wait_busy_with_delay(delay, false).await?;

            // Wait for BUSY to actually assert before waiting for it to clear. A fixed settling
            // delay is not sufficient: assertion latency varies, and a 10 ms guard was observed
            // holding on some refreshes and missing on others on the same board, leaving the
            // panel a frame behind whenever it missed.
            bus.send_command(cmd::DISPLAY_REFRESH).await?;
            bus.wait_busy_assert(delay, false, BUSY_ASSERT_TIMEOUT_MS)
                .await?;
            bus.wait_busy_with_delay(delay, false).await?;
            return self.close_window(bus).await;
        }

        if self.variant == Uc8253Variant::Sfe213zhbw {
            match self.refresh_mode {
                Uc8253RefreshMode::GlobalClear => {
                    self.load_luts(bus, LUTS_GC).await?;
                    self.auto_refresh_and_wait(bus, delay).await?;
                }
                Uc8253RefreshMode::Gray4 => {
                    let luts = LutBank::gray4(self.gray4_black_frames);
                    self.fast_refresh(bus, delay, luts).await?;
                }
                Uc8253RefreshMode::Balanced => {
                    let luts = LutBank::balanced_bw(self.balanced_frames);
                    self.fast_refresh(bus, delay, luts).await?;
                }
                _ => {
                    self.load_luts(bus, LUTS_DU).await?;
                    bus.send_command_with_data(cmd::AUTO_SEQUENCE, &[0xA5])
                        .await?;
                }
            }
            return self.close_window(bus).await;
        }

        let is_fast = matches!(
            self.refresh_mode,
            Uc8253RefreshMode::FastFull | Uc8253RefreshMode::FastPartial
        );
        let is_partial = matches!(
            self.refresh_mode,
            Uc8253RefreshMode::Partial | Uc8253RefreshMode::FastPartial
        );

        if is_fast {
            let temp = if is_partial { 0x6E } else { 0x5A };
            bus.send_command_with_data(cmd::CASCADE_SETTING, &[0x02])
                .await?;
            bus.send_command_with_data(cmd::FORCE_TEMP, &[temp]).await?;
        }

        let cdi = if is_partial { 0xD7 } else { 0x97 };
        bus.send_command_with_data(cmd::CDI, &[cdi]).await?;

        bus.send_command(cmd::POWER_ON).await?;
        bus.wait_busy_with_delay(delay, false).await?;

        // Wait for the BUSY edge before waiting for completion. On a host that polls quickly this
        // is what separates "the refresh finished" from "the panel had not started yet" — without
        // it a refresh can report 0 ms while still running, or while the controller ignored the
        // command entirely. Costs one poll when BUSY asserts normally, so the fast-partial path is
        // unaffected; only the already-broken case pays the timeout.
        bus.send_command(cmd::DISPLAY_REFRESH).await?;
        bus.wait_busy_assert(delay, false, BUSY_ASSERT_TIMEOUT_MS)
            .await?;
        bus.wait_busy_with_delay(delay, false).await?;

        bus.send_command(cmd::POWER_OFF).await?;
        bus.wait_busy_with_delay(delay, false).await?;

        if is_fast {
            self.reset_panel_setting(bus, delay).await?;
        }

        self.close_window(bus).await
    }

    async fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::POWER_OFF).await?;
        bus.wait_busy_with_delay(delay, false).await?;
        bus.send_command_with_data(cmd::DEEP_SLEEP, &[0xA5]).await
    }
}
