//! Panel specification for GDEY0213F52 (2.13" 122x250 Quad-Color e-Paper display).
//!
//! ### Hardware Notes:
//! - **Panel Model**: `GDEY0213F52` ([Product Page](https://www.good-display.com/product/463.html)),
//!   rear sticker `0213SW-F52-B2`
//! - **Seeed Studio hardware**: [2.13" Quadruple Color ePaper Display (122x250)](https://www.seeedstudio.com/2-13-Quadruple-Color-ePaper-Display-with-122x250-Pixels-p-5779.html)
//!   (SKU 104990846) carries the same `0213SW-F52-B2` sticker and reads the JD79676AA
//!   revision bytes, so it is this panel.
//! - **Identifying an unlabelled unit**: the flex ribbon is stamped `FPC-J002`, the same stamp
//!   as the `GDEY0213F51` / `ZJY122250-0213AJH-E5`. The ribbon does not tell the two apart.
//!   Check the rear sticker (`F52` vs `F51`), since the controller IC differs.
//! - **Controller IC**: JD79676AA. Use [`Jd79676Controller`](crate::controllers::Jd79676Controller),
//!   which matches Good Display's own demo (`E9 01` then `PON`, the rest from OTP). The
//!   JD79661 init sequence was also seen to draw this panel correctly on a XIAO ESP32-C3
//!   (9 Oct 2026), so the pairing is a parity choice with the vendor demo, not a hard rule.
//! - **Observed `R70H` revision bytes**: `09 02 01` on a Seeed unit (lot `NRA2L21`) and
//!   `89 02 01` on a Good Display unit (lot `N5B3S02`). The JD79676AA datasheet lists
//!   `09 02 01`. The set top bit on the Good Display unit is unexplained.
//! - **Native Resolution**: 122 x 250 pixels
//! - **RAM Alignment**: 128 pixels (32 bytes per row), 8,000 bytes per 2bpp Quad-Color frame.
//! - **Refresh**: full refresh only, no partial. The datasheet says about 11 s and 15.9 s was
//!   measured on an RP2040 with the plain init. `Jd7966xRefreshMode::Fast` on the controller
//!   sends Good Display's V2.0 `EPD_init_Fast` instead (`E0 02`, `E6 5A`, a bare `A5`) and
//!   measured 10.9 s on two units with clean colours. Two of those three commands are not in
//!   the datasheet, so it is opt-in and `Full` stays the default. BUSY is active-low.
//! - **Colour codes**: black `00`, white `01`, yellow `10`, red `11`.

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel specification for Good Display `GDEY0213F52` (JD79676AA).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEY0213F52;

impl EpdPanel for GDEY0213F52 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 122;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 250;

    /// Panel color operating mode (Quad-Color: Black, White, Red, Yellow).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;

    /// The JD79676AA RAM is 128 pixels wide (32 bytes per row), see the "RAM Alignment" note.
    const RAM_WIDTH: u32 = 128;
}
