//! Panel specification for ZJY122250-0213AJH-E5 (2.13" 122x250 Quad-Color e-Paper display).
//!
//! ### Hardware Notes:
//! - **Panel Model**: `ZJY122250-0213AJH-E5` (FPC ribbon cable model stamp)
//! - **Good Display Equivalent**: `GDEY0213F51` ([Product Page](https://www.good-display.com/product/463.html), referenced in GxEPD2 library under `GxEPD2_4C` 4-color family as `GxEPD2_213c_GDEY0213F51`)
//! - **Adafruit Hardware**: Used in Adafruit Product IDs [6373](https://www.adafruit.com/product/6373) and [6366](https://www.adafruit.com/product/6366) (Adafruit ThinkInk 2.13" Quad-Color display breakout: Black, White, Red, Yellow)
//! - **Identifying an unlabelled unit**: the flex ribbon is stamped `FPC-J002` followed
//!   by a batch date code (e.g. `22.02.28`). The same stamp is on the `GDEY0213F52`
//!   family, including the Seeed Studio 5779 (sticker `0213SW-F52-B2`), which uses a
//!   JD79676AA. The ribbon does not separate the two. Check the rear sticker: this
//!   panel's is `ZJY122250-0213...-E5`, the F52's is `0213SW-F52-B2`.
//! - **Observed `R70H` revision bytes**: `09 01 01` on an Adafruit unit (sticker
//!   `ZJY122250-0213EAH-E5`). The JD79661AA datasheet lists `03 02 01`, which was not
//!   seen, so the register is a way to tell units apart, not a confirmed IC check.
//! - **Controller IC**: JD79661
//! - **Native Resolution**: 122 x 250 pixels
//! - **RAM Alignment**: 128 pixels (32 bytes per row) in JD79661 RAM layout (8,000 bytes total per 2bpp QuadColor frame).

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for ZJY122250-0213AJH-E5 / Good Display [GDEY0213F51](https://www.good-display.com/product/463.html) / Adafruit [6373](https://www.adafruit.com/product/6373)/[6366](https://www.adafruit.com/product/6366).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZJY122250_0213AJH_E5;

/// Type alias for Good Display 4-color part number `GDEY0213F51`.
pub type GDEY0213F51 = ZJY122250_0213AJH_E5;

/// GxEPD2 reference alias for this panel (`GxEPD2_213c_GDEY0213F51`).
#[allow(non_camel_case_types)]
pub type GxEPD2_213c_GDEY0213F51 = ZJY122250_0213AJH_E5;

impl EpdPanel for ZJY122250_0213AJH_E5 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 122;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 250;

    /// Panel color operating mode (Quad-Color: Black, White, Red, Yellow).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;

    /// JD79661 pads this 122px panel to 128px (32 bytes/row) RAM columns — see the "RAM
    /// Alignment" note above.
    const RAM_WIDTH: u32 = 128;
}
