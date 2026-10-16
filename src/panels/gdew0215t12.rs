//! Panel specification for GDEW0215T12 (2.15" 112x208 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Vendor**: Good Display. Formerly sold as `GDEW0215T11`; the part number printed on the glass
//!   is `GDEW0215T12`.
//! - **Controller IC**: UC8151D, driven by [`Uc8151dController`](crate::controllers::Uc8151dController).
//!   The two other UC8253 profiles in this crate do not apply: their init, RAM plane order and
//!   refresh handling differ, and picking the wrong controller renders blank or inverted instead
//!   of erroring.
//! - **Native resolution**: 112 x 208 pixels (source by gate), already byte-aligned. The vendor
//!   calls it 208 x 112 in landscape.
//! - **Busy polarity**: Active-**LOW** (busy while LOW). Use a pull-up input.
//! - **SPI clock**: at most 10 MHz. The datasheet's serial write cycle is at least 100 ns
//!   (`tSCYCW`), and Good Display's demos run it at 4 to 10 MHz. The 16 MHz some other panels in
//!   this crate accept is out of spec here.
//! - **Waveform**: stored in the on-chip OTP, 1-bit (black and white only). There is no 4-gray
//!   mode on this panel.
//! - **Status**: hardware-verified on 15 Oct 2026 with the blocking API on an XIAO ESP32-C3 on the Seeed
//!   ePaper Driver Board (full refresh, whole-frame partial, windowed partial). The register
//!   sequence is Good Display's own `AU-GDEW0215T12-P-20230926` demo, checked against the panel
//!   specification (`GDEW0215T12-new.pdf`, rev 1.0). The same four screens also ran to completion on
//!   an Adafruit Feather RP2040 ThinkInk and on a Pico 2, including the async (Embassy) API.

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel specification for the Good Display 2.15" Monochrome ePaper (UC8151D).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEW0215T12;

impl EpdPanel for GDEW0215T12 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 112;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 208;

    /// Panel color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}
