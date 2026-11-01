//! Panel specification for `SFE213ZHBW` (2.13" 122x250 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Panel Model**: `SFE213ZHBW` (controller + 2.13" size marker in the part number)
//! - **Controller IC**: UC8253 — driven through
//!   [`Uc8253Controller`](crate::controllers::Uc8253Controller) with
//!   [`Uc8253Variant::Sfe213zhbw`](crate::controllers::Uc8253Variant::Sfe213zhbw). The variant is
//!   **not** optional: like [`GDEY037T03`](super::GDEY037T03) this is a single-plane black/white
//!   panel, so the Black/White image plane is
//!   [`ColorChannel::BlackWhite`](crate::traits::ColorChannel::BlackWhite) (`WRITE_NEW_DATA`,
//!   `0x13`) and the previous-frame plane is
//!   [`ColorChannel::RedYellow`](crate::traits::ColorChannel::RedYellow) (`WRITE_OLD_DATA`,
//!   `0x10`). Init writes the register set and GC LUTs, and refresh loads the DU LUTs, each
//!   triggered by the Auto Sequence command (`0x17`). Selecting the wrong variant does not
//!   error: it renders blank or hangs on BUSY.
//! - **Native Resolution**: 122 x 250 pixels (raster, not the landscape 250 x 122 viewing
//!   orientation). Width is not a multiple of 8, so each row occupies
//!   `122.div_ceil(8) = 16` bytes of controller RAM and the trailing 6 bits are off-panel
//!   padding — 4,000 bytes total per frame, same RAM geometry as
//!   [`GDEM0213B74`](super::GDEM0213B74).
//! - **Busy Polarity**: Active-**LOW** (busy while LOW), as for the other UC8253 panels here.
//! - **Ink Polarity**: Monochrome `0xFF`-is-white convention — a cleared bit is ink, so
//!   [`EpdDriver::clear_frame`](crate::driver::EpdDriver::clear_frame) fills with `0xFF` for a
//!   white background. This is the opposite of the tri-colour
//!   [`SE0352N14TNGA0`](super::SE0352N14TNGA0), which clears with `0x00`.

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for the 2.13" `SFE213ZHBW` Monochrome e-Paper display
/// (UC8253, 122 x 250).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Uc8252c213;

/// All-caps spelling of [`Uc8252c213`], matching the panel's part-number casing.
#[allow(non_camel_case_types)]
pub type UC8252C213 = Uc8252c213;

impl EpdPanel for Uc8252c213 {
    /// Panel physical width in pixels (raster).
    const WIDTH: u32 = 122;

    /// Panel physical height in pixels (raster).
    const HEIGHT: u32 = 250;

    /// Panel color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}
