//! Panel specification for GDEM0154F61H (1.54" 200x200 Quad-Color e-Paper display).
//!
//! ### Hardware Notes:
//! - **Vendor**: Good Display, rear marking `0154SW-F61H-B2`, 24-pin FPC (`FPC-8101`),
//!   `DESPI-C02` compatible.
//! - **Controller IC**: SSD2681, driven by [`Ssd2681Controller`](crate::controllers::Ssd2681Controller).
//!   The look-alike [`GDEM0154F51H`](super::GDEM0154F51H) uses the JD79660 and a different init.
//!   Picking the wrong controller draws nothing or wrong colors instead of erroring.
//! - **Native resolution**: 200 x 200 pixels, already byte-aligned (50 bytes per row, 10,000
//!   bytes per frame at 2 bits per pixel).
//! - **Busy polarity**: Active-**LOW** (busy while LOW). Use a pull-up input.
//! - **Colour codes**: black `00`, white `01`, yellow `10`, red `11`.
//! - **Refresh**: full refresh only, no partial.
//! - **Status**: register sequences replay Good Display's `AU-GDEM0154F61H-F-20260513` demo,
//!   checked against `SSD2681.pdf` (rev 1.0). Not yet bench verified.

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel specification for the Good Display 1.54" Quad-Color ePaper (SSD2681).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEM0154F61H;

impl EpdPanel for GDEM0154F61H {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 200;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 200;

    /// Panel color operating mode (Quad-Color: Black, White, Red, Yellow).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;
}
