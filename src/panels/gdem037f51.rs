//! Panel specification for GDEM037F51 (3.7" 240x416 Quad-Color e-Paper display).
//!
//! ### Hardware Notes:
//! - **Panel Model**: `GDEM037F51` ([Product Page](https://www.good-display.com/product/1224.html)).
//!   Waveshare sells the same glass as the "3.7inch e-Paper (G)" (SKU 31065); the FPC carries
//!   the markings `FPC-2303` and `037SVV-F51-B2`. Both vendors' demos send the same init bytes.
//! - **Controller IC**: IST7163 (`IST7163CA1-C`). Use
//!   [`Ist7163Controller`](crate::controllers::Ist7163Controller). The IC differs from the
//!   JD79661 family in PWR and BTST length and in its test-mode banks, so do not pair this
//!   panel with [`Jd7966xController`](crate::controllers::Jd7966xController). The mismatch
//!   cannot be caught at compile time.
//! - **Native Resolution**: 240 x 416 pixels, portrait (240 source, 416 gate). Waveshare's
//!   listing says 416x240, which is the same glass rotated.
//! - **RAM Alignment**: 240 is a multiple of 4, so 60 bytes per row and 24,960 bytes per 2bpp
//!   Quad-Color frame, the demo's `Width * Height` with `Width = 240 / 4`.
//! - **Refresh**: full refresh only, about 20 s. BUSY is active-low.
//! - **Colour codes**: black `00`, white `01`, yellow `10`, red `11`.

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel specification for Good Display `GDEM037F51` (IST7163).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEM037F51;

impl EpdPanel for GDEM037F51 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 240;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 416;

    /// Panel color operating mode (Quad-Color: Black, White, Red, Yellow).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;
}
